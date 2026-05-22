//! Agent status display — kubectl-style status tables.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Health status of an agent or component.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// Running and healthy.
    Running,
    /// Degraded performance or partial failure.
    Degraded,
    /// Not responding or failed.
    Failed,
    /// Pending assignment or startup.
    Pending,
    /// Gracefully shut down.
    Terminated,
    /// Unknown state.
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Failed => write!(f, "Failed"),
            Self::Pending => write!(f, "Pending"),
            Self::Terminated => write!(f, "Terminated"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Status report for a single agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusReport {
    /// Agent identifier.
    pub agent_id: String,
    /// Human-readable agent name.
    pub name: String,
    /// Current health status.
    pub status: HealthStatus,
    /// Node the agent is running on.
    pub node: Option<String>,
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Number of active sessions.
    pub active_sessions: u32,
    /// Total completed tasks.
    pub completed_tasks: u64,
    /// Total failed tasks.
    pub failed_tasks: u64,
    /// Current CPU usage (0.0 - 1.0 per core).
    pub cpu_usage: f64,
    /// Current memory usage in bytes.
    pub memory_bytes: u64,
    /// Total tokens consumed.
    pub tokens_consumed: u64,
    /// Last heartbeat timestamp.
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// Agent version.
    pub version: String,
    /// Labels/tags.
    pub labels: Vec<(String, String)>,
    /// Restart count.
    pub restart_count: u32,
}

impl AgentStatusReport {
    /// Format as a kubectl-style table row.
    pub fn table_row(&self, wide: bool) -> String {
        let status_icon = match &self.status {
            HealthStatus::Running => "●",
            HealthStatus::Degraded => "◐",
            HealthStatus::Failed => "✖",
            HealthStatus::Pending => "◌",
            HealthStatus::Terminated => "○",
            HealthStatus::Unknown => "?",
        };
        let uptime = format_duration(self.uptime_secs);
        let mem = format_bytes(self.memory_bytes);
        let tokens = format_number(self.tokens_consumed);

        if wide {
            format!(
                "{icon} {id:<16} {status:<10} {node:<12} {uptime:<10} \
                 {sess:<5} {cpu:<8} {mem:<10} {tok:<10} {ver:<8} {restarts}",
                icon = status_icon,
                id = self.agent_id,
                status = self.status,
                node = self.node.as_deref().unwrap_or("-"),
                uptime = uptime,
                sess = self.active_sessions,
                cpu = format!("{:.1}%", self.cpu_usage * 100.0),
                mem = mem,
                tok = tokens,
                ver = self.version,
                restarts = self.restart_count,
            )
        } else {
            format!(
                "{icon} {id:<16} {status:<10} {node:<12} {uptime:<10} \
                 {sess:<5} {cpu:<8} {mem:<10}",
                icon = status_icon,
                id = self.agent_id,
                status = self.status,
                node = self.node.as_deref().unwrap_or("-"),
                uptime = uptime,
                sess = self.active_sessions,
                cpu = format!("{:.1}%", self.cpu_usage * 100.0),
                mem = mem,
            )
        }
    }

    /// Print header row.
    pub fn table_header(wide: bool) -> String {
        if wide {
            format!(
                "  {id:<16} {status:<10} {node:<12} {uptime:<10} \
                 {sess:<5} {cpu:<8} {mem:<10} {tok:<10} {ver:<8} {r}",
                id = "AGENT",
                status = "STATUS",
                node = "NODE",
                uptime = "UPTIME",
                sess = "SESS",
                cpu = "CPU",
                mem = "MEMORY",
                tok = "TOKENS",
                ver = "VERSION",
                r = "RESTARTS",
            )
        } else {
            format!(
                "  {id:<16} {status:<10} {node:<12} {uptime:<10} \
                 {sess:<5} {cpu:<8} {mem:<10}",
                id = "AGENT",
                status = "STATUS",
                node = "NODE",
                uptime = "UPTIME",
                sess = "SESS",
                cpu = "CPU",
                mem = "MEMORY",
            )
        }
    }
}

/// Print a status table for multiple agents.
pub fn print_status_table(reports: &[AgentStatusReport], wide: bool) {
    println!("{}", AgentStatusReport::table_header(wide));
    for report in reports {
        println!("{}", report.table_row(wide));
    }
    // Summary line
    let running = reports
        .iter()
        .filter(|r| r.status == HealthStatus::Running)
        .count();
    let total = reports.len();
    println!("\n{running}/{total} agents running");
}

/// Print status in JSON format.
pub fn print_status_json(reports: &[AgentStatusReport]) {
    match serde_json::to_string_pretty(reports) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error: {e}"),
    }
}

