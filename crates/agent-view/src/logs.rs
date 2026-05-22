//! Log streaming and display — kubectl-style log viewer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of the log entry.
    pub timestamp: DateTime<Utc>,
    /// Log level.
    pub level: LogLevel,
    /// Component/module that produced the log.
    pub component: String,
    /// Agent ID (if applicable).
    pub agent_id: Option<String>,
    /// Session ID (if applicable).
    pub session_id: Option<String>,
    /// Log message.
    pub message: String,
    /// Additional structured fields.
    #[serde(default)]
    pub fields: std::collections::HashMap<String, serde_json::Value>,
}

/// Log level with ordering for filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace => write!(f, "TRACE"),
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO "),
            Self::Warn => write!(f, "WARN "),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

impl LogLevel {
    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" | "err" => Some(Self::Error),
            _ => None,
        }
    }
}

/// Configuration for log display/filtering.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Number of recent lines to show.
    pub tail: usize,
    /// Minimum log level to display.
    pub min_level: Option<LogLevel>,
    /// Filter by component name.
    pub component: Option<String>,
    /// Whether to follow (stream) new entries.
    pub follow: bool,
    /// Use color output.
    pub color: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            tail: 100,
            min_level: None,
            component: None,
            follow: false,
            color: true,
        }
    }
}

/// Format a log entry for terminal display.
pub fn format_log_entry(entry: &LogEntry, color: bool) -> String {
    let ts = entry.timestamp.format("%H:%M:%S%.3f");
    let level_str = entry.level.to_string();

    if color {
        let level_colored = match entry.level {
            LogLevel::Error => format!("\x1b[31m{level_str}\x1b[0m"), // red
            LogLevel::Warn => format!("\x1b[33m{level_str}\x1b[0m"),  // yellow
            LogLevel::Info => format!("\x1b[32m{level_str}\x1b[0m"),  // green
            LogLevel::Debug => format!("\x1b[36m{level_str}\x1b[0m"), // cyan
            LogLevel::Trace => format!("\x1b[90m{level_str}\x1b[0m"), // gray
        };
        format!(
            "\x1b[90m{ts}\x1b[0m {level} \x1b[34m{comp}\x1b[0m {msg}",
            ts = ts,
            level = level_colored,
            comp = entry.component,
            msg = entry.message,
        )
    } else {
        format!(
            "{ts} {level} {comp} {msg}",
            ts = ts,
            level = level_str,
            comp = entry.component,
            msg = entry.message,
        )
    }
}

/// Filter log entries based on configuration.
pub fn filter_logs<'a>(entries: &'a [LogEntry], config: &'a LogConfig) -> Vec<&'a LogEntry> {
    entries
        .iter()
        .filter(|e| {
            if let Some(ref min_level) = config.min_level {
                if e.level < *min_level {
                    return false;
                }
            }
            if let Some(ref component) = config.component {
                if !e.component.contains(component.as_str()) {
                    return false;
                }
            }
            true
        })
        .collect()
}

/// Display log entries (tail + filter).
pub fn display_logs(entries: &[LogEntry], config: &LogConfig) {
    let filtered = filter_logs(entries, config);
    let start = if filtered.len() > config.tail {
        filtered.len() - config.tail
    } else {
        0
    };

    for entry in &filtered[start..] {
        println!("{}", format_log_entry(entry, config.color));
    }

    if filtered.len() > config.tail {
        eprintln!(
            "\n... showing last {} of {} entries (use -n to see more)",
            config.tail,
            filtered.len()
        );
    }
}

/// Simulate following logs (in real impl, this would be a streaming connection).
pub fn follow_logs_hint(agent_id: &str) {
    eprintln!("Following logs for {agent_id}... (Ctrl+C to stop)");
    eprintln!("(In production, this connects to the agent's log stream via gRPC/WebSocket)");
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_entry(level: LogLevel, comp: &str, msg: &str) -> LogEntry {
        LogEntry {
            timestamp: Utc::now(),
            level,
            component: comp.to_string(),
            agent_id: Some("agent-1".to_string()),
            session_id: None,
            message: msg.to_string(),
            fields: HashMap::new(),
        }
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "TRACE");
        assert_eq!(LogLevel::Info.to_string(), "INFO ");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn test_log_level_parse() {
        assert_eq!(LogLevel::from_str_loose("info"), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_str_loose("WARN"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str_loose("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_str_loose("err"), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_str_loose("bogus"), None);
    }

    #[test]
    fn test_format_log_entry_no_color() {
        let entry = make_entry(LogLevel::Info, "scheduler", "Agent started");
        let formatted = format_log_entry(&entry, false);
        assert!(formatted.contains("INFO "));
        assert!(formatted.contains("scheduler"));
        assert!(formatted.contains("Agent started"));
    }

    #[test]
    fn test_format_log_entry_color() {
        let entry = make_entry(LogLevel::Error, "api", "Connection failed");
        let formatted = format_log_entry(&entry, true);
        assert!(formatted.contains("\x1b[31m")); // red for error
        assert!(formatted.contains("Connection failed"));
    }

    #[test]
    fn test_filter_by_level() {
        let entries = vec![
            make_entry(LogLevel::Debug, "a", "debug msg"),
            make_entry(LogLevel::Info, "a", "info msg"),
            make_entry(LogLevel::Error, "a", "error msg"),
        ];
        let config = LogConfig {
            min_level: Some(LogLevel::Info),
            ..Default::default()
        };
        let filtered = filter_logs(&entries, &config);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_component() {
        let entries = vec![
            make_entry(LogLevel::Info, "scheduler", "msg1"),
            make_entry(LogLevel::Info, "controller", "msg2"),
            make_entry(LogLevel::Info, "scheduler", "msg3"),
        ];
        let config = LogConfig {
            component: Some("sched".to_string()),
            ..Default::default()
        };
        let filtered = filter_logs(&entries, &config);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_combined() {
        let entries = vec![
            make_entry(LogLevel::Debug, "scheduler", "d1"),
            make_entry(LogLevel::Info, "scheduler", "i1"),
            make_entry(LogLevel::Info, "controller", "i2"),
            make_entry(LogLevel::Error, "scheduler", "e1"),
        ];
        let config = LogConfig {
            min_level: Some(LogLevel::Info),
            component: Some("scheduler".to_string()),
            ..Default::default()
        };
        let filtered = filter_logs(&entries, &config);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_tail_limit() {
        let entries: Vec<LogEntry> = (0..200)
            .map(|i| make_entry(LogLevel::Info, "test", &format!("msg {i}")))
            .collect();
        let config = LogConfig {
            tail: 50,
            ..Default::default()
        };
        // display_logs should only print last 50
        display_logs(&entries, &config);
    }

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();
        assert_eq!(config.tail, 100);
        assert!(!config.follow);
        assert!(config.color);
        assert!(config.min_level.is_none());
    }
}
