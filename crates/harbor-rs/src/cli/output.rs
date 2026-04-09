use console::Style;
use std::sync::LazyLock;

static SUCCESS: LazyLock<Style> = LazyLock::new(|| Style::new().green().bold());
static ERROR: LazyLock<Style> = LazyLock::new(|| Style::new().red().bold());
static INFO: LazyLock<Style> = LazyLock::new(|| Style::new().cyan());
static SUBTLE: LazyLock<Style> = LazyLock::new(|| Style::new().dim());
static HEADER: LazyLock<Style> = LazyLock::new(|| Style::new().magenta().bold());

/// Print a success message.
pub fn success(msg: &str) {
    eprintln!("{}", SUCCESS.apply_to(format!("✓ {msg}")));
}

/// Print an error message.
pub fn error(msg: &str) {
    eprintln!("{}", ERROR.apply_to(format!("✗ {msg}")));
}

/// Print an info message.
pub fn info(msg: &str) {
    eprintln!("{}", INFO.apply_to(format!("→ {msg}")));
}

/// Print a subtle/dim message.
pub fn subtle(msg: &str) {
    eprintln!("{}", SUBTLE.apply_to(msg));
}

/// Print a header.
pub fn header(msg: &str) {
    eprintln!("{}", HEADER.apply_to(msg));
}

/// Print a deployment summary table.
pub fn deployment_summary(results: &[DeployResult]) {
    eprintln!();
    header("Deployment Summary");

    let name_w = 30;
    let ip_w = 16;
    let status_w = 10;
    let dur_w = 10;

    eprintln!(
        "{:<name_w$} {:<ip_w$} {:<status_w$} {:<dur_w$}",
        "Server", "IP", "Status", "Duration"
    );
    eprintln!("{}", "─".repeat(name_w + ip_w + status_w + dur_w));

    let mut successful = 0;
    for r in results {
        let ip_str = r.ip.map_or("-".to_owned(), |ip| ip.to_string());
        let (status_str, is_success) = match &r.status {
            DeployStatus::Success => ("success".to_owned(), true),
            DeployStatus::Failed(msg) => (format!("failed: {msg}"), false),
        };
        if is_success {
            successful += 1;
        }

        let dur = format!("{:.0?}", r.duration);
        let styled_status = if is_success {
            SUCCESS.apply_to(&status_str).to_string()
        } else {
            ERROR.apply_to(&status_str).to_string()
        };

        eprintln!(
            "{:<name_w$} {:<ip_w$} {:<status_w$} {:<dur_w$}",
            r.name, ip_str, styled_status, dur
        );
    }

    let failed = results.len() - successful;
    eprintln!(
        "\nTotal: {}, Successful: {}, Failed: {}",
        results.len(),
        SUCCESS.apply_to(successful),
        ERROR.apply_to(failed)
    );
}

/// Result of a single server deployment.
pub struct DeployResult {
    pub name: String,
    pub ip: Option<std::net::IpAddr>,
    pub status: DeployStatus,
    pub duration: std::time::Duration,
}

/// Status of a server deployment.
pub enum DeployStatus {
    Success,
    Failed(String),
}
