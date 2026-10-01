mod backup;
mod caddy;
mod chrony_nts;
mod deploy;
mod directories;
mod docker;
mod env;
mod fail2ban_rs;
mod files;
mod fish;
mod git_auth;
mod golang;
mod hostname;
mod kernel_hardening;
mod mount_hardening;
mod packages;
mod path;
mod podman;
mod rc_install;
mod rclone_install;
mod rust_lang;
mod services;
mod ssh_hardening;
mod swap;
mod ufw;
mod updates;
mod user;

#[cfg(test)]
mod script_backup_test;
#[cfg(test)]
mod script_backup_units_test;
#[cfg(test)]
mod script_backup_wiring_test;
#[cfg(test)]
mod script_builder_test;
#[cfg(test)]
mod script_components_test;
#[cfg(test)]
mod script_deploy_test;
#[cfg(test)]
mod script_install_test;
#[cfg(test)]
mod script_services_docker_test;
#[cfg(test)]
mod script_services_gap_test;
#[cfg(test)]
mod script_services_podman_test;
#[cfg(test)]
mod script_services_security_test;
#[cfg(test)]
mod script_test_helpers;

#[allow(unused_imports)] // wired into from_setup_config by spec 014
pub use backup::BackupComponent;
pub use caddy::CaddyComponent;
pub use chrony_nts::ChronyNtsComponent;
pub use deploy::{DeployComponent, RollbackComponent};
pub use directories::DirectoriesComponent;
pub use docker::DockerComponent;
pub use env::EnvComponent;
pub use fail2ban_rs::Fail2banRsComponent;
pub use files::{FilesComponent, ResolvedFile};
pub use fish::FishComponent;
pub use git_auth::GitAuthComponent;
pub use golang::GoComponent;
pub use hostname::HostnameComponent;
pub use kernel_hardening::KernelHardeningComponent;
pub use mount_hardening::MountHardeningComponent;
pub use packages::PackagesComponent;
pub use path::PathComponent;
pub use podman::PodmanComponent;
#[allow(unused_imports)] // wired into from_setup_config by spec 014
pub use rc_install::RcInstallComponent;
#[allow(unused_imports)] // wired into from_setup_config by spec 014
pub use rclone_install::RcloneInstallComponent;
pub use rust_lang::RustComponent;
pub use services::ServicesComponent;
pub use ssh_hardening::SshHardeningComponent;
pub use swap::SwapComponent;
pub use ufw::UfwComponent;
pub use updates::UpdatesComponent;
pub use user::SystemUserComponent;

use std::path::Path;

use anyhow::{Context, Result};

use crate::config::setup::SetupSection;
use crate::config::{BackupCredentials, BackupTransport, ContainerRuntime, SetupConfig};

/// Sentinel prefix for harbor status lines. Emitted by `status_echo` and
/// parsed by `provision::output` to drive the spinner. Lets harbor's own
/// progress messages pass through while ignoring arbitrary apt/dpkg chatter.
pub(crate) const STATUS_SENTINEL: &str = "::step::";

/// Produce a bash `echo` line for a harbor status message. The sentinel is
/// stripped by the output filter before it reaches the user.
pub(crate) fn status_echo(msg: &str) -> String {
    format!("echo '{STATUS_SENTINEL} {msg}'")
}

/// Quote `s` as a single bash word: wrap in `'…'`, escaping embedded
/// single quotes as `'\''`.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Go version installed when `components.go.version` is empty.
const DEFAULT_GO_VERSION: &str = "1.24.5";

/// Container hardening fields are silently ignored on native services;
/// say so instead of letting users believe they apply.
fn warn_ignored_container_fields(setup: &SetupSection) {
    for svc in &setup.services {
        if svc.image.is_none()
            && (!svc.cap_drop.is_empty() || !svc.cap_add.is_empty() || svc.read_only)
        {
            tracing::warn!(
                service = svc.name,
                "cap_drop/cap_add/read_only are ignored on native services (no image)"
            );
        }
    }
}

/// A component that can render bash script lines.
pub trait ScriptComponent {
    /// Produce the bash lines for this provisioning step.
    fn render(&self) -> Vec<String>;
}

/// Collects `ScriptComponent`s and builds a complete bash setup script.
pub struct ScriptBuilder {
    components: Vec<Box<dyn ScriptComponent>>,
}

