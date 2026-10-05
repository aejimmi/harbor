//! Bash-execution integration tests for the deploy / rollback pipeline.
//!
//! Gated to Linux: rendered bash uses `mv -T` (GNU coreutils only).

#![cfg(target_os = "linux")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used
)]

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use crate::script::{DeployComponent, RollbackComponent, ScriptComponent};

const DEPLOY_NAME: &str = "web";

// --- git helpers ---

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("/usr/bin/git")
        .args(args)
        .env("GIT_AUTHOR_NAME", "harbor-test")
        .env("GIT_AUTHOR_EMAIL", "test@harbor")
        .env("GIT_COMMITTER_NAME", "harbor-test")
        .env("GIT_COMMITTER_EMAIL", "test@harbor")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(cwd)
        .status()
        .expect("git command failed to spawn");
    assert!(status.success(), "git {args:?} exited {status}");
}

fn git_output(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("/usr/bin/git")
        .args(args)
        .env("GIT_AUTHOR_NAME", "harbor-test")
        .env("GIT_AUTHOR_EMAIL", "test@harbor")
        .env("GIT_COMMITTER_NAME", "harbor-test")
        .env("GIT_COMMITTER_EMAIL", "test@harbor")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(cwd)
        .output()
        .expect("git command failed to spawn");
    assert!(out.status.success(), "git {args:?} exited {}", out.status);
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn git_commit(cwd: &Path, work_path: &str, msg: &str) {
    git(cwd, &["-C", work_path, "commit", "-m", msg]);
}

// --- repo fixtures ---

/// Initialise a bare git repo, seed it with `VERSION=<version>`, return `(bare, sha)`.
fn make_origin(tmp: &Path, version: &str) -> (PathBuf, String) {
    let bare = tmp.join("origin.git");
    let work = tmp.join("work_seed");
    std::fs::create_dir_all(&work).unwrap();
    git(
        tmp,
        &[
            "init",
            "--bare",
            "--initial-branch=main",
            bare.to_str().unwrap(),
        ],
    );
    git(
        tmp,
        &[
            "-C",
            work.to_str().unwrap(),
            "init",
            "--initial-branch=main",
        ],
    );
    git(
        tmp,
        &[
            "-C",
            work.to_str().unwrap(),
            "remote",
            "add",
            "origin",
            bare.to_str().unwrap(),
        ],
    );
    std::fs::write(work.join("VERSION"), format!("VERSION={version}\n")).unwrap();
    git(tmp, &["-C", work.to_str().unwrap(), "add", "VERSION"]);
    git_commit(tmp, work.to_str().unwrap(), "init");
    git(
        tmp,
        &["-C", work.to_str().unwrap(), "push", "origin", "main"],
    );
    let sha = git_output(tmp, &["-C", work.to_str().unwrap(), "rev-parse", "HEAD"]);
    (bare, sha)
}

/// Push a new VERSION commit to `bare_repo`, return new SHA.
fn push_new_version(tmp: &Path, bare_repo: &Path, version: &str) -> String {
    let work = tmp.join(format!("work_push_{version}"));
    git(
        tmp,
        &["clone", bare_repo.to_str().unwrap(), work.to_str().unwrap()],
    );
    std::fs::write(work.join("VERSION"), format!("VERSION={version}\n")).unwrap();
    git(tmp, &["-C", work.to_str().unwrap(), "add", "VERSION"]);
    git_commit(tmp, work.to_str().unwrap(), &format!("bump to {version}"));
    git(
        tmp,
        &["-C", work.to_str().unwrap(), "push", "origin", "main"],
    );
    git_output(tmp, &["-C", work.to_str().unwrap(), "rev-parse", "HEAD"])
}

fn file_url(bare: &Path) -> String {
    format!("file://{}", bare.display())
}

// --- script runner ---

fn run_script(fake_home: &Path, install_root: &Path, lines: &[String]) -> (ExitStatus, String) {
    let mut script = "#!/bin/bash\nset -e\n\n".to_owned();
    for line in lines {
        script.push_str(line);
        script.push('\n');
    }
    let script_path = fake_home.join("_harbor_test_script.sh");
    std::fs::write(&script_path, &script).unwrap();
    let out = Command::new("/bin/bash")
        .arg(script_path.to_str().unwrap())
        .env_clear()
        .env("HOME", fake_home.to_str().unwrap())
        .env("HARBOR_INSTALL_ROOT", install_root.to_str().unwrap())
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_AUTHOR_NAME", "harbor-test")
        .env("GIT_AUTHOR_EMAIL", "test@harbor")
        .env("GIT_COMMITTER_NAME", "harbor-test")
        .env("GIT_COMMITTER_EMAIL", "test@harbor")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("failed to spawn /bin/bash");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status, combined)
}

// --- assertion helpers ---

