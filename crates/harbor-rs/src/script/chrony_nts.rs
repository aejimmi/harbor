use super::{ScriptComponent, status_echo};

/// Install chrony and configure NTP with NTS.
pub struct ChronyNtsComponent;

impl ScriptComponent for ChronyNtsComponent {
    fn render(&self) -> Vec<String> {
        vec![
            status_echo("Configuring NTP with NTS"),
            "apt-get install -y chrony".to_owned(),
            "cat > /etc/chrony/chrony.conf << 'EOF'
confdir /etc/chrony/conf.d

server time.cloudflare.com iburst nts
server nts.netnod.se iburst nts
server virginia.time.system76.com iburst nts

ntsdumpdir /var/lib/chrony
logdir /var/log/chrony
maxupdateskew 100.0
rtcsync
makestep 1 3
leapsectz right/UTC
EOF"
            .to_owned(),
            "systemctl restart chrony".to_owned(),
        ]
    }
}
