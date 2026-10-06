//! `harbor deploy <name> --binary <path>` — ship a binary built on the
//! operator's machine instead of pulling and building on the server.

use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result, bail};

use super::{deploy_cmd, output, remote};
use crate::config::setup::DeployConfig;
use crate::provision::{Provisioner, Spinner};
use crate::script::{LocalDeployComponent, ScriptComponent, deploy_basename, shell_quote};

/// ELF `e_machine` values harbor deploys to.
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;

/// The ELF machine a server type runs: Hetzner `cax*` types are ARM,
/// everything else is x86_64.
#[must_use]
pub(super) fn expected_machine(server_type: &str) -> u16 {
    if server_type.starts_with("cax") {
        EM_AARCH64
    } else {
        EM_X86_64
    }
}

/// Reject anything that is not a little-endian ELF executable for
/// `machine` — catches shipping the macOS build or the wrong arch.
pub(super) fn check_elf(header: &[u8], machine: u16) -> Result<()> {
    let Some(magic) = header.get(..4) else {
        bail!("binary is too small to be an executable");
    };
    if magic != b"\x7fELF" {
        bail!("not a Linux ELF binary (built for the host OS?) — cross-compile for the server");
    }
    let (Some(&lo), Some(&hi)) = (header.get(18), header.get(19)) else {
        bail!("truncated ELF header");
    };
    let found = u16::from_le_bytes([lo, hi]);
    if found != machine {
        bail!(
            "binary is built for ELF machine {found}, server needs {machine} (62 = x86_64, 183 = aarch64)"
        );
    }
    Ok(())
}

/// Render the remote install script for an uploaded binary.
pub(super) fn build_local_script(name: &str, deploy: &DeployConfig, staged: &str) -> String {
    let svc_refs: Vec<&str> = deploy.services.iter().map(String::as_str).collect();
    let component = LocalDeployComponent {
        name: name.to_owned(),
        staged: staged.to_owned(),
        base: deploy_basename(&deploy.binary).to_owned(),
        install: deploy.install.clone(),
        services: deploy.services.clone(),
        health_check: remote::health_check_lines(&svc_refs),
    };
    let mut lines = vec!["#!/bin/bash".to_owned(), "set -e".to_owned(), String::new()];
    lines.extend(remote::lock_preamble());
    lines.push(String::new());
    lines.extend(component.render());
    lines.join("\n")
}

/// Stream `local` to `remote_path` over harbor's pinned ssh.
fn upload(ip: std::net::IpAddr, local: &Path, remote_path: &str) -> Result<()> {
    let file =
        std::fs::File::open(local).with_context(|| format!("opening {}", local.display()))?;
    let status = remote::ssh_command(ip)
        .arg(format!("cat > {}", shell_quote(remote_path)))
        .stdin(Stdio::from(file))
        .stdout(Stdio::null())
        .status()
        .context("running ssh for upload")?;
    if !status.success() {
        bail!("upload of {} failed ({status})", local.display());
    }
    Ok(())
}

/// Validate, upload and install a locally built binary for deploy `name`.
pub(super) async fn run(
    name: &str,
    binary: &Path,
    debug: bool,
    config_path: Option<&Path>,
) -> Result<()> {
    let server = remote::resolve_server(config_path).await?;
    let names = deploy_cmd::sorted_names(&server.config);
    let deploy = deploy_cmd::resolve_entry(&server.config, name, &names)?;
    let server_type = server
        .config
        .server
        .as_ref()
        .map_or("", |s| s.r#type.as_str());
    let header = read_header(binary)?;
    check_elf(&header, expected_machine(server_type))?;

    output::header(&format!(
        "Deploying local {name} to {} ({})",
        server.name, server.ip
    ));
    let spinner = Spinner::start("Uploading binary...", debug);
    let staged = format!("/tmp/harbor-upload-{name}");
    if let Err(e) = upload(server.ip, binary, &staged) {
        spinner.fail();
        return Err(e);
    }
    spinner.set_step("Installing...");
    let script = build_local_script(name, deploy, &staged);
    if let Err(e) = Provisioner::new(debug, false)
        .provision(server.ip, &server.name, &script, Some(&spinner))
        .await
    {
        spinner.fail();
        return Err(e).context(format!("deploy '{name}' failed"));
    }
    spinner.success(format!("Deployed local {name} to {}", server.name));
    Ok(())
}

/// First 64 bytes — enough for the ELF identification and machine fields.
fn read_header(binary: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(64);
    std::fs::File::open(binary)
        .with_context(|| format!("opening {}", binary.display()))?
        .take(64)
        .read_to_end(&mut buf)
        .with_context(|| format!("reading {}", binary.display()))?;
    Ok(buf)
}
