use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Resource usage snapshot for an agent at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_limit_bytes: u64,
    pub tokens_used: u64,
    pub tokens_limit: u64,
    pub active_tasks: u32,
    pub queued_tasks: u32,
    pub network_bytes_in: u64,
    pub network_bytes_out: u64,
}

impl ResourceSnapshot {
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            cpu_usage_percent: 0.0,
            memory_usage_bytes: 0,
            memory_limit_bytes: 1024 * 1024 * 1024, // 1GB default
            tokens_used: 0,
            tokens_limit: 100_000,
            active_tasks: 0,
            queued_tasks: 0,
            network_bytes_in: 0,
            network_bytes_out: 0,
        }
    }

    pub fn memory_usage_percent(&self) -> f64 {
        if self.memory_limit_bytes == 0 {
            0.0
        } else {
            self.memory_usage_bytes as f64 / self.memory_limit_bytes as f64 * 100.0
        }
    }

    pub fn token_usage_percent(&self) -> f64 {
        if self.tokens_limit == 0 {
            0.0
        } else {
            self.tokens_used as f64 / self.tokens_limit as f64 * 100.0
        }
    }

    pub fn is_cpu_critical(&self) -> bool {
        self.cpu_usage_percent > 90.0
    }
    pub fn is_memory_critical(&self) -> bool {
        self.memory_usage_percent() > 85.0
    }
    pub fn is_token_critical(&self) -> bool {
        self.token_usage_percent() > 90.0
    }

    pub fn resource_pressure_score(&self) -> f64 {
        let cpu = self.cpu_usage_percent / 100.0;
        let mem = self.memory_usage_percent() / 100.0;
        let tok = self.token_usage_percent() / 100.0;
        (cpu * 0.4 + mem * 0.3 + tok * 0.3).min(1.0)
    }
}

/// Tracks resource usage history for agents
pub struct ResourceTracker {
    history: HashMap<String, Vec<ResourceSnapshot>>,
    max_history_per_agent: usize,
}

