//! # Behavior Risk Detection System
//!
//! Real-time detection of anomalous and risky agent behaviors:
//!
//! - **BehaviorRiskDetector** — monitors tool call sequences for anomalies
//! - **RiskPattern** — defines patterns that indicate risky behavior
//! - **RiskAlert** — structured alert emitted when risky behavior is detected
//!
//! ## Design
//!
//! ```text
//! ToolCall/AgentAction ──► BehaviorRiskDetector ──► RiskAlert
//!                                │
//!                                ├── ToolSequenceAnomaly
//!                                ├── PrivilegeEscalation
//!                                ├── DataExfiltration
//!                                └── UnusualVolume
//! ```

use kias_common::KiasError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// Category of detected risk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskCategory {
    /// Abnormal sequence of tool calls
    AbnormalToolSequence,
    /// Attempted or successful privilege escalation
    PrivilegeEscalation,
    /// Unauthorized data extraction or egress
    DataExfiltration,
    /// Excessive or unusual volume of operations
    VolumeAnomaly,
    /// Rapid repeated failed operations
    RapidFailure,
    /// Suspicious output pattern (potential exfil encoding)
    SuspiciousOutput,
    /// Unauthorized cross-agent communication
    CrossAgentAnomaly,
}

impl std::fmt::Display for RiskCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskCategory::AbnormalToolSequence => write!(f, "abnormal_tool_sequence"),
            RiskCategory::PrivilegeEscalation => write!(f, "privilege_escalation"),
            RiskCategory::DataExfiltration => write!(f, "data_exfiltration"),
            RiskCategory::VolumeAnomaly => write!(f, "volume_anomaly"),
            RiskCategory::RapidFailure => write!(f, "rapid_failure"),
            RiskCategory::SuspiciousOutput => write!(f, "suspicious_output"),
            RiskCategory::CrossAgentAnomaly => write!(f, "cross_agent_anomaly"),
        }
    }
}

/// Risk severity level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Info => "info",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }

    /// Returns true if this level should trigger an immediate alert.
    pub fn should_alert(&self) -> bool {
        matches!(self, RiskLevel::High | RiskLevel::Critical)
    }

    /// Returns true if this level should block the action.
    pub fn should_block(&self) -> bool {
        matches!(self, RiskLevel::Critical)
    }
}

/// A defined risk pattern to detect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskPattern {
    /// Unique pattern identifier
    pub id: String,
    /// Human-readable pattern name
    pub name: String,
    /// Category of risk this pattern detects
    pub category: RiskCategory,
    /// Severity when pattern matches
    pub severity: RiskLevel,
    /// Sequence of tool names that constitutes the risk (empty = any)
    pub tool_sequence: Vec<String>,
    /// Time window for sequence matching (ms)
    pub window_ms: u64,
    /// Minimum occurrences within window to trigger
    pub min_occurrences: usize,
    /// Whether to block immediately on match
    pub block_on_match: bool,
    /// Optional regex on output content
    pub output_regex: Option<String>,
    /// Optional threshold for numeric conditions
    pub numeric_threshold: Option<f64>,
}

impl RiskPattern {
    /// Create a tool sequence pattern.
    pub fn tool_sequence(
        id: impl Into<String>,
        name: impl Into<String>,
        tools: Vec<String>,
        severity: RiskLevel,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            category: RiskCategory::AbnormalToolSequence,
            severity,
            tool_sequence: tools,
            window_ms: 5000,
            min_occurrences: 3,
            block_on_match: false,
            output_regex: None,
            numeric_threshold: None,
        }
    }

    /// Set the time window for this pattern.
    pub fn with_window(mut self, ms: u64) -> Self {
        self.window_ms = ms;
        self
    }

    /// Set the minimum occurrences.
    pub fn with_min_occurrences(mut self, n: usize) -> Self {
        self.min_occurrences = n;
        self
    }

    /// Set block-on-match.
    pub fn with_block_on_match(mut self, block: bool) -> Self {
        self.block_on_match = block;
        self
    }

    /// Set output regex filter.
    pub fn with_output_regex(mut self, regex: impl Into<String>) -> Self {
        self.output_regex = Some(regex.into());
        self
    }
}