fn versioned_dir_count(install_root: &Path, name: &str) -> usize {
    let dir = install_root.join(name);
    match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count(),
        Err(_) => 0,
    }
}

fn read_log(fake_home: &Path) -> String {
    std::fs::read_to_string(fake_home.join(".harbor/deploys.log")).expect("deploys.log not found")
}

fn assert_log_line_count(fake_home: &Path, expected: usize) {
    let log = read_log(fake_home);
    let count = log.lines().count();
    assert_eq!(
        count, expected,
        "expected {expected} log lines, got {count}:\n{log}"
    );
}

fn assert_log_contains(fake_home: &Path, needle: &str) {
    let log = read_log(fake_home);
    assert!(log.contains(needle), "log missing {needle:?}:\n{log}");
}

fn render_deploy(url: &str, install: &Path) -> Vec<String> {
    DeployComponent {
        name: DEPLOY_NAME.to_owned(),
        repo: url.to_owned(),
        steps: Vec::new(),
        binary: "VERSION".to_owned(),
        install: install.to_str().unwrap().to_owned(),
        services: Vec::new(),
        health_check: Vec::new(),
    }
    .render()
}

fn render_rollback(version: &str, install: &Path) -> Vec<String> {
    RollbackComponent {
        name: DEPLOY_NAME.to_owned(),
        version: Some(version.to_owned()),
        binary: "VERSION".to_owned(),
        install: install.to_str().unwrap().to_owned(),
        services: Vec::new(),
        health_check: Vec::new(),
    }
    .render()
}

// --- tests ---

/// Fresh deploy: versioned dir and symlink created; log has one entry.
#[test]
fn test_deploy_creates_versioned_dir_and_symlink() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, sha1) = make_origin(tmp.path(), "1.0");

    let lines = render_deploy(&file_url(&bare), &install_link);
    let (status, output) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(status.success(), "script failed:\n{output}");

    let versioned = install_root
        .path()
        .join(DEPLOY_NAME)
        .join(&sha1)
        .join("VERSION");
    assert!(versioned.exists(), "versioned file missing");
    assert!(std::fs::read_to_string(&versioned).unwrap().contains("1.0"));

    let meta = std::fs::symlink_metadata(&install_link).expect("install symlink missing");
    assert!(
        meta.file_type().is_symlink(),
        "install path is not a symlink"
    );
    assert_eq!(
        std::fs::canonicalize(&install_link).unwrap(),
        std::fs::canonicalize(&versioned).unwrap()
    );
    assert!(
        std::fs::read_to_string(&install_link)
            .unwrap()
            .contains("1.0")
    );

    assert_log_line_count(fake_home.path(), 1);
    assert_log_contains(fake_home.path(), &format!("deploy {DEPLOY_NAME}"));
    assert_log_contains(fake_home.path(), &sha1);
}

/// Second deploy: both versioned dirs exist; symlink points at the newer one.
#[test]
fn test_second_deploy_swaps_symlink_atomically() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, sha1) = make_origin(tmp.path(), "1.0");
    let lines = render_deploy(&file_url(&bare), &install_link);

    let (s1, o1) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(s1.success(), "first deploy failed:\n{o1}");

    let sha2 = push_new_version(tmp.path(), &bare, "2.0");
    let (s2, o2) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(s2.success(), "second deploy failed:\n{o2}");

    let dir1 = install_root
        .path()
        .join(DEPLOY_NAME)
        .join(&sha1)
        .join("VERSION");
    let dir2 = install_root
        .path()
        .join(DEPLOY_NAME)
        .join(&sha2)
        .join("VERSION");
    assert!(dir1.exists(), "sha1 versioned file missing");
    assert!(dir2.exists(), "sha2 versioned file missing");
    assert_eq!(
        std::fs::canonicalize(&install_link).unwrap(),
        std::fs::canonicalize(&dir2).unwrap()
    );
    assert!(
        std::fs::read_to_string(&install_link)
            .unwrap()
            .contains("2.0")
    );
    assert_log_line_count(fake_home.path(), 2);
}

/// Rollback: symlink swaps back to sha1 without touching git HEAD.
#[test]
fn test_rollback_swaps_symlink_without_git_or_rebuild() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, sha1) = make_origin(tmp.path(), "1.0");
    let lines = render_deploy(&file_url(&bare), &install_link);

    let (s1, o1) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(s1.success(), "deploy v1 failed:\n{o1}");
    let sha2 = push_new_version(tmp.path(), &bare, "2.0");
    let (s2, o2) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(s2.success(), "deploy v2 failed:\n{o2}");

    let rb_lines = render_rollback(&sha1, &install_link);
    let (sr, or) = run_script(fake_home.path(), install_root.path(), &rb_lines);
    assert!(sr.success(), "rollback failed:\n{or}");

    let sha1_versioned = install_root
        .path()
        .join(DEPLOY_NAME)
        .join(&sha1)
        .join("VERSION");
    assert_eq!(
        std::fs::canonicalize(&install_link).unwrap(),
        std::fs::canonicalize(&sha1_versioned).unwrap()
    );
    assert!(
        std::fs::read_to_string(&install_link)
            .unwrap()
            .contains("1.0")
    );

    // git HEAD in $HOME/origin is still sha2 — rollback never touched git
    let repo_dir = fake_home.path().join("origin");
    let head = git_output(
        repo_dir.parent().unwrap(),
        &["-C", repo_dir.to_str().unwrap(), "rev-parse", "HEAD"],
    );
    assert_eq!(head, sha2, "git HEAD changed — rollback must not touch git");
    assert_log_contains(fake_home.path(), &format!("rollback {DEPLOY_NAME}"));
}

