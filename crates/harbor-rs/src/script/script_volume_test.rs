use super::*;
use crate::config::{SetupConfig, VolumeFormat};

fn tell_mount() -> VolumeMount {
    VolumeMount {
        name: "tell-data".to_owned(),
        device: "/dev/disk/by-id/scsi-0HC_Volume_42".to_owned(),
        mount: "/opt/tell".to_owned(),
        format: VolumeFormat::Ext4,
    }
}

fn render_mounts(volumes: Vec<VolumeMount>) -> String {
    VolumeMountComponent { volumes }.render().join("\n")
}

#[test]
fn test_volume_mount_formats_only_blank_device() {
    let out = render_mounts(vec![tell_mount()]);
    assert!(out.contains("dev='/dev/disk/by-id/scsi-0HC_Volume_42'"));
    assert!(
        out.contains("blkid -o value -s TYPE \"$dev\" >/dev/null 2>&1 || mkfs.ext4 -q \"$dev\""),
        "mkfs must be guarded by blkid:\n{out}"
    );
}

#[test]
fn test_volume_mount_waits_for_device_and_fails_when_missing() {
    let out = render_mounts(vec![tell_mount()]);
    assert!(out.contains("for _ in $(seq 1 60); do [ -e \"$dev\" ] && break; sleep 1; done"));
    assert!(out.contains("did not appear\" >&2; exit 1; }"));
}

#[test]
fn test_volume_mount_writes_uuid_fstab_entry_with_nofail_once() {
    let out = render_mounts(vec![tell_mount()]);
    assert!(out.contains("grep -q \"^UUID=$uuid \" /etc/fstab ||"));
    assert!(out.contains("UUID=$uuid /opt/tell $fstype defaults,nofail,discard 0 2"));
}

#[test]
fn test_volume_mount_is_idempotent_on_mountpoint() {
    let out = render_mounts(vec![tell_mount()]);
    assert!(out.contains("mkdir -p '/opt/tell'"));
    assert!(out.contains("mountpoint -q '/opt/tell' || mount '/opt/tell'"));
    assert!(out.ends_with("systemctl daemon-reload"));
}

#[test]
fn test_volume_mount_uses_declared_format() {
    let xfs = VolumeMount {
        format: VolumeFormat::Xfs,
        ..tell_mount()
    };
    assert!(render_mounts(vec![xfs]).contains("mkfs.xfs -q"));
}

#[test]
fn test_volume_mount_renders_nothing_without_volumes() {
    assert!(VolumeMountComponent { volumes: vec![] }.render().is_empty());
}

#[test]
fn test_prepend_puts_volume_mount_before_directories() {
    let yaml = "name: tell\nsetup:\n  directories:\n    - { path: /opt/tell/data, owner: root, group: root, mode: \"700\" }\n";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("test yaml should parse");
    let mut builder =
        ScriptBuilder::from_setup_config(&config, "", std::path::Path::new("."), None)
            .expect("script should build");
    builder.prepend(VolumeMountComponent {
        volumes: vec![tell_mount()],
    });
    let script = builder.build();
    let mount_at = script
        .find("Mounting volume tell-data")
        .expect("mount step rendered");
    let dirs_at = script
        .find("mkdir -p /opt/tell/data")
        .expect("directory step rendered");
    assert!(mount_at < dirs_at, "volume must mount before directories");
}

#[test]
fn test_journald_component_writes_drop_in_and_restarts() {
    let out = JournaldComponent {
        max_use: "1G".to_owned(),
    }
    .render()
    .join("\n");
    assert!(out.contains(
        "printf '[Journal]\\nSystemMaxUse=1G\\n' > /etc/systemd/journald.conf.d/harbor.conf"
    ));
    assert!(out.contains("systemctl restart systemd-journald"));
}

#[test]
fn test_journald_wired_from_system_config() {
    let yaml = "name: tell\nsetup:\n  system:\n    journald_max_use: 1G\n";
    let config: SetupConfig = serde_yaml::from_str(yaml).expect("test yaml should parse");
    let script = ScriptBuilder::from_setup_config(&config, "", std::path::Path::new("."), None)
        .expect("script should build")
        .build();
    assert!(script.contains("SystemMaxUse=1G"));
}

#[test]
fn test_journald_absent_by_default() {
    let config: SetupConfig =
        serde_yaml::from_str("name: tell\nsetup: {}\n").expect("test yaml should parse");
    let script = ScriptBuilder::from_setup_config(&config, "", std::path::Path::new("."), None)
        .expect("script should build")
        .build();
    assert!(!script.contains("journald"));
}