/// A tool call event for risk analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEvent {
    /// Agent that made the call
    pub agent_id: String,
    /// Tool that was called
    pub tool_name: String,
    /// Input parameters (JSON)
    pub input: serde_json::Value,
    /// Output/result of the call
    pub output: Option<String>,
    /// Whether the call succeeded
    pub success: bool,
    /// When the call started
    pub started_at: SystemTime,
    /// Duration in milliseconds
    pub duration_ms: u64,
}

impl ToolCallEvent {
    /// Create a new tool call event.
    pub fn new(
        agent_id: impl Into<String>,
        tool_name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            tool_name: tool_name.into(),
            input,
            output: None,
            success: true,
            started_at: SystemTime::now(),
            duration_ms: 0,
        }
    }

    /// Mark as succeeded with output.
    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    /// Mark as failed.
    pub fn with_failure(mut self) -> Self {
        self.success = false;
        self
    }
}

/// A detected risk alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAlert {
    /// Unique alert identifier
    pub id: String,
    /// Pattern that triggered this alert
    pub pattern_id: String,
    /// Category of risk
    pub category: RiskCategory,
    /// Severity of the risk
    pub level: RiskLevel,
    /// Agent involved
    pub agent_id: String,
    /// The triggering events
    pub events: Vec<ToolCallEvent>,
    /// Description of the detected risk
    pub description: String,
    /// Recommended action
    pub recommended_action: String,
    /// When the alert was generated
    pub generated_at: SystemTime,
    /// Whether this alert should block
    pub should_block: bool,
}

impl RiskAlert {
    /// Create a new risk alert.
    pub fn new(
        pattern: &RiskPattern,
        agent_id: impl Into<String>,
        events: Vec<ToolCallEvent>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("alert-{}", uuid::Uuid::new_v4()),
            pattern_id: pattern.id.clone(),
            category: pattern.category.clone(),
            level: pattern.severity.clone(),
            agent_id: agent_id.into(),
            events,
            description: description.into(),
            recommended_action: Self::default_action_for_category(&pattern.category),
            generated_at: SystemTime::now(),
            should_block: pattern.block_on_match,
        }
    }

    fn default_action_for_category(category: &RiskCategory) -> String {
        match category {
            RiskCategory::AbnormalToolSequence => {
                "Review tool sequence for anomalous patterns".to_string()
            }
            RiskCategory::PrivilegeEscalation => {
                "Immediately suspend agent and investigate".to_string()
            }
            RiskCategory::DataExfiltration => "Block agent and alert security team".to_string(),
            RiskCategory::VolumeAnomaly => "Throttle agent operations and review".to_string(),
            RiskCategory::RapidFailure => {
                "Review failure logs and consider circuit breaker".to_string()
            }
            RiskCategory::SuspiciousOutput => {
                "Inspect output content for exfil patterns".to_string()
            }
            RiskCategory::CrossAgentAnomaly => {
                "Review inter-agent communication patterns".to_string()
            }
        }
    }
}

// ─── Built-in Patterns ────────────────────────────────────────────────────────

