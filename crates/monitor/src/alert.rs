use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;

/// Alert severity level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

/// Alert state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertState {
    Pending,
    Firing,
    Resolved,
    Silenced,
}

/// Alert rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub metric_name: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub duration_seconds: u64,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub enabled: bool,
}

/// Alert condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    GreaterThan(f64),
    LessThan(f64),
    Equal(f64),
    NotEqual(f64),
    GreaterThanPercentile { percentile: f64, threshold: f64 },
    RateIncrease { threshold: f64, window_seconds: u64 },
}

impl AlertCondition {
    pub fn evaluate(&self, value: f64) -> bool {
        match self {
            AlertCondition::GreaterThan(threshold) => value > *threshold,
            AlertCondition::LessThan(threshold) => value < *threshold,
            AlertCondition::Equal(target) => (value - target).abs() < f64::EPSILON,
            AlertCondition::NotEqual(target) => (value - target).abs() > f64::EPSILON,
            AlertCondition::GreaterThanPercentile { threshold, .. } => value > *threshold,
            AlertCondition::RateIncrease { threshold, .. } => value > *threshold,
        }
    }
}

/// An active or historical alert instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertInstance {
    pub alert_id: String,
    pub rule_id: String,
    pub rule_name: String,
    pub severity: AlertSeverity,
    pub state: AlertState,
    pub triggered_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub value: f64,
    pub threshold: f64,
    pub labels: HashMap<String, String>,
    pub message: String,
}

impl AlertInstance {
    pub fn is_active(&self) -> bool {
        matches!(self.state, AlertState::Pending | AlertState::Firing)
    }

    pub fn duration(&self) -> Duration {
        let end = self.resolved_at.unwrap_or_else(Utc::now);
        end - self.triggered_at
    }
}