/// Rollback to a fabricated SHA exits non-zero; symlink stays unchanged.
#[test]
fn test_rollback_to_missing_sha_exits_nonzero_without_changing_symlink() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, _sha1) = make_origin(tmp.path(), "1.0");

    let (s1, o1) = run_script(
        fake_home.path(),
        install_root.path(),
        &render_deploy(&file_url(&bare), &install_link),
    );
    assert!(s1.success(), "deploy failed:\n{o1}");
    let baseline = std::fs::canonicalize(&install_link).unwrap();

    let fake_sha = "deadbeef1234567890abcdef1234567890abcdef";
    let (sr, output) = run_script(
        fake_home.path(),
        install_root.path(),
        &render_rollback(fake_sha, &install_link),
    );
    assert!(!sr.success(), "expected non-zero exit for missing SHA");
    assert!(
        output.contains("not preserved") || output.contains("rebuild"),
        "expected error about missing preserved binary, got:\n{output}"
    );
    assert_eq!(
        std::fs::canonicalize(&install_link).unwrap(),
        baseline,
        "symlink changed"
    );
    assert_log_line_count(fake_home.path(), 1);
}

/// Failing build step aborts before symlink swap; no versioned dirs created.
#[test]
fn test_failing_build_aborts_before_symlink_swap() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, _) = make_origin(tmp.path(), "1.0");

    let lines = DeployComponent {
        name: DEPLOY_NAME.to_owned(),
        repo: file_url(&bare),
        steps: vec!["false".to_owned()],
        binary: "VERSION".to_owned(),
        install: install_link.to_str().unwrap().to_owned(),
        services: Vec::new(),
        health_check: Vec::new(),
    }
    .render();

    let (status, _output) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(
        !status.success(),
        "expected non-zero exit from failing step"
    );
    assert_eq!(
        versioned_dir_count(install_root.path(), DEPLOY_NAME),
        0,
        "versioned dirs must not exist"
    );
    assert!(
        std::fs::symlink_metadata(&install_link).is_err(),
        "install symlink must not exist after failing build"
    );
    assert!(
        !fake_home.path().join(".harbor/deploys.log").exists(),
        "deploys.log must not exist"
    );
}

/// Six deploys leave exactly 5 versioned dirs; oldest removed; symlink at newest.
#[test]
fn test_retention_keeps_last_five_versions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let fake_home = tempfile::TempDir::new().unwrap();
    let install_root = tempfile::TempDir::new().unwrap();
    let install_link = install_root.path().join("web-link");
    let (bare, sha1) = make_origin(tmp.path(), "1.0");
    let lines = render_deploy(&file_url(&bare), &install_link);

    let (s, o) = run_script(fake_home.path(), install_root.path(), &lines);
    assert!(s.success(), "deploy 1 failed:\n{o}");

    let mut shas = vec![sha1.clone()];
    for i in 2..=6 {
        let sha = push_new_version(tmp.path(), &bare, &format!("{i}.0"));
        shas.push(sha);
        let (si, oi) = run_script(fake_home.path(), install_root.path(), &lines);
        assert!(si.success(), "deploy {i} failed:\n{oi}");
    }
    let sha6 = &shas[5];

    let count = versioned_dir_count(install_root.path(), DEPLOY_NAME);
    assert_eq!(
        count, 5,
        "expected 5 versioned dirs after 6 deploys, got {count}"
    );

    assert!(
        !install_root.path().join(DEPLOY_NAME).join(&sha1).exists(),
        "oldest versioned dir should have been GC'd"
    );
    for sha in &shas[1..] {
        let dir = install_root.path().join(DEPLOY_NAME).join(sha);
        assert!(dir.exists(), "versioned dir missing for sha {sha}");
    }

    let sha6_versioned = install_root
        .path()
        .join(DEPLOY_NAME)
        .join(sha6)
        .join("VERSION");
    assert_eq!(
        std::fs::canonicalize(&install_link).unwrap(),
        std::fs::canonicalize(&sha6_versioned).unwrap()
    );
    assert_log_line_count(fake_home.path(), 6);
}