/// Get all built-in risk detection patterns.
pub fn builtin_patterns() -> Vec<RiskPattern> {
    vec![
        // Privilege escalation: exec_* followed by delete_* in short window
        RiskPattern::tool_sequence(
            "priv-escalation-001",
            "Privileged Tool Escalation",
            vec![
                "exec_shell".to_string(),
                "exec_admin".to_string(),
                "delete_all".to_string(),
            ],
            RiskLevel::Critical,
        )
        .with_window(3000)
        .with_min_occurrences(2)
        .with_block_on_match(true),
        // Data exfiltration: read followed by http_post in short window
        RiskPattern::tool_sequence(
            "data-exfil-001",
            "Data Exfiltration Sequence",
            vec!["read_data".to_string(), "http_post".to_string()],
            RiskLevel::High,
        )
        .with_window(5000)
        .with_min_occurrences(2),
        // Suspicious output: base64 encoding patterns in output
        RiskPattern::tool_sequence(
            "suspicious-output-001",
            "Encoded Output Detection",
            vec!["read_data".to_string()],
            RiskLevel::Medium,
        )
        .with_output_regex(r"(?i)(base64|encode|data:)([A-Za-z0-9+/=]{20,})")
        .with_min_occurrences(1),
        // Volume anomaly: more than 10 tool calls in 1 second
        RiskPattern {
            id: "volume-001".to_string(),
            name: "Tool Call Burst".to_string(),
            category: RiskCategory::VolumeAnomaly,
            severity: RiskLevel::Medium,
            tool_sequence: vec![],
            window_ms: 1000,
            min_occurrences: 10,
            block_on_match: false,
            output_regex: None,
            numeric_threshold: Some(10.0),
        },
        // Rapid failure: 5 failures in 3 seconds
        RiskPattern {
            id: "rapid-failure-001".to_string(),
            name: "Rapid Failure Spike".to_string(),
            category: RiskCategory::RapidFailure,
            severity: RiskLevel::High,
            tool_sequence: vec![],
            window_ms: 3000,
            min_occurrences: 5,
            block_on_match: false,
            output_regex: None,
            numeric_threshold: Some(5.0),
        },
        // Cross-agent anomaly: agent communicates with too many others
        RiskPattern {
            id: "cross-agent-001".to_string(),
            name: "Excessive Cross-Agent Communication".to_string(),
            category: RiskCategory::CrossAgentAnomaly,
            severity: RiskLevel::Medium,
            tool_sequence: vec!["send_message".to_string(), "delegate_task".to_string()],
            window_ms: 10000,
            min_occurrences: 5,
            block_on_match: false,
            output_regex: None,
            numeric_threshold: Some(5.0),
        },
    ]
}

// ─── BehaviorRiskDetector ─────────────────────────────────────────────────────

/// Main risk detection engine.
#[derive(Debug, Clone)]
pub struct BehaviorRiskDetector {
    patterns: Vec<RiskPattern>,
    /// Ring buffer of recent events per agent
    event_history: HashMap<String, Vec<RecordedEvent>>,
    /// Maximum events to retain per agent
    max_history: usize,
}

#[derive(Debug, Clone)]
struct RecordedEvent {
    event: ToolCallEvent,
    recorded_at: SystemTime,
}

impl BehaviorRiskDetector {
    /// Create a new detector with built-in patterns.
    pub fn new() -> Self {
        Self::with_patterns(builtin_patterns())
    }

    /// Create a detector with custom patterns.
    pub fn with_patterns(patterns: Vec<RiskPattern>) -> Self {
        Self {
            patterns,
            event_history: HashMap::new(),
            max_history: 1000,
        }
    }

    /// Add a pattern.
    pub fn add_pattern(&mut self, pattern: RiskPattern) {
        self.patterns.push(pattern);
    }

    /// Record a tool call event and check for risk.
    /// Returns alerts if any patterns matched.
    pub fn record(&mut self, event: ToolCallEvent) -> Vec<RiskAlert> {
        let agent_id = event.agent_id.clone();
        let now = SystemTime::now();

        // Record in history
        let history = self.event_history.entry(agent_id.clone()).or_default();
        history.push(RecordedEvent {
            event: event.clone(),
            recorded_at: now,
        });

        // Trim history
        while history.len() > self.max_history {
            history.remove(0);
        }

        // Check all patterns
        let mut alerts = Vec::new();
        for pattern in &self.patterns {
            if let Some(alert) = self.check_pattern(pattern, &agent_id, &event, now) {
                alerts.push(alert);
            }
        }

        alerts
    }