/// Alert manager: evaluates rules against metrics and manages alert lifecycle
pub struct AlertManager {
    rules: Vec<AlertRule>,
    active_alerts: Vec<AlertInstance>,
    alert_history: Vec<AlertInstance>,
    max_history: usize,
    /// Silenced rule IDs
    silenced: HashMap<String, DateTime<Utc>>,
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertManager {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            active_alerts: Vec::new(),
            alert_history: Vec::new(),
            max_history: 10_000,
            silenced: HashMap::new(),
        }
    }

    pub fn with_max_history(max: usize) -> Self {
        Self { max_history: max, ..Default::default() }
    }

    /// Add or update an alert rule
    pub fn add_rule(&mut self, rule: AlertRule) {
        if let Some(existing) = self.rules.iter_mut().find(|r| r.rule_id == rule.rule_id) {
            *existing = rule;
        } else {
            self.rules.push(rule);
        }
    }

    /// Remove an alert rule
    pub fn remove_rule(&mut self, rule_id: &str) {
        self.rules.retain(|r| r.rule_id != rule_id);
    }

    /// Evaluate all rules against current metric values
    pub fn evaluate(&mut self, metrics: &HashMap<String, f64>) -> Vec<AlertInstance> {
        let mut new_alerts = Vec::new();
        let mut rule_ids_to_resolve: Vec<String> = Vec::new();

        // Unsilence expired silences
        let now = Utc::now();
        self.silenced.retain(|_, until| now < *until);

        for rule in &self.rules {
            if !rule.enabled { continue; }
            if self.silenced.contains_key(&rule.rule_id) { continue; }

            if let Some(&value) = metrics.get(&rule.metric_name) {
                if rule.condition.evaluate(value) {
                    // Check if already firing
                    let already_active = self.active_alerts.iter()
                        .any(|a| a.rule_id == rule.rule_id && a.is_active());

                    if !already_active {
                        let threshold = match &rule.condition {
                            AlertCondition::GreaterThan(t) | AlertCondition::LessThan(t)
                            | AlertCondition::Equal(t) | AlertCondition::NotEqual(t) => *t,
                            AlertCondition::GreaterThanPercentile { threshold, .. } => *threshold,
                            AlertCondition::RateIncrease { threshold, .. } => *threshold,
                        };

                        let alert = AlertInstance {
                            alert_id: uuid::Uuid::new_v4().to_string(),
                            rule_id: rule.rule_id.clone(),
                            rule_name: rule.name.clone(),
                            severity: rule.severity.clone(),
                            state: AlertState::Firing,
                            triggered_at: Utc::now(),
                            resolved_at: None,
                            value,
                            threshold,
                            labels: rule.labels.clone(),
                            message: format!("{}: {} = {:.2} (threshold: {:.2})", rule.name, rule.metric_name, value, threshold),
                        };

                        self.active_alerts.push(alert.clone());
                        new_alerts.push(alert);
                    }
                } else {
                    // Mark for resolution
                    rule_ids_to_resolve.push(rule.rule_id.clone());
                }
            }
        }

        // Resolve active alerts for rules that no longer fire
        for rule_id in rule_ids_to_resolve {
            let to_resolve: Vec<String> = self.active_alerts.iter()
                .filter(|a| a.rule_id == rule_id)
                .map(|a| a.alert_id.clone())
                .collect();

            for alert_id in to_resolve {
                if let Some(idx) = self.active_alerts.iter().position(|a| a.alert_id == alert_id) {
                    let mut alert = self.active_alerts.remove(idx);
                    alert.state = AlertState::Resolved;
                    alert.resolved_at = Some(Utc::now());
                    self.push_history(alert);
                }
            }
        }

        new_alerts
    }

    /// Silence a rule for a duration
    pub fn silence_rule(&mut self, rule_id: &str, duration: Duration) {
        self.silenced.insert(rule_id.to_string(), Utc::now() + duration);
        // Mark active alerts as silenced
        for alert in &mut self.active_alerts {
            if alert.rule_id == rule_id {
                alert.state = AlertState::Silenced;
            }
        }
    }

    /// Unsilence a rule
    pub fn unsilence_rule(&mut self, rule_id: &str) {
        self.silenced.remove(rule_id);
    }

    /// Get currently active alerts
    pub fn active_alerts(&self) -> &[AlertInstance] {
        &self.active_alerts
    }

    /// Get alert history
    pub fn history(&self) -> &[AlertInstance] {
        &self.alert_history
    }

    /// Get all rules
    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }

    /// Count active alerts by severity
    pub fn active_by_severity(&self) -> HashMap<AlertSeverity, usize> {
        let mut counts = HashMap::new();
        for alert in &self.active_alerts {
            *counts.entry(alert.severity.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Check if there are any critical/emergency active alerts
    pub fn has_critical_alerts(&self) -> bool {
        self.active_alerts.iter().any(|a| {
            matches!(a.severity, AlertSeverity::Critical | AlertSeverity::Emergency)
                && a.is_active()
        })
    }

    /// Get alerts for a specific metric
    pub fn alerts_for_metric(&self, metric_name: &str) -> Vec<&AlertInstance> {
        self.rules.iter()
            .filter(|r| r.metric_name == metric_name)
            .flat_map(|r| {
                self.active_alerts.iter().filter(move |a| a.rule_id == r.rule_id)
            })
            .collect()
    }

    fn push_history(&mut self, alert: AlertInstance) {
        self.alert_history.push(alert);
        if self.alert_history.len() > self.max_history {
            let drain = self.alert_history.len() - self.max_history;
            self.alert_history.drain(0..drain);
        }
    }

    pub fn clear(&mut self) {
        self.active_alerts.clear();
        self.alert_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_high_rule() -> AlertRule {
        AlertRule {
            rule_id: "cpu-high".to_string(),
            name: "CPU High".to_string(),
            description: "CPU usage is too high".to_string(),
            metric_name: "cpu_usage".to_string(),
            condition: AlertCondition::GreaterThan(90.0),
            severity: AlertSeverity::Critical,
            duration_seconds: 0,
            labels: HashMap::new(),
            annotations: HashMap::new(),
            enabled: true,
        }
    }

    fn mem_warning_rule() -> AlertRule {
        AlertRule {
            rule_id: "mem-warning".to_string(),
            name: "Memory Warning".to_string(),
            description: "Memory usage warning".to_string(),
            metric_name: "memory_usage".to_string(),
            condition: AlertCondition::GreaterThan(70.0),
            severity: AlertSeverity::Warning,
            duration_seconds: 0,
            labels: HashMap::new(),
            annotations: HashMap::new(),
            enabled: true,
        }
    }

    #[test]
    fn test_alert_condition_gt() {
        let cond = AlertCondition::GreaterThan(90.0);
        assert!(cond.evaluate(95.0));
        assert!(!cond.evaluate(85.0));
    }

    #[test]
    fn test_alert_condition_lt() {
        let cond = AlertCondition::LessThan(10.0);
        assert!(cond.evaluate(5.0));
        assert!(!cond.evaluate(15.0));
    }

    #[test]
    fn test_alert_manager_evaluate_triggers() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);

        let new_alerts = manager.evaluate(&metrics);
        assert_eq!(new_alerts.len(), 1);
        assert_eq!(new_alerts[0].severity, AlertSeverity::Critical);
        assert_eq!(manager.active_alerts().len(), 1);
    }

    #[test]
    fn test_alert_manager_evaluate_no_trigger() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 50.0);

        let new_alerts = manager.evaluate(&metrics);
        assert_eq!(new_alerts.len(), 0);
        assert_eq!(manager.active_alerts().len(), 0);
    }

    #[test]
    fn test_alert_manager_resolve() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        // Trigger
        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        manager.evaluate(&metrics);
        assert_eq!(manager.active_alerts().len(), 1);

        // Resolve
        metrics.insert("cpu_usage".to_string(), 50.0);
        manager.evaluate(&metrics);
        assert_eq!(manager.active_alerts().len(), 0);
        assert_eq!(manager.history().len(), 1);
        assert_eq!(manager.history()[0].state, AlertState::Resolved);
    }

    #[test]
    fn test_alert_manager_no_duplicate() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);

        manager.evaluate(&metrics);
        manager.evaluate(&metrics); // second evaluation
        assert_eq!(manager.active_alerts().len(), 1); // still only one
    }

    #[test]
    fn test_alert_manager_silence() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        manager.silence_rule("cpu-high", Duration::hours(1));

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        let alerts = manager.evaluate(&metrics);
        assert_eq!(alerts.len(), 0);
    }

    #[test]
    fn test_alert_manager_multiple_rules() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());
        manager.add_rule(mem_warning_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        metrics.insert("memory_usage".to_string(), 80.0);

        let alerts = manager.evaluate(&metrics);
        assert_eq!(alerts.len(), 2);
    }

    #[test]
    fn test_alert_manager_has_critical() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());

        assert!(!manager.has_critical_alerts());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        manager.evaluate(&metrics);

        assert!(manager.has_critical_alerts());
    }

    #[test]
    fn test_active_by_severity() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());
        manager.add_rule(mem_warning_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        metrics.insert("memory_usage".to_string(), 80.0);
        manager.evaluate(&metrics);

        let counts = manager.active_by_severity();
        assert_eq!(counts.get(&AlertSeverity::Critical), Some(&1));
        assert_eq!(counts.get(&AlertSeverity::Warning), Some(&1));
    }

    #[test]
    fn test_alert_instance_duration() {
        let alert = AlertInstance {
            alert_id: "a1".to_string(),
            rule_id: "r1".to_string(),
            rule_name: "test".to_string(),
            severity: AlertSeverity::Info,
            state: AlertState::Firing,
            triggered_at: Utc::now() - Duration::minutes(30),
            resolved_at: None,
            value: 100.0,
            threshold: 90.0,
            labels: HashMap::new(),
            message: "test".to_string(),
        };
        assert!(alert.duration().num_minutes() >= 29);
        assert!(alert.is_active());
    }

    #[test]
    fn test_alert_manager_remove_rule() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());
        assert_eq!(manager.rules().len(), 1);
        manager.remove_rule("cpu-high");
        assert_eq!(manager.rules().len(), 0);
    }

    #[test]
    fn test_alert_manager_disabled_rule() {
        let mut manager = AlertManager::new();
        let mut rule = cpu_high_rule();
        rule.enabled = false;
        manager.add_rule(rule);

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        let alerts = manager.evaluate(&metrics);
        assert_eq!(alerts.len(), 0);
    }

    #[test]
    fn test_alerts_for_metric() {
        let mut manager = AlertManager::new();
        manager.add_rule(cpu_high_rule());
        manager.add_rule(mem_warning_rule());

        let mut metrics = HashMap::new();
        metrics.insert("cpu_usage".to_string(), 95.0);
        metrics.insert("memory_usage".to_string(), 80.0);
        manager.evaluate(&metrics);

        let cpu_alerts = manager.alerts_for_metric("cpu_usage");
        assert_eq!(cpu_alerts.len(), 1);
    }
}
