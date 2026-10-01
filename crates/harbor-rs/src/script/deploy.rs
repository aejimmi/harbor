use std::path::Path;

use super::{ScriptComponent, status_echo};

/// Clone (or pull) a repo, run build steps, preserve the built binary
/// under `/opt/harbor/<name>/<sha>/<basename>`, and atomically swap a
/// symlink at `install` to point at it.
///
/// Rollback becomes a symlink swap — no rebuild. Old versioned dirs
/// are garbage-collected down to the five most recent on every forward
/// deploy; the active symlink target is never deleted.
///
/// `services` are restarted with `systemctl restart` after the swap
/// and before the health check so the new inode is in use when the
/// health check runs. `health_check` is the precomputed bash block
/// rendered by `remote::health_check_lines`; empty = no block emitted.
pub struct DeployComponent {
    pub name: String,
    pub repo: String,
    pub steps: Vec<String>,
    /// Repo-relative path to the built binary (e.g. `target/release/web`).
    pub binary: String,
    /// Absolute path for the active-version symlink (e.g. `/usr/local/bin/web`).
    pub install: String,
    /// Systemd units to `systemctl restart` after the symlink swap.
    pub services: Vec<String>,
    /// Precomputed health-check bash lines (from `remote::health_check_lines`).
    pub health_check: Vec<String>,
}

impl DeployComponent {
    /// Extract the short repo name from a URL (e.g. `myapp` from `github.com/user/myapp`).
    #[must_use]
    pub fn repo_name(repo: &str) -> &str {
        repo.rsplit('/')
            .next()
            .unwrap_or(repo)
            .trim_end_matches(".git")
    }

    /// Build an HTTPS clone URL from a repo string.
    ///
    /// Inputs that already carry a URL scheme (anything containing
    /// `://`) or that use SSH shorthand (`git@host:user/repo.git`) are
    /// returned unchanged. Everything else — GitHub-style shorthand
    /// like `github.com/user/myapp` — gets `https://` prepended.
    #[must_use]
    pub fn clone_url(repo: &str) -> String {
        if repo.contains("://") || repo.starts_with("git@") {
            repo.to_owned()
        } else {
            format!("https://{repo}")
        }
    }
}

/// Derive the trailing basename from a repo-relative `binary:` path.
/// Used as the filename under `/opt/harbor/<name>/<sha>/<basename>`.
fn basename(binary: &str) -> &str {
    Path::new(binary)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(binary)
}

/// The `HARBOR_INSTALL_ROOT` line — `/opt/harbor` in production, a
/// tempdir in tests. Overridable only via env, never via config.
fn install_root_preamble() -> String {
    r#": "${HARBOR_INSTALL_ROOT:=/opt/harbor}""#.to_owned()
}

/// Clone-or-pull block: clones the repo into `$HOME/<repo_name>` on
/// first run, pulls on subsequent runs.
fn clone_or_pull_lines(repo: &str) -> Vec<String> {
    let repo_name = DeployComponent::repo_name(repo);
    let clone_url = DeployComponent::clone_url(repo);
    vec![
        format!("if [ -d \"$HOME/{repo_name}\" ]; then"),
        format!("  {}", status_echo("Updating existing repo")),
        format!("  cd $HOME/{repo_name} && git pull"),
        "else".to_owned(),
        format!("  {}", status_echo("Cloning repo")),
        format!("  cd $HOME && git clone {clone_url} {repo_name}"),
        "fi".to_owned(),
        format!("cd $HOME/{repo_name}"),
    ]
}

/// Lines that preserve the built binary under the versioned dir and
/// atomically swap the `install` symlink to point at it.
fn preserve_and_swap_lines(name: &str, binary: &str, install: &str) -> Vec<String> {
    let base = basename(binary);
    vec![
        "SHA=$(git rev-parse HEAD)".to_owned(),
        format!("VERSION_DIR=\"$HARBOR_INSTALL_ROOT/{name}/$SHA\""),
        format!("if [ ! -f \"{binary}\" ]; then"),
        format!("  echo 'Built binary {binary} not found' >&2"),
        "  exit 1".to_owned(),
        "fi".to_owned(),
        "mkdir -p \"$VERSION_DIR\"".to_owned(),
        format!("install -m 755 \"{binary}\" \"$VERSION_DIR/{base}\""),
        format!("ln -sfn \"$VERSION_DIR/{base}\" \"{install}.new\""),
        format!("mv -T \"{install}.new\" \"{install}\""),
    ]
}

/// `systemctl restart` for each service — mandatory after a symlink
/// swap because the systemd process still holds the old inode open.
fn restart_lines(services: &[String]) -> Vec<String> {
    services
        .iter()
        .map(|svc| format!("systemctl restart {svc}"))
        .collect()
}

/// Append a `deploys.log` record tagged with the deploy name and the
/// forward/rollback verb. Uses `$SHA` or `$TARGET_SHA` — caller picks.
fn log_lines(sha_var: &str, verb: &str, name: &str) -> Vec<String> {
    vec![
        "mkdir -p ~/.harbor".to_owned(),
        format!(
            "echo \"$(date -u +%Y-%m-%dT%H:%M:%SZ) $(whoami) ${sha_var} {verb} {name}\" >> ~/.harbor/deploys.log"
        ),
    ]
}

