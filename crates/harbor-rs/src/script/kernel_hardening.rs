use super::{ScriptComponent, status_echo};

/// Apply kernel network hardening, disable unused modules, core dumps.
pub struct KernelHardeningComponent;

impl ScriptComponent for KernelHardeningComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Applying kernel hardening"),
            // Disable unused kernel modules
            "cat > /etc/modprobe.d/disable-unused.conf << 'EOF'
install dccp /bin/true
install sctp /bin/true
install rds /bin/true
install tipc /bin/true
EOF"
                .to_owned(),
            // Disable core dumps
            "echo '* hard core 0' >> /etc/security/limits.conf".to_owned(),
            // Network hardening via sysctl
            "cat > /etc/sysctl.d/99-hardening.conf << 'EOF'
net.ipv4.tcp_syncookies = 1
net.ipv4.conf.all.rp_filter = 1
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.all.send_redirects = 0
net.ipv6.conf.all.accept_redirects = 0
net.ipv4.icmp_echo_ignore_broadcasts = 1
net.ipv4.conf.all.log_martians = 1
kernel.randomize_va_space = 2
EOF"
                .to_owned(),
            "sysctl --system".to_owned(),
            // Remove bloat
            "apt-get purge -y docker.io containerd at packagekit 2>/dev/null || true".to_owned(),
            "apt-get autoremove --purge -y".to_owned(),
            // Purge old removed packages
            "dpkg -l | grep '^rc' | awk '{print $2}' | xargs -r apt-get purge -y 2>/dev/null || true"
                .to_owned(),
        ]
    }
}