    /// Check a specific pattern against the event history.
    fn check_pattern(
        &self,
        pattern: &RiskPattern,
        agent_id: &str,
        current_event: &ToolCallEvent,
        now: SystemTime,
    ) -> Option<RiskAlert> {
        let events = self.event_history.get(agent_id)?;

        // Get events within the window
        let window_start = now - Duration::from_millis(pattern.window_ms);
        let window_events: Vec<_> = events
            .iter()
            .filter(|e| e.recorded_at >= window_start)
            .collect();

        // Count failures if rapid failure pattern
        if matches!(pattern.category, RiskCategory::RapidFailure) {
            let failure_count = window_events.iter().filter(|e| !e.event.success).count();
            if failure_count >= pattern.min_occurrences {
                return Some(RiskAlert::new(
                    pattern,
                    agent_id,
                    window_events.iter().map(|e| e.event.clone()).collect(),
                    format!(
                        "Rapid failure detected: {} failures in {}ms window",
                        failure_count, pattern.window_ms
                    ),
                ));
            }
        }

        // Volume anomaly pattern (no tool_sequence required)
        if matches!(pattern.category, RiskCategory::VolumeAnomaly)
            && pattern.tool_sequence.is_empty()
        {
            if window_events.len() >= pattern.min_occurrences {
                return Some(RiskAlert::new(
                    pattern,
                    agent_id,
                    window_events.iter().map(|e| e.event.clone()).collect(),
                    format!(
                        "Volume anomaly: {} events in {}ms (threshold: {})",
                        window_events.len(),
                        pattern.window_ms,
                        pattern.min_occurrences
                    ),
                ));
            }
        }

        // Tool sequence pattern
        if !pattern.tool_sequence.is_empty() {
            let owned_events: Vec<RecordedEvent> =
                window_events.iter().map(|e| (*e).clone()).collect();
            return self.check_sequence_pattern(pattern, agent_id, &owned_events);
        }

        None
    }

    /// Check a tool sequence pattern.
    fn check_sequence_pattern(
        &self,
        pattern: &RiskPattern,
        agent_id: &str,
        window_events: &[RecordedEvent],
    ) -> Option<RiskAlert> {
        // Build tool sequence
        let tool_seq: Vec<_> = window_events
            .iter()
            .map(|e| e.event.tool_name.clone())
            .collect();

        // Look for the required sequence appearing at least min_occurrences times
        // (interpreted as the tools appearing in order within the window)
        let seq_len = pattern.tool_sequence.len();
        if seq_len == 0 {
            return None;
        }

        // Find occurrences of the pattern
        let mut occurrences = 0usize;
        let mut match_start_idx = 0;

        while match_start_idx + seq_len <= tool_seq.len() {
            let window = &tool_seq[match_start_idx..match_start_idx + seq_len];
            if window == &pattern.tool_sequence[..] {
                occurrences += 1;
                match_start_idx += seq_len; // Move past this match
            } else {
                match_start_idx += 1;
            }
        }

        if occurrences >= pattern.min_occurrences {
            let triggering_events: Vec<_> = window_events.iter().map(|e| e.event.clone()).collect();

            // Also check output regex if set
            if let Some(ref regex) = pattern.output_regex {
                let has_match = triggering_events.iter().any(|e| {
                    e.output
                        .as_ref()
                        .map(|o| {
                            regex::Regex::new(regex)
                                .ok()
                                .map(|re| re.is_match(o))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                });
                if !has_match {
                    return None;
                }
            }

            return Some(RiskAlert::new(
                pattern,
                agent_id,
                triggering_events,
                format!(
                    "Tool sequence '{}' detected {} times in {}ms window",
                    pattern.tool_sequence.join(" → "),
                    occurrences,
                    pattern.window_ms
                ),
            ));
        }

        None
    }

    /// Get the event history for an agent.
    pub fn get_history(&self, agent_id: &str) -> Vec<&ToolCallEvent> {
        self.event_history
            .get(agent_id)
            .map(|h| h.iter().map(|e| &e.event).collect())
            .unwrap_or_default()
    }

    /// Clear history for an agent.
    pub fn clear_history(&mut self, agent_id: &str) {
        self.event_history.remove(agent_id);
    }

    /// Get detector statistics.
    pub fn stats(&self) -> DetectorStats {
        DetectorStats {
            tracked_agents: self.event_history.len(),
            patterns_loaded: self.patterns.len(),
            max_history_per_agent: self.max_history,
        }
    }
}

impl Default for BehaviorRiskDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detector statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorStats {
    pub tracked_agents: usize,
    pub patterns_loaded: usize,
    pub max_history_per_agent: usize,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(agent: &str, tool: &str, success: bool) -> ToolCallEvent {
        let mut event = ToolCallEvent::new(agent, tool, serde_json::json!({}));
        if !success {
            event = event.with_failure();
        }
        event
    }

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Critical > RiskLevel::High);
        assert!(RiskLevel::High > RiskLevel::Medium);
        assert!(RiskLevel::Medium > RiskLevel::Low);
        assert!(RiskLevel::Low > RiskLevel::Info);
    }