/// Retention GC — keep the five most-recent versioned dirs, never the
/// one the active symlink points at. Runs on forward deploys only.
fn gc_lines(name: &str, install: &str) -> Vec<String> {
    vec![
        format!(
            "ls -1dt \"$HARBOR_INSTALL_ROOT/{name}\"/*/ 2>/dev/null | tail -n +6 | while read -r dir; do"
        ),
        format!("  target=\"$(readlink -f \"{install}\")\""),
        "  case \"$target\" in".to_owned(),
        "    \"${dir%/}\"/*) continue ;;".to_owned(),
        "  esac".to_owned(),
        "  rm -rf -- \"$dir\"".to_owned(),
        "done".to_owned(),
    ]
}

impl ScriptComponent for DeployComponent {
    fn render(&self) -> Vec<String> {
        let mut lines = vec![
            status_echo(&format!("Deploying {}", self.repo)),
            install_root_preamble(),
        ];
        lines.extend(clone_or_pull_lines(&self.repo));
        lines.extend(self.steps.iter().cloned());
        lines.extend(preserve_and_swap_lines(
            &self.name,
            &self.binary,
            &self.install,
        ));
        lines.extend(restart_lines(&self.services));
        lines.extend(self.health_check.iter().cloned());
        lines.extend(log_lines("SHA", "deploy", &self.name));
        lines.extend(gc_lines(&self.name, &self.install));
        lines.push(status_echo(&format!("Deploy of {} complete", self.repo)));
        lines
    }
}

/// Roll back a named deploy to a previously-preserved SHA by swapping
/// the `install` symlink. No git, no rebuild.
///
/// If `version` is `Some`, that SHA is the target. If `None`, rollback
/// reads the previous `deploy <name>` line from `~/.harbor/deploys.log`
/// to find the target. A missing versioned dir (pre-feature SHA, or one
/// already garbage-collected) fails loud and leaves the symlink
/// untouched.
pub struct RollbackComponent {
    pub name: String,
    pub version: Option<String>,
    pub binary: String,
    pub install: String,
    pub services: Vec<String>,
    pub health_check: Vec<String>,
}

/// Lines that resolve `TARGET_SHA` from an explicit version or from
/// `deploys.log`. In both cases the result is a bash var the rest of
/// the rollback can reference.
///
/// Without an explicit version, the target is the deploy that came
/// before the currently live one: unique `deploy <name>` SHAs newest
/// first, then the entry after the live SHA (read from the install
/// symlink). Repeated rollbacks therefore keep stepping back instead of
/// flipping between two versions, and a single-deploy history fails
/// instead of "rolling back" to itself.
fn resolve_target_sha(name: &str, install: &str, version: Option<&str>) -> Vec<String> {
    if let Some(sha) = version {
        return vec![format!("TARGET_SHA=\"{sha}\"")];
    }
    vec![
        "if [ ! -f ~/.harbor/deploys.log ]; then".to_owned(),
        "  echo 'No deploy history found' >&2".to_owned(),
        "  exit 1".to_owned(),
        "fi".to_owned(),
        format!("CURRENT_SHA=$(basename \"$(dirname \"$(readlink -f '{install}')\")\")"),
        format!(
            "TARGET_SHA=$(grep -E \" deploy {name}$\" ~/.harbor/deploys.log | awk '{{print $3}}' \
             | tac | awk '!seen[$0]++' \
             | awk -v cur=\"$CURRENT_SHA\" 'found {{ print; exit }} $0 == cur {{ found = 1 }}')"
        ),
        "if [ -z \"$TARGET_SHA\" ]; then".to_owned(),
        format!("  echo \"No version before $CURRENT_SHA found for deploy {name}\" >&2"),
        "  exit 1".to_owned(),
        "fi".to_owned(),
    ]
}

/// Assert the versioned binary exists on disk, then swap the install
/// symlink to point at it. Missing dir = loud failure, no swap.
fn verify_and_swap(name: &str, binary: &str, install: &str) -> Vec<String> {
    let base = basename(binary);
    vec![
        format!("VERSION_DIR=\"$HARBOR_INSTALL_ROOT/{name}/$TARGET_SHA\""),
        format!("TARGET=\"$VERSION_DIR/{base}\""),
        "if [ ! -f \"$TARGET\" ]; then".to_owned(),
        format!(
            "  echo \"binary for $TARGET_SHA not preserved under $HARBOR_INSTALL_ROOT/{name}/ — cannot rollback without rebuild; re-deploy while pinned to that ref via \\`harbor deploy {name}\\`\" >&2"
        ),
        "  exit 1".to_owned(),
        "fi".to_owned(),
        format!("ln -sfn \"$TARGET\" \"{install}.new\""),
        format!("mv -T \"{install}.new\" \"{install}\""),
    ]
}

impl ScriptComponent for RollbackComponent {
    fn render(&self) -> Vec<String> {
        let title = match &self.version {
            Some(sha) => format!("Rolling back {} to {sha}", self.name),
            None => format!("Rolling back {} to previous version", self.name),
        };

        let mut lines = vec![status_echo(&title), install_root_preamble()];
        lines.extend(resolve_target_sha(
            &self.name,
            &self.install,
            self.version.as_deref(),
        ));
        lines.extend(verify_and_swap(&self.name, &self.binary, &self.install));
        lines.extend(restart_lines(&self.services));
        lines.extend(self.health_check.iter().cloned());
        lines.extend(log_lines("TARGET_SHA", "rollback", &self.name));
        lines.push(status_echo("Rollback complete"));
        lines
    }
}
