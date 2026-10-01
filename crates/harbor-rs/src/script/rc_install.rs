//! Install the pinned `rc` (rustfs/cli) transport binary for backups.
//!
//! Bumping `RC_VERSION` is a one-line PR: edit the const, bump the
//! CHANGELOG. The download URL template lives next to the const so
//! reviewers see both in the same diff.
//!
//! Idempotent: if `/usr/local/bin/rc` already reports `RC_VERSION`
//! via `rc --version`, the download is skipped. The rendered bash
//! never uses `curl | bash` piping — harbor fetches the pinned
//! release tarball from GitHub and extracts the binary itself.

use super::{ScriptComponent, status_echo};

/// Pinned `rc` release. Update in a dedicated PR alongside a
/// CHANGELOG entry — no other file needs to change.
pub(crate) const RC_VERSION: &str = "0.1.11";

/// GitHub release URL template. `{version}` and `{arch}` are the
/// only substitutions — no user input ever reaches this string.
///
/// rustfs/cli names its release assets as
/// `rustfs-cli-linux-{arm64|amd64}-gnu-v{version}.tar.gz` (the
/// `-gnu` variant is the glibc build; the `-musl`-less variant is
/// the default we want on Debian/Ubuntu). The tarball contains a
/// single executable named `rc`.
pub(crate) const RC_DOWNLOAD_URL_TEMPLATE: &str = "https://github.com/rustfs/cli/releases/download/v{version}/rustfs-cli-linux-{arch}-gnu-v{version}.tar.gz";

/// Install the pinned `rc` transport binary.
///
/// No fields — matches the `Fail2banRsComponent` ergonomics. All
/// configuration lives in the module-level consts so `harbor up`
/// always installs exactly one version.
pub struct RcInstallComponent;

impl ScriptComponent for RcInstallComponent {
    fn render(&self) -> Vec<String> {
        let url = RC_DOWNLOAD_URL_TEMPLATE
            .replace("{version}", RC_VERSION)
            .replace("{arch}", "$RC_ARCH");

        // Each step is a separate statement so `set -e` catches a
        // failure on any line. Earlier versions joined these with
        // `&&` — bash's documented quirk is that `set -e` does NOT
        // fire when any command except the FINAL `&&` in a chain
        // fails, so a wget/tar/install failure was masked and the
        // downstream "rc X installed" echo still ran.
        vec![
            status_echo(&format!("Installing rc {RC_VERSION}")),
            // dpkg --print-architecture emits the arch names that
            // rustfs/cli uses in its release assets (`arm64`,
            // `amd64`) — pass through, no remap needed.
            "RC_ARCH=$(dpkg --print-architecture)".to_owned(),
            format!(
                "if [ -x /usr/local/bin/rc ] \
                 && /usr/local/bin/rc --version 2>/dev/null | grep -q '{RC_VERSION}'; then"
            ),
            format!("    echo 'rc {RC_VERSION} already installed'"),
            "else".to_owned(),
            format!("    RC_URL=\"{url}\""),
            "    wget -O /tmp/rc.tar.gz \"$RC_URL\"".to_owned(),
            "    tar -xzf /tmp/rc.tar.gz -C /tmp rc".to_owned(),
            "    test -x /tmp/rc".to_owned(),
            "    install -m 0755 -o root -g root /tmp/rc /usr/local/bin/rc".to_owned(),
            "    rm -f /tmp/rc /tmp/rc.tar.gz".to_owned(),
            "fi".to_owned(),
            status_echo(&format!("rc {RC_VERSION} installed")),
        ]
    }
}