impl ScriptBuilder {
    /// Create an empty script builder.
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Add a component to the script.
    pub fn add(&mut self, component: impl ScriptComponent + 'static) -> &mut Self {
        self.components.push(Box::new(component));
        self
    }

    /// Render the complete bash script.
    pub fn build(&self) -> String {
        let mut lines = vec![
            "#!/bin/bash".to_owned(),
            // `pipefail` is load-bearing: without it, a failure in
            // any command except the last of a pipe (e.g. `curl |
            // gpg --dearmor`) is silently swallowed, and `set -e`
            // doesn't fire. The setup script then marches past the
            // failure and harbor reports success on a broken box.
            "set -eo pipefail".to_owned(),
            String::new(),
            "# Non-interactive apt — no dpkg config file prompts".to_owned(),
            "export DEBIAN_FRONTEND=noninteractive".to_owned(),
            "export APT_LISTCHANGES_FRONTEND=none".to_owned(),
            r"APT_OPTS='-o Dpkg::Options::=--force-confold -o Dpkg::Options::=--force-confdef'"
                .to_owned(),
            String::new(),
            status_echo("Starting server setup"),
            String::new(),
            "# Update package lists".to_owned(),
            "apt-get update".to_owned(),
            String::new(),
        ];

        for component in &self.components {
            let rendered = component.render();
            if !rendered.is_empty() {
                lines.extend(rendered);
                lines.push(String::new());
            }
        }

        lines.push(status_echo("Setup completed successfully"));
        lines.join("\n")
    }

    /// Build a script from a `SetupConfig`.
    ///
    /// `config_dir` is the directory containing the setup YAML — used to resolve
    /// relative `source` paths in `files` entries.
    ///
    /// Returns `Err` if any `ServiceSpec` sets both `image` and a
    /// non-empty `exec_start`, or declares an empty / whitespace-only
    /// `image`. Auto-installs `DockerComponent` and/or `PodmanComponent`
    /// based on which container runtimes are referenced by the declared
    /// services.
    pub fn from_setup_config(
        config: &SetupConfig,
        github_token: &str,
        config_dir: &Path,
        backup_creds: Option<&BackupCredentials>,
    ) -> Result<Self> {
        // Validate here too, so no entry point renders an unvalidated config.
        config.validate()?;
        let setup = &config.setup;
        warn_ignored_container_fields(setup);

        let mut builder = Self::new();
        builder.add_base(setup, github_token);
        builder.add_system(setup);
        builder.add_files(setup, config_dir)?;
        builder.add_security(setup);
        builder.add_infra(setup);
        builder.add_backup(config, backup_creds)?;
        builder.add_runtime(setup);
        Ok(builder)
    }

    /// Apt-repo components, packages, toolchains, and git auth — in that
    /// order, since components that add apt repos must precede packages.
    fn add_base(&mut self, setup: &SetupSection, github_token: &str) {
        if setup.components.fish.enabled {
            self.add(FishComponent);
        }
        if setup.components.caddy.enabled {
            self.add(CaddyComponent);
        }
        if !setup.packages.is_empty() {
            self.add(PackagesComponent {
                packages: setup.packages.clone(),
            });
        }
        if setup.components.go.enabled {
            let version = if setup.components.go.version.is_empty() {
                DEFAULT_GO_VERSION.to_owned()
            } else {
                setup.components.go.version.clone()
            };
            self.add(GoComponent { version });
        }
        if setup.components.rust.enabled {
            self.add(RustComponent);
        }
        if !github_token.is_empty() {
            self.add(GitAuthComponent {
                token: github_token.to_owned(),
            });
        }
    }

    /// PATH, environment, system user, directories, container runtimes,
    /// and timezone.
    fn add_system(&mut self, setup: &SetupSection) {
        if !setup.path.paths.is_empty() {
            self.add(PathComponent {
                mode: setup.path.mode,
                paths: setup.path.paths.clone(),
            });
        }
        if !setup.environment.is_empty() {
            self.add(EnvComponent {
                vars: setup.environment.clone(),
            });
        }
        if !setup.system_user.name.is_empty() {
            self.add(SystemUserComponent {
                name: setup.system_user.name.clone(),
                home: setup.system_user.home.clone(),
                shell: setup.system_user.shell.clone(),
            });
        }
        if !setup.directories.is_empty() {
            self.add(DirectoriesComponent {
                dirs: setup.directories.clone(),
            });
        }
        self.add_container_runtimes(setup);
        if !setup.system.timezone.is_empty() {
            self.add(hostname::TimezoneComponent {
                timezone: setup.system.timezone.clone(),
            });
        }
    }