impl Default for ResourceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceTracker {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            max_history_per_agent: 1000,
        }
    }

    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            history: HashMap::new(),
            max_history_per_agent: max_history,
        }
    }

    pub fn record(&mut self, snapshot: ResourceSnapshot) {
        let agent_history = self.history.entry(snapshot.agent_id.clone()).or_default();
        agent_history.push(snapshot);
        if agent_history.len() > self.max_history_per_agent {
            let drain = agent_history.len() - self.max_history_per_agent;
            agent_history.drain(0..drain);
        }
    }

    pub fn latest(&self, agent_id: &str) -> Option<&ResourceSnapshot> {
        self.history.get(agent_id)?.last()
    }

    pub fn history(&self, agent_id: &str) -> &[ResourceSnapshot] {
        self.history.get(agent_id).map_or(&[], |h| h.as_slice())
    }

    pub fn average_cpu(&self, agent_id: &str) -> f64 {
        let h = match self.history.get(agent_id) {
            Some(h) if !h.is_empty() => h,
            _ => return 0.0,
        };
        h.iter().map(|s| s.cpu_usage_percent).sum::<f64>() / h.len() as f64
    }

    pub fn average_memory(&self, agent_id: &str) -> f64 {
        let h = match self.history.get(agent_id) {
            Some(h) if !h.is_empty() => h,
            _ => return 0.0,
        };
        h.iter().map(|s| s.memory_usage_percent()).sum::<f64>() / h.len() as f64
    }

    pub fn peak_cpu(&self, agent_id: &str) -> f64 {
        self.history.get(agent_id).map_or(0.0, |h| {
            h.iter().map(|s| s.cpu_usage_percent).fold(0.0f64, f64::max)
        })
    }

    pub fn peak_memory_bytes(&self, agent_id: &str) -> u64 {
        self.history.get(agent_id).map_or(0, |h| {
            h.iter().map(|s| s.memory_usage_bytes).max().unwrap_or(0)
        })
    }

    pub fn total_tokens_used(&self, agent_id: &str) -> u64 {
        self.history
            .get(agent_id)
            .map_or(0, |h| h.iter().map(|s| s.tokens_used).max().unwrap_or(0))
    }

    pub fn agent_ids(&self) -> Vec<String> {
        self.history.keys().cloned().collect()
    }

    pub fn agents_over_pressure(&self, threshold: f64) -> Vec<String> {
        self.history
            .iter()
            .filter_map(|(id, h)| {
                h.last()
                    .filter(|s| s.resource_pressure_score() > threshold)
                    .map(|_| id.clone())
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn clear_agent(&mut self, agent_id: &str) {
        self.history.remove(agent_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(agent_id: &str, cpu: f64, mem: u64, tokens: u64) -> ResourceSnapshot {
        ResourceSnapshot {
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            cpu_usage_percent: cpu,
            memory_usage_bytes: mem,
            memory_limit_bytes: 1024 * 1024 * 1024,
            tokens_used: tokens,
            tokens_limit: 100_000,
            active_tasks: 0,
            queued_tasks: 0,
            network_bytes_in: 0,
            network_bytes_out: 0,
        }
    }

    #[test]
    fn test_resource_snapshot_new() {
        let snap = ResourceSnapshot::new("a1");
        assert_eq!(snap.agent_id, "a1");
        assert!((snap.cpu_usage_percent - 0.0).abs() < f64::EPSILON);
        assert_eq!(snap.memory_limit_bytes, 1024 * 1024 * 1024);
    }

    #[test]
    fn test_memory_usage_percent() {
        let snap = make_snapshot("a1", 0.0, 512 * 1024 * 1024, 0);
        assert!((snap.memory_usage_percent() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_token_usage_percent() {
        let snap = make_snapshot("a1", 0.0, 0, 80_000);
        assert!((snap.token_usage_percent() - 80.0).abs() < 0.1);
    }

    #[test]
    fn test_critical_thresholds() {
        let snap = make_snapshot("a1", 95.0, 900 * 1024 * 1024, 95_000);
        assert!(snap.is_cpu_critical());
        assert!(snap.is_memory_critical());
        assert!(snap.is_token_critical());
    }

    #[test]
    fn test_resource_pressure_score() {
        let low = make_snapshot("a1", 10.0, 100 * 1024 * 1024, 10_000);
        let high = make_snapshot("a2", 95.0, 900 * 1024 * 1024, 95_000);
        assert!(low.resource_pressure_score() < high.resource_pressure_score());
    }

    #[test]
    fn test_resource_tracker_record() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 50.0, 500_000, 50_000));
        tracker.record(make_snapshot("a1", 60.0, 600_000, 60_000));
        assert_eq!(tracker.history("a1").len(), 2);
    }

    #[test]
    fn test_resource_tracker_latest() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 50.0, 500_000, 50_000));
        tracker.record(make_snapshot("a1", 70.0, 700_000, 70_000));
        let latest = tracker.latest("a1").unwrap();
        assert!((latest.cpu_usage_percent - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_tracker_max_history() {
        let mut tracker = ResourceTracker::with_max_history(3);
        for i in 0..5 {
            tracker.record(make_snapshot("a1", i as f64 * 10.0, 100_000, 10_000));
        }
        assert_eq!(tracker.history("a1").len(), 3);
    }

    #[test]
    fn test_resource_tracker_averages() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 20.0, 200_000, 20_000));
        tracker.record(make_snapshot("a1", 40.0, 400_000, 40_000));
        assert!((tracker.average_cpu("a1") - 30.0).abs() < 0.1);
    }

    #[test]
    fn test_resource_tracker_peak() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 20.0, 200_000, 20_000));
        tracker.record(make_snapshot("a1", 80.0, 800_000, 80_000));
        tracker.record(make_snapshot("a1", 50.0, 500_000, 50_000));
        assert!((tracker.peak_cpu("a1") - 80.0).abs() < f64::EPSILON);
        assert_eq!(tracker.peak_memory_bytes("a1"), 800_000);
    }

    #[test]
    fn test_agents_over_pressure() {
        let mut tracker = ResourceTracker::new();
        let mut low = make_snapshot("a1", 10.0, 100_000, 10_000);
        low.memory_limit_bytes = 1_000_000;
        tracker.record(low);
        let mut high = make_snapshot("a2", 95.0, 950_000, 95_000);
        high.memory_limit_bytes = 1_000_000;
        tracker.record(high);
        let over = tracker.agents_over_pressure(0.8);
        assert_eq!(over.len(), 1);
        assert_eq!(over[0], "a2");
    }

    #[test]
    fn test_agent_ids() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 10.0, 100_000, 10_000));
        tracker.record(make_snapshot("a2", 20.0, 200_000, 20_000));
        let ids = tracker.agent_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_clear_agent() {
        let mut tracker = ResourceTracker::new();
        tracker.record(make_snapshot("a1", 10.0, 100_000, 10_000));
        tracker.record(make_snapshot("a2", 20.0, 200_000, 20_000));
        tracker.clear_agent("a1");
        assert_eq!(tracker.agent_ids().len(), 1);
    }
}
