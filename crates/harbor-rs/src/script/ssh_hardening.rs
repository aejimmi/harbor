use super::ScriptComponent;

/// Apply standard SSH hardening to sshd_config.
pub struct SshHardeningComponent;

impl ScriptComponent for SshHardeningComponent {
    fn render(&self) -> Vec<String> {
        vec![
            "echo 'Applying SSH hardening'".to_owned(),
            "sed -i 's/#PermitRootLogin.*/PermitRootLogin prohibit-password/' /etc/ssh/sshd_config"
                .to_owned(),
            "sed -i 's/#PasswordAuthentication.*/PasswordAuthentication no/' /etc/ssh/sshd_config"
                .to_owned(),
            "cat >> /etc/ssh/sshd_config << 'EOF'
PubkeyAuthentication yes
ChallengeResponseAuthentication no
X11Forwarding no
AllowTcpForwarding no
AllowAgentForwarding no
ClientAliveCountMax 2
LogLevel VERBOSE
MaxAuthTries 3
MaxSessions 2
Banner /etc/issue.net
EOF"
            .to_owned(),
            "echo 'Authorized access only.' > /etc/issue.net".to_owned(),
            "passwd -l root".to_owned(),
            "systemctl restart ssh".to_owned(),
        ]
    }
}