    /// Docker when enabled or any service runs a Docker image; Podman when
    /// any service selects it.
    fn add_container_runtimes(&mut self, setup: &SetupSection) {
        let uses = |runtime: ContainerRuntime| {
            setup
                .services
                .iter()
                .any(|s| s.image.is_some() && s.runtime == runtime)
        };
        if setup.components.docker.enabled || uses(ContainerRuntime::Docker) {
            self.add(DockerComponent);
        }
        if uses(ContainerRuntime::Podman) {
            self.add(PodmanComponent);
        }
    }

    /// Config files, deployed before services. A missing source is an
    /// error: provisioning without it would report success on a server
    /// missing its config.
    fn add_files(&mut self, setup: &SetupSection, config_dir: &Path) -> Result<()> {
        if setup.files.is_empty() {
            return Ok(());
        }
        let files = setup
            .files
            .iter()
            .map(|f| {
                let source_path = config_dir.join(&f.source);
                let content = std::fs::read_to_string(&source_path)
                    .with_context(|| format!("reading files: source {}", source_path.display()))?;
                Ok(ResolvedFile {
                    target: f.target.clone(),
                    content,
                    owner: f.owner.clone(),
                    group: f.group.clone(),
                    mode: f.mode.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        self.add(FilesComponent { files });
        Ok(())
    }

    /// SSH, kernel, and mount hardening, then the firewall.
    fn add_security(&mut self, setup: &SetupSection) {
        let security = &setup.security;
        if security.ssh_hardening {
            self.add(SshHardeningComponent);
        }
        if security.kernel_hardening {
            self.add(KernelHardeningComponent);
        }
        if security.mount_hardening {
            self.add(MountHardeningComponent);
        }
        if security.ufw.enabled {
            self.add(UfwComponent::from_config(
                &security.ufw.allow_ports,
                &security.ufw.rules,
            ));
        }
    }

    /// Time sync and intrusion prevention.
    fn add_infra(&mut self, setup: &SetupSection) {
        if setup.components.chrony_nts.enabled {
            self.add(ChronyNtsComponent);
        }
        if setup.components.fail2ban_rs.enabled {
            self.add(Fail2banRsComponent);
        }
    }

    /// Backup transport install, then the backup component, so the
    /// rendered scripts can rely on `rc`/`rclone` existing by the time
    /// the timer first fires.
    fn add_backup(
        &mut self,
        config: &SetupConfig,
        backup_creds: Option<&BackupCredentials>,
    ) -> Result<()> {
        let Some(backup) = &config.setup.backup else {
            return Ok(());
        };
        let creds = backup_creds.ok_or_else(|| {
            anyhow::anyhow!(
                "backup: is declared but user config has no backup.projects.{} — \
                 add credentials to ~/.harbor/config.yaml",
                config.name
            )
        })?;
        match backup.transport {
            BackupTransport::Rc => self.add(RcInstallComponent),
            BackupTransport::Rclone => self.add(RcloneInstallComponent),
        };
        self.add(BackupComponent {
            project: config.name.clone(),
            transport: backup.transport,
            destination: backup.destination.clone(),
            endpoint: backup.endpoint.clone(),
            schedule: backup.schedule,
            retention_days: backup.retention_days,
            stop_services: backup.stop_services.clone(),
            paths: backup.paths.clone(),
            access_key_id: creds.access_key_id.clone(),
            secret_access_key: creds.secret_access_key.clone(),
        });
        Ok(())
    }

    /// Swap, services, and finally OS updates. Named `deploys:` are not
    /// run here — `harbor deploy` drives them after `up`.
    fn add_runtime(&mut self, setup: &SetupSection) {
        if !setup.components.swap.size.is_empty() {
            self.add(SwapComponent {
                size: setup.components.swap.size.clone(),
            });
        }
        if !setup.services.is_empty() {
            self.add(ServicesComponent {
                services: setup.services.clone(),
            });
        }
        if setup.updates.auto_upgrade {
            self.add(UpdatesComponent {
                auto_upgrade: setup.updates.auto_upgrade,
                upgrade_kernel: setup.updates.upgrade_kernel,
                reboot_after_kernel: setup.updates.reboot_after_kernel,
            });
        }
    }
}
