use super::*;

#[test]
fn test_extract_hostname_standard_pattern() {
    assert_eq!(extract_hostname("collector-tergar-prod-nbg1"), "tergar");
}

#[test]
fn test_extract_hostname_multi_part() {
    assert_eq!(extract_hostname("api-server-customer1-dev-fsn1"), "server");
}

#[test]
fn test_extract_hostname_single_word() {
    assert_eq!(extract_hostname("singleword"), "singleword");
}

#[test]
fn test_extract_hostname_empty() {
    assert_eq!(extract_hostname(""), "");
}

#[test]
fn test_extract_hostname_two_parts() {
    assert_eq!(extract_hostname("app-prod"), "prod");
}

#[test]
fn test_full_hostname() {
    assert_eq!(
        full_hostname("tergar", ".i.usercanal.com"),
        "tergar.i.usercanal.com"
    );
}

#[test]
fn test_full_hostname_custom_domain() {
    assert_eq!(full_hostname("api", ".example.com"), "api.example.com");
}

#[test]
fn test_is_configured_both_present() {
    let config = make_config("token", "zone");
    assert!(is_configured(&config));
}

#[test]
fn test_is_configured_missing_token() {
    let config = make_config("", "zone");
    assert!(!is_configured(&config));
}

#[test]
fn test_is_configured_missing_zone() {
    let config = make_config("token", "");
    assert!(!is_configured(&config));
}

#[test]
fn test_is_configured_both_missing() {
    let config = make_config("", "");
    assert!(!is_configured(&config));
}

fn make_config(api_token: &str, zone_id: &str) -> UserConfig {
    use crate::config::{
        CloudflareCredentials, DnsSettings, GitHubCredentials, HetznerCredentials,
    };

    UserConfig {
        cloudflare: CloudflareCredentials {
            api_token: api_token.to_owned(),
            zone_id: zone_id.to_owned(),
        },
        hetzner: HetznerCredentials {
            token: String::new(),
        },
        dns: DnsSettings::default(),
        github: GitHubCredentials::default(),
    }
}