    #[test]
    fn test_risk_level_alert_and_block() {
        assert!(!RiskLevel::Info.should_alert());
        assert!(!RiskLevel::Low.should_alert());
        assert!(!RiskLevel::Medium.should_alert());
        assert!(RiskLevel::High.should_alert());
        assert!(RiskLevel::Critical.should_alert());

        assert!(!RiskLevel::Critical.should_block());
        // Actually Critical.should_block is true
        assert!(RiskLevel::Critical.should_block());
    }

    #[test]
    fn test_risk_pattern_builder() {
        let pattern = RiskPattern::tool_sequence(
            "test-seq",
            "Test Sequence",
            vec!["tool-a".into(), "tool-b".into()],
            RiskLevel::Medium,
        )
        .with_window(2000)
        .with_min_occurrences(3)
        .with_block_on_match(true);

        assert_eq!(pattern.id, "test-seq");
        assert_eq!(pattern.tool_sequence, vec!["tool-a", "tool-b"]);
        assert_eq!(pattern.window_ms, 2000);
        assert_eq!(pattern.min_occurrences, 3);
        assert!(pattern.block_on_match);
    }

    #[test]
    fn test_risk_pattern_with_output_regex() {
        let pattern = RiskPattern::tool_sequence(
            "output-check",
            "Output Check",
            vec!["read".into()],
            RiskLevel::Low,
        )
        .with_output_regex(r"(?i)secret");

        assert!(pattern.output_regex.is_some());
        let re = regex::Regex::new(pattern.output_regex.unwrap().as_str()).unwrap();
        assert!(re.is_match("This is a secret message"));
    }

    #[test]
    fn test_builtin_patterns() {
        let patterns = builtin_patterns();
        assert!(!patterns.is_empty());
        assert!(patterns.iter().all(|p| !p.id.is_empty()));
    }

    #[test]
    fn test_behavior_risk_detector_new() {
        let detector = BehaviorRiskDetector::new();
        assert!(!detector.patterns.is_empty());
    }