// ── Formatting helpers ──────────────────────────────────────────────────

pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{}h", secs / 86400, (secs % 86400) / 3600)
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn format_number(n: u64) -> String {
    if n < 1_000 {
        format!("{n}")
    } else if n < 1_000_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else if n < 1_000_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else {
        format!("{:.1}G", n as f64 / 1_000_000_000.0)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_report(id: &str, status: HealthStatus) -> AgentStatusReport {
        AgentStatusReport {
            agent_id: id.to_string(),
            name: format!("Agent {id}"),
            status,
            node: Some("node-1".to_string()),
            uptime_secs: 3725,
            active_sessions: 3,
            completed_tasks: 42,
            failed_tasks: 2,
            cpu_usage: 0.45,
            memory_bytes: 536_870_912,
            tokens_consumed: 1_250_000,
            last_heartbeat: Some(Utc::now()),
            version: "1.0.0".to_string(),
            labels: vec![("env".to_string(), "prod".to_string())],
            restart_count: 0,
        }
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Running.to_string(), "Running");
        assert_eq!(HealthStatus::Degraded.to_string(), "Degraded");
        assert_eq!(HealthStatus::Failed.to_string(), "Failed");
        assert_eq!(HealthStatus::Pending.to_string(), "Pending");
        assert_eq!(HealthStatus::Terminated.to_string(), "Terminated");
        assert_eq!(HealthStatus::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(90), "1m30s");
        assert_eq!(format_duration(3725), "1h2m");
        assert_eq!(format_duration(90000), "1d1h");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1536), "1.5KiB");
        assert_eq!(format_bytes(536_870_912), "512.0MiB");
        assert_eq!(format_bytes(2_147_483_648), "2.0GiB");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(1_250_000), "1.2M");
        assert_eq!(format_number(2_000_000_000), "2.0G");
    }

    #[test]
    fn test_table_row_running() {
        let report = make_report("agent-1", HealthStatus::Running);
        let row = report.table_row(false);
        assert!(row.contains("●"));
        assert!(row.contains("agent-1"));
        assert!(row.contains("Running"));
        assert!(row.contains("1h2m"));
        assert!(row.contains("512.0MiB"));
    }

    #[test]
    fn test_table_row_wide() {
        let report = make_report("agent-1", HealthStatus::Running);
        let row = report.table_row(true);
        assert!(row.contains("1.0.0"));
        assert!(row.contains("1.2M"));
    }

    #[test]
    fn test_table_header() {
        let header = AgentStatusReport::table_header(false);
        assert!(header.contains("AGENT"));
        assert!(header.contains("STATUS"));
        assert!(header.contains("NODE"));
        let wide_header = AgentStatusReport::table_header(true);
        assert!(wide_header.contains("TOKENS"));
        assert!(wide_header.contains("VERSION"));
    }

    #[test]
    fn test_status_emoji() {
        let running = make_report("a1", HealthStatus::Running);
        assert!(running.table_row(false).contains("●"));

        let failed = make_report("a2", HealthStatus::Failed);
        assert!(failed.table_row(false).contains("✖"));

        let pending = make_report("a3", HealthStatus::Pending);
        assert!(pending.table_row(false).contains("◌"));
    }

    #[test]
    fn test_print_status_table() {
        let reports = vec![
            make_report("agent-1", HealthStatus::Running),
            make_report("agent-2", HealthStatus::Degraded),
        ];
        // Just verify it doesn't panic
        print_status_table(&reports, false);
        print_status_table(&reports, true);
    }

    #[test]
    fn test_status_json() {
        let reports = vec![make_report("agent-1", HealthStatus::Running)];
        // Verify JSON serialization doesn't panic
        let json = serde_json::to_string(&reports).unwrap();
        assert!(json.contains("agent-1"));
    }
}
