use super::{ScriptComponent, status_echo};

/// Harden filesystem mounts: `/tmp`, `/var/tmp`, `/dev/shm` get
/// `noexec,nosuid,nodev`. Prevents writing and executing binaries in
/// world-writable directories.
///
/// Note: binding `/var/tmp` to the `/tmp` tmpfs means `/var/tmp` data
/// does not persist across reboots (unlike the POSIX default).
pub struct MountHardeningComponent;

impl ScriptComponent for MountHardeningComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Applying filesystem mount hardening"),
            // /tmp as noexec tmpfs (idempotent — anchored grep prevents
            // false matches against /var/tmp or comments)
            concat!(
                "grep -q '^tmpfs /tmp ' /etc/fstab || ",
                "echo 'tmpfs /tmp tmpfs defaults,noexec,nosuid,nodev,size=512M 0 0' >> /etc/fstab"
            )
            .to_owned(),
            "mount -o remount /tmp 2>/dev/null || mount /tmp 2>/dev/null || true".to_owned(),
            // /dev/shm as noexec
            concat!(
                "grep -q '^tmpfs /dev/shm ' /etc/fstab || ",
                "echo 'tmpfs /dev/shm tmpfs defaults,noexec,nosuid,nodev 0 0' >> /etc/fstab"
            )
            .to_owned(),
            "mount -o remount /dev/shm 2>/dev/null || true".to_owned(),
            // /var/tmp bind-mounted to /tmp (inherits restrictions)
            concat!(
                "grep -q '^/tmp /var/tmp ' /etc/fstab || ",
                "echo '/tmp /var/tmp none bind 0 0' >> /etc/fstab"
            )
            .to_owned(),
            "mount --bind /tmp /var/tmp 2>/dev/null || true".to_owned(),
        ]
    }
}
