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
mod github_repos;
mod golang;
mod hostname;
mod kernel_hardening;
mod packages;
mod path;
mod rust_lang;
mod services;
mod ssh_hardening;
mod swap;
mod ufw;
mod updates;
mod user;

#[cfg(test)]
mod script_test;

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
pub use github_repos::GithubReposComponent;
pub use golang::GoComponent;
pub use hostname::HostnameComponent;
pub use kernel_hardening::KernelHardeningComponent;
pub use packages::PackagesComponent;
pub use path::PathComponent;
pub use rust_lang::RustComponent;
pub use services::ServicesComponent;
pub use ssh_hardening::SshHardeningComponent;
pub use swap::SwapComponent;
pub use ufw::UfwComponent;
pub use updates::UpdatesComponent;
pub use user::SystemUserComponent;

use std::path::Path;

use crate::config::SetupConfig;

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
            "set -e".to_owned(),
            String::new(),
            "# Non-interactive apt — no dpkg config file prompts".to_owned(),
            "export DEBIAN_FRONTEND=noninteractive".to_owned(),
            "export APT_LISTCHANGES_FRONTEND=none".to_owned(),
            r"APT_OPTS='-o Dpkg::Options::=--force-confold -o Dpkg::Options::=--force-confdef'"
                .to_owned(),
            String::new(),
            "echo 'Starting server setup...'".to_owned(),
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

        lines.push("echo 'Setup completed successfully'".to_owned());
        lines.join("\n")
    }

    /// Build a script from a `SetupConfig`.
    ///
    /// `config_dir` is the directory containing the setup YAML — used to resolve
    /// relative `source` paths in `files` entries.
    pub fn from_setup_config(config: &SetupConfig, github_token: &str, config_dir: &Path) -> Self {
        let setup = &config.setup;
        let mut builder = Self::new();

        // --- Components that add apt repos (before packages) ---

        if setup.components.fish.enabled {
            builder.add(FishComponent);
        }

        if setup.components.caddy.enabled {
            builder.add(CaddyComponent);
        }

        // --- Packages ---

        if !setup.packages.is_empty() {
            builder.add(PackagesComponent {
                packages: setup.packages.clone(),
            });
        }

        // --- Language toolchains ---

        if setup.components.go.enabled {
            let version = if setup.components.go.version.is_empty() {
                "1.24.5".to_owned()
            } else {
                setup.components.go.version.clone()
            };
            builder.add(GoComponent { version });
        }

        if setup.components.rust.enabled {
            builder.add(RustComponent);
        }

        // --- Git HTTPS auth ---

        if !github_token.is_empty() {
            builder.add(GitAuthComponent {
                token: github_token.to_owned(),
            });
        }

        // --- System configuration ---

        if !setup.path.paths.is_empty() {
            builder.add(PathComponent {
                mode: setup.path.mode,
                paths: setup.path.paths.clone(),
            });
        }

        if !setup.environment.is_empty() {
            builder.add(EnvComponent {
                vars: setup.environment.clone(),
            });
        }

        if !setup.system_user.name.is_empty() {
            builder.add(SystemUserComponent {
                name: setup.system_user.name.clone(),
                home: setup.system_user.home.clone(),
                shell: setup.system_user.shell.clone(),
            });
        }

        if !setup.directories.is_empty() {
            builder.add(DirectoriesComponent {
                dirs: setup.directories.clone(),
            });
        }

        if setup.components.docker.enabled {
            builder.add(DockerComponent);
        }

        if !setup.github_repos.is_empty() {
            builder.add(GithubReposComponent {
                repos: setup.github_repos.clone(),
                github_token: github_token.to_owned(),
                system_user: if setup.system_user.name.is_empty() {
                    None
                } else {
                    Some(setup.system_user.name.clone())
                },
            });
        }

        if !setup.system.timezone.is_empty() {
            builder.add(hostname::TimezoneComponent {
                timezone: setup.system.timezone.clone(),
            });
        }

        // --- Files (deploy config files before services) ---

        if !setup.files.is_empty() {
            let resolved: Vec<ResolvedFile> = setup
                .files
                .iter()
                .filter_map(|f| {
                    let source_path = config_dir.join(&f.source);
                    match std::fs::read_to_string(&source_path) {
                        Ok(content) => Some(ResolvedFile {
                            target: f.target.clone(),
                            content,
                            owner: f.owner.clone(),
                            group: f.group.clone(),
                            mode: f.mode.clone(),
                        }),
                        Err(e) => {
                            eprintln!(
                                "Warning: could not read file {}: {e}",
                                source_path.display()
                            );
                            None
                        }
                    }
                })
                .collect();

            if !resolved.is_empty() {
                builder.add(FilesComponent { files: resolved });
            }
        }

        // --- Security ---

        if setup.security.ssh_hardening {
            builder.add(SshHardeningComponent);
        }

        if setup.security.kernel_hardening {
            builder.add(KernelHardeningComponent);
        }

        if setup.security.ufw.enabled {
            builder.add(UfwComponent::from_config(
                &setup.security.ufw.allow_ports,
                &setup.security.ufw.rules,
            ));
        }

        // --- Infrastructure components ---

        if setup.components.chrony_nts.enabled {
            builder.add(ChronyNtsComponent);
        }

        if setup.components.fail2ban_rs.enabled {
            builder.add(Fail2banRsComponent);
        }

        if !setup.components.swap.size.is_empty() {
            builder.add(SwapComponent {
                size: setup.components.swap.size.clone(),
            });
        }

        // --- Deploy (clone + build + install) ---

        if let Some(ref deploy) = setup.deploy {
            builder.add(DeployComponent {
                repo: deploy.repo.clone(),
                steps: deploy.steps.clone(),
            });
        }

        // --- Services ---

        if !setup.services.is_empty() {
            builder.add(ServicesComponent {
                services: setup.services.clone(),
            });
        }

        // --- Updates (last) ---

        if setup.updates.auto_upgrade {
            builder.add(UpdatesComponent {
                auto_upgrade: setup.updates.auto_upgrade,
                upgrade_kernel: setup.updates.upgrade_kernel,
                reboot_after_kernel: setup.updates.reboot_after_kernel,
            });
        }

        builder
    }
}
