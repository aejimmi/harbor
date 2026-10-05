//! Mount harbor-managed block volumes before anything writes to them.

use crate::config::VolumeFormat;

use super::{ScriptComponent, shell_quote, status_echo};

/// Seconds to wait for a freshly attached volume's device link.
const DEVICE_WAIT_SECS: u32 = 60;

/// One attached volume to mount.
#[derive(Debug, Clone)]
pub struct VolumeMount {
    pub name: String,
    /// Device path from the provider, e.g. `/dev/disk/by-id/scsi-0HC_Volume_1`.
    pub device: String,
    /// Absolute mount point (validated at config load).
    pub mount: String,
    /// Filesystem to write only when the device is blank.
    pub format: VolumeFormat,
}

/// Format blank volumes, add a `UUID=` fstab entry with `nofail`, and
/// mount. Idempotent: a device that already holds a filesystem is
/// never reformatted, and an existing fstab entry is not duplicated.
pub struct VolumeMountComponent {
    pub volumes: Vec<VolumeMount>,
}

impl ScriptComponent for VolumeMountComponent {
    fn render(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for vol in &self.volumes {
            lines.extend(render_one(vol));
        }
        if !lines.is_empty() {
            lines.push("systemctl daemon-reload".to_owned());
        }
        lines
    }
}

fn render_one(vol: &VolumeMount) -> Vec<String> {
    let dev = shell_quote(&vol.device);
    let mount = shell_quote(&vol.mount);
    let fs = vol.format.as_str();
    vec![
        status_echo(&format!("Mounting volume {} at {}", vol.name, vol.mount)),
        format!("dev={dev}"),
        format!("for _ in $(seq 1 {DEVICE_WAIT_SECS}); do [ -e \"$dev\" ] && break; sleep 1; done"),
        "[ -e \"$dev\" ] || { echo \"volume device $dev did not appear\" >&2; exit 1; }".to_owned(),
        // blkid exits non-zero on a device without a filesystem.
        format!("blkid -o value -s TYPE \"$dev\" >/dev/null 2>&1 || mkfs.{fs} -q \"$dev\""),
        "fstype=$(blkid -o value -s TYPE \"$dev\")".to_owned(),
        "uuid=$(blkid -o value -s UUID \"$dev\")".to_owned(),
        "[ -n \"$uuid\" ] || { echo \"volume device $dev has no UUID\" >&2; exit 1; }".to_owned(),
        format!("mkdir -p {mount}"),
        format!(
            "grep -q \"^UUID=$uuid \" /etc/fstab || \
             echo \"UUID=$uuid {} $fstype defaults,nofail,discard 0 2\" >> /etc/fstab",
            vol.mount
        ),
        format!("mountpoint -q {mount} || mount {mount}"),
    ]
}
