//! Resource monitoring — kubectl-style `top` command.

use serde::{Deserialize, Serialize};

/// Resource snapshot for a single agent at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    /// Agent identifier.
    pub agent_id: String,
    /// CPU usage as fraction of one core (0.0 - N.0 for multi-core).
    pub cpu_cores: f64,
    /// Memory usage in bytes.
    pub memory_bytes: u64,
    /// Memory limit in bytes (0 = unlimited).
    pub memory_limit: u64,
    /// Tokens consumed in current window.
    pub tokens: u64,
    /// Token rate (tokens per second).
    pub token_rate: f64,
    /// Active LLM requests.
    pub active_requests: u32,
    /// Queue depth (pending tasks).
    pub queue_depth: u32,
    /// Network bytes sent.
    pub net_tx_bytes: u64,
    /// Network bytes received.
    pub net_rx_bytes: u64,
}

impl ResourceSnapshot {
    /// Memory usage as percentage of limit.
    pub fn memory_percent(&self) -> f64 {
        if self.memory_limit == 0 {
            0.0
        } else {
            (self.memory_bytes as f64 / self.memory_limit as f64) * 100.0
        }
    }

    /// Format as a table row.
    pub fn table_row(&self, _sort_by: &str) -> String {
        let mem_pct = if self.memory_limit > 0 {
            format!("{:.0}%", self.memory_percent())
        } else {
            "-".to_string()
        };
        format!(
            "{id:<16} {cpu:<10} {mem:<12} {pct:<6} {tok:<10} {rate:<10} {req:<5} {q:<5}",
            id = self.agent_id,
            cpu = format!("{:.2} cores", self.cpu_cores),
            mem = super::status::format_bytes(self.memory_bytes),
            pct = mem_pct,
            tok = super::status::format_number(self.tokens),
            rate = format!("{:.1}/s", self.token_rate),
            req = self.active_requests,
            q = self.queue_depth,
        )
    }
}

/// Print resource monitoring table.
pub fn print_top_table(snapshots: &[ResourceSnapshot], sort_by: &str) {
    let mut sorted = snapshots.to_vec();
    match sort_by {
        "cpu" => sorted.sort_by(|a, b| {
            b.cpu_cores
                .partial_cmp(&a.cpu_cores)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
        "memory" | "mem" => sorted.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
        "tokens" | "tok" => sorted.sort_by(|a, b| b.tokens.cmp(&a.tokens)),
        _ => {}
    }

    // Header
    println!(
        "  {id:<16} {cpu:<10} {mem:<12} {pct:<6} {tok:<10} {rate:<10} {req:<5} {q:<5}",
        id = "AGENT",
        cpu = "CPU",
        mem = "MEMORY",
        pct = "MEM%",
        tok = "TOKENS",
        rate = "TOK/s",
        req = "REQ",
        q = "QUEUE",
    );

    for snap in &sorted {
        println!("{}", snap.table_row(sort_by));
    }

    // Summary
    let total_cpu: f64 = sorted.iter().map(|s| s.cpu_cores).sum();
    let total_mem: u64 = sorted.iter().map(|s| s.memory_bytes).sum();
    let total_tok: u64 = sorted.iter().map(|s| s.tokens).sum();
    println!(
        "\n  TOTAL: {:.2} cores, {} memory, {} tokens",
        total_cpu,
        super::status::format_bytes(total_mem),
        super::status::format_number(total_tok),
    );
}

/// Generate a simple ASCII sparkline for a value history.
pub fn sparkline(values: &[f64]) -> String {
    let chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    if values.is_empty() {
        return String::new();
    }
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let range = max - min;

    values
        .iter()
        .map(|v| {
            if range == 0.0 {
                chars[4]
            } else {
                let idx = ((v - min) / range * 7.0) as usize;
                chars[idx.min(7)]
            }
        })
        .collect()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(id: &str, cpu: f64, mem: u64) -> ResourceSnapshot {
        ResourceSnapshot {
            agent_id: id.to_string(),
            cpu_cores: cpu,
            memory_bytes: mem,
            memory_limit: 1024 * 1024 * 1024, // 1 GiB
            tokens: 50000,
            token_rate: 12.5,
            active_requests: 2,
            queue_depth: 5,
            net_tx_bytes: 1024,
            net_rx_bytes: 2048,
        }
    }

    #[test]
    fn test_memory_percent() {
        let snap = make_snapshot("a1", 0.5, 512 * 1024 * 1024);
        assert!((snap.memory_percent() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_memory_percent_no_limit() {
        let mut snap = make_snapshot("a1", 0.5, 512 * 1024 * 1024);
        snap.memory_limit = 0;
        assert_eq!(snap.memory_percent(), 0.0);
    }

    #[test]
    fn test_table_row() {
        let snap = make_snapshot("agent-1", 1.5, 512 * 1024 * 1024);
        let row = snap.table_row("cpu");
        assert!(row.contains("agent-1"));
        assert!(row.contains("1.50 cores"));
        assert!(row.contains("512.0MiB"));
        assert!(row.contains("50%"));
    }

    #[test]
    fn test_print_top_table() {
        let snapshots = vec![
            make_snapshot("agent-1", 2.0, 512 * 1024 * 1024),
            make_snapshot("agent-2", 0.5, 128 * 1024 * 1024),
        ];
        // Just verify it doesn't panic
        print_top_table(&snapshots, "cpu");
        print_top_table(&snapshots, "memory");
        print_top_table(&snapshots, "tokens");
    }

    #[test]
    fn test_sparkline() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let spark = sparkline(&values);
        assert_eq!(spark.chars().count(), 8);
        assert_eq!(spark, "▁▂▃▄▅▆▇█");
    }

    #[test]
    fn test_sparkline_flat() {
        let values = vec![5.0, 5.0, 5.0, 5.0];
        let spark = sparkline(&values);
        assert_eq!(spark, "▅▅▅▅"); // flat values map to index 4
    }

    #[test]
    fn test_sparkline_empty() {
        assert_eq!(sparkline(&[]), "");
    }

    #[test]
    fn test_sparkline_single() {
        let spark = sparkline(&[42.0]);
        assert_eq!(spark.chars().count(), 1);
    }
}