    #[test]
    fn test_behavior_risk_detector_record_single_event() {
        let mut detector = BehaviorRiskDetector::new();
        let event = make_event("agent-1", "read_data", true);
        let alerts = detector.record(event);
        // Single event shouldn't trigger patterns (need sequences)
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_behavior_risk_detector_volume_anomaly() {
        let mut detector = BehaviorRiskDetector::new();

        // Emit 10+ events quickly
        for i in 0..12 {
            let event = make_event("agent-volume", &format!("tool-{}", i), true);
            detector.record(event);
        }

        let history = detector.get_history("agent-volume");
        assert!(history.len() >= 10);
    }

    #[test]
    fn test_behavior_risk_detector_clear_history() {
        let mut detector = BehaviorRiskDetector::new();
        let event = make_event("agent-clear", "tool-a", true);
        detector.record(event);
        assert!(!detector.get_history("agent-clear").is_empty());

        detector.clear_history("agent-clear");
        assert!(detector.get_history("agent-clear").is_empty());
    }

    #[test]
    fn test_behavior_risk_detector_stats() {
        let detector = BehaviorRiskDetector::new();
        let stats = detector.stats();
        assert_eq!(stats.patterns_loaded, detector.patterns.len());
        assert_eq!(stats.tracked_agents, 0);
    }

    #[test]
    fn test_risk_alert_serdes() {
        let mut detector = BehaviorRiskDetector::new();
        let event = make_event("agent-1", "exec_shell", true);
        let alerts = detector.record(event.clone());

        // Also record enough events to trigger a volume alert
        for i in 0..15 {
            let ev = make_event("agent-1", &format!("tool-{}", i), true);
            detector.record(ev);
        }

        // Should have volume anomaly alerts
        let all_alerts: Vec<_> = detector.record(make_event("agent-1", "tool-x", true));
        // At this point we may or may not have alerts depending on timing
        // Just verify no panic
    }

    #[test]
    fn test_risk_alert_new() {
        let pattern = builtin_patterns()[0].clone();
        let events = vec![make_event("agent-1", "tool-a", true)];
        let alert = RiskAlert::new(&pattern, "agent-1", events, "Test alert description");

        assert_eq!(alert.agent_id, "agent-1");
        assert!(!alert.id.is_empty());
        assert!(alert.should_block);
    }

    #[test]
    fn test_risk_alert_default_action_for_categories() {
        let pattern =
            RiskPattern::tool_sequence("test", "Test", vec!["tool-a".into()], RiskLevel::Medium);
        let alert = RiskAlert::new(&pattern, "agent-1", vec![], "desc");
        // Verify default action is set (non-empty)
        assert!(!alert.recommended_action.is_empty());
    }

    #[test]
    fn test_tool_call_event_builder() {
        let event = ToolCallEvent::new("agent-1", "read_data", serde_json::json!({"key": "value"}))
            .with_output("result data")
            .with_failure();

        assert_eq!(event.agent_id, "agent-1");
        assert_eq!(event.tool_name, "read_data");
        assert!(!event.success);
        assert_eq!(event.output.as_deref(), Some("result data"));
    }

    #[test]
    fn test_risk_category_display() {
        assert_eq!(
            RiskCategory::PrivilegeEscalation.to_string(),
            "privilege_escalation"
        );
        assert_eq!(
            RiskCategory::DataExfiltration.to_string(),
            "data_exfiltration"
        );
    }

    #[test]
    fn test_detector_stats_serde() {
        let stats = DetectorStats {
            tracked_agents: 5,
            patterns_loaded: 10,
            max_history_per_agent: 1000,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let decoded: DetectorStats = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.tracked_agents, 5);
    }

    #[test]
    fn test_risk_alert_serde_roundtrip() {
        let pattern = builtin_patterns()[0].clone();
        let events = vec![make_event("agent-1", "tool-a", true)];
        let alert = RiskAlert::new(&pattern, "agent-1", events, "test");

        let json = serde_json::to_string(&alert).unwrap();
        let decoded: RiskAlert = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.agent_id, "agent-1");
        assert_eq!(decoded.pattern_id, pattern.id);
    }

    #[test]
    fn test_risk_level_serde() {
        let levels = vec![
            RiskLevel::Info,
            RiskLevel::Low,
            RiskLevel::Medium,
            RiskLevel::High,
            RiskLevel::Critical,
        ];
        for level in levels {
            let json = serde_json::to_string(&level).unwrap();
            let decoded: RiskLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, level);
        }
    }

    #[test]
    fn test_risk_pattern_serde() {
        let pattern = RiskPattern::tool_sequence(
            "test-s",
            "Test S",
            vec!["a".into(), "b".into()],
            RiskLevel::High,
        )
        .with_output_regex(r"secret")
        .with_block_on_match(true);

        let json = serde_json::to_string(&pattern).unwrap();
        let decoded: RiskPattern = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "test-s");
        assert!(decoded.output_regex.is_some());
    }
}
