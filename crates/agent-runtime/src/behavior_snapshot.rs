//! Agent Behavior Snapshot — Regression Testing Framework
//!
//! Captures "golden state" snapshots of agent behavior (tool calls, outputs,
//! decisions) and compares current runs against them to detect regressions.
//!
//! ## Use Cases
//!
//! - **CI regression gates**: Block PRs that change agent behavior.
//! - **GxP compliance**: Prove agent behavior hasn't changed between releases.
//! - **Canary analysis**: Compare production vs. staging behavior.
//!
//! ## Design
//!
//! 1. `BehaviorSnapshot` captures a single agent run's observable behavior.
//! 2. `BehaviorComparator` diffs two snapshots and produces a `DiffReport`.
//! 3. `GoldenSuite` manages a collection of golden snapshots for a test suite.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
//  BehaviorSnapshot — captures one agent run
// ═══════════════════════════════════════════════════════════════════════════

/// Observable behavior of a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSnapshot {
    /// Unique identifier for this snapshot.
    pub id: String,
    /// Which agent produced this behavior.
    pub agent_id: String,
    /// Version/commit of the agent code.
    pub agent_version: String,
    /// The input prompt that triggered this run.
    pub input: String,
    /// Sequence of tool calls made during the run.
    pub tool_calls: Vec<ToolCall>,
    /// Final output produced by the agent.
    pub output: String,
    /// Token usage breakdown.
    pub token_usage: TokenUsage,
    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Custom metadata (e.g. model name, temperature).
    pub metadata: HashMap<String, serde_json::Value>,
    /// When this snapshot was captured.
    pub captured_at: chrono::DateTime<chrono::Utc>,
}

/// A single tool call observed during an agent run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Name of the tool invoked.
    pub tool_name: String,
    /// Arguments passed (JSON).
    pub arguments: serde_json::Value,
    /// Whether the call succeeded.
    pub success: bool,
    /// Duration of this specific call in milliseconds.
    pub duration_ms: u64,
}

/// Token usage for a run.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
//  DiffReport — what changed between two snapshots
// ═══════════════════════════════════════════════════════════════════════════

/// Result of comparing two behavior snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffReport {
    /// Whether the behaviors are considered equivalent.
    pub equivalent: bool,
    /// All detected differences.
    pub differences: Vec<BehaviorDifference>,
    /// Summary statistics.
    pub summary: DiffSummary,
}

/// A single difference between two behavior snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorDifference {
    /// Category of difference.
    pub kind: DifferenceKind,
    /// Human-readable description.
    pub description: String,
    /// Value from the golden snapshot.
    pub golden_value: Option<String>,
    /// Value from the current snapshot.
    pub current_value: Option<String>,
}

/// Category of behavioral difference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DifferenceKind {
    /// Different tool calls were made.
    ToolCallSequence,
    /// A tool call had different arguments.
    ToolCallArguments,
    /// A tool call had a different success/failure status.
    ToolCallResult,
    /// Final output differs.
    Output,
    /// Token usage changed significantly.
    TokenUsage,
    /// Duration changed significantly.
    Duration,
    /// A new tool call was added.
    ToolCallAdded,
    /// A tool call was removed.
    ToolCallRemoved,
}

/// Summary of differences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    /// Total number of differences.
    pub total_differences: usize,
    /// Number of critical differences (tool call changes, output changes).
    pub critical_differences: usize,
    /// Token usage delta (current - golden).
    pub token_delta: i64,
    /// Duration delta in ms (current - golden).
    pub duration_delta_ms: i64,
}

// ═══════════════════════════════════════════════════════════════════════════
//  BehaviorComparator — diffs two snapshots
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for behavior comparison sensitivity.
#[derive(Debug, Clone)]
pub struct ComparisonConfig {
    /// How much token usage can differ before flagging (percentage, 0.0–1.0).
    pub token_tolerance: f64,
    /// How much duration can differ before flagging (percentage, 0.0–1.0).
    pub duration_tolerance: f64,
    /// Whether to compare tool call arguments strictly.
    pub strict_arguments: bool,
    /// Whether output comparison is case-sensitive.
    pub case_sensitive_output: bool,
}

impl Default for ComparisonConfig {
    fn default() -> Self {
        Self {
            token_tolerance: 0.2,    // 20% tolerance
            duration_tolerance: 0.5, // 50% tolerance
            strict_arguments: true,
            case_sensitive_output: true,
        }
    }
}

/// Compares two behavior snapshots and produces a diff report.
pub struct BehaviorComparator {
    config: ComparisonConfig,
}

impl BehaviorComparator {
    pub fn new(config: ComparisonConfig) -> Self {
        Self { config }
    }

    pub fn with_defaults() -> Self {
        Self::new(ComparisonConfig::default())
    }

    /// Compare a current snapshot against a golden snapshot.
    pub fn compare(&self, golden: &BehaviorSnapshot, current: &BehaviorSnapshot) -> DiffReport {
        let mut differences = Vec::new();

        // Compare tool call sequences
        self.compare_tool_calls(golden, current, &mut differences);

        // Compare output
        self.compare_output(golden, current, &mut differences);

        // Compare token usage
        self.compare_token_usage(golden, current, &mut differences);

        // Compare duration
        self.compare_duration(golden, current, &mut differences);

        let critical_differences = differences
            .iter()
            .filter(|d| {
                matches!(
                    d.kind,
                    DifferenceKind::ToolCallSequence
                        | DifferenceKind::ToolCallResult
                        | DifferenceKind::Output
                )
            })
            .count();

        let token_delta =
            current.token_usage.total_tokens as i64 - golden.token_usage.total_tokens as i64;
        let duration_delta_ms = current.duration_ms as i64 - golden.duration_ms as i64;

        DiffReport {
            equivalent: differences.is_empty(),
            summary: DiffSummary {
                total_differences: differences.len() + critical_differences,
                critical_differences,
                token_delta,
                duration_delta_ms,
            },
            differences,
        }
    }

    fn compare_tool_calls(
        &self,
        golden: &BehaviorSnapshot,
        current: &BehaviorSnapshot,
        differences: &mut Vec<BehaviorDifference>,
    ) {
        let golden_count = golden.tool_calls.len();
        let current_count = current.tool_calls.len();

        if golden_count != current_count {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::ToolCallSequence,
                description: format!("Tool call count changed: {golden_count} -> {current_count}"),
                golden_value: Some(golden_count.to_string()),
                current_value: Some(current_count.to_string()),
            });
        }

        let min_len = golden_count.min(current_count);
        for i in 0..min_len {
            let g = &golden.tool_calls[i];
            let c = &current.tool_calls[i];

            if g.tool_name != c.tool_name {
                differences.push(BehaviorDifference {
                    kind: DifferenceKind::ToolCallSequence,
                    description: format!("Tool call #{i} name changed"),
                    golden_value: Some(g.tool_name.clone()),
                    current_value: Some(c.tool_name.clone()),
                });
            }

            if self.config.strict_arguments && g.arguments != c.arguments {
                differences.push(BehaviorDifference {
                    kind: DifferenceKind::ToolCallArguments,
                    description: format!("Tool call #{i} arguments changed"),
                    golden_value: Some(g.arguments.to_string()),
                    current_value: Some(c.arguments.to_string()),
                });
            }

            if g.success != c.success {
                differences.push(BehaviorDifference {
                    kind: DifferenceKind::ToolCallResult,
                    description: format!("Tool call #{i} result changed"),
                    golden_value: Some(g.success.to_string()),
                    current_value: Some(c.success.to_string()),
                });
            }
        }

        // Extra tool calls in current
        for i in min_len..current_count {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::ToolCallAdded,
                description: format!("New tool call #{i}: {}", current.tool_calls[i].tool_name),
                golden_value: None,
                current_value: Some(current.tool_calls[i].tool_name.clone()),
            });
        }

        // Missing tool calls
        for i in min_len..golden_count {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::ToolCallRemoved,
                description: format!("Removed tool call #{i}: {}", golden.tool_calls[i].tool_name),
                golden_value: Some(golden.tool_calls[i].tool_name.clone()),
                current_value: None,
            });
        }
    }

    fn compare_output(
        &self,
        golden: &BehaviorSnapshot,
        current: &BehaviorSnapshot,
        differences: &mut Vec<BehaviorDifference>,
    ) {
        let matches = if self.config.case_sensitive_output {
            golden.output == current.output
        } else {
            golden.output.to_lowercase() == current.output.to_lowercase()
        };

        if !matches {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::Output,
                description: "Final output differs".to_string(),
                golden_value: Some(truncate(&golden.output, 200)),
                current_value: Some(truncate(&current.output, 200)),
            });
        }
    }

    fn compare_token_usage(
        &self,
        golden: &BehaviorSnapshot,
        current: &BehaviorSnapshot,
        differences: &mut Vec<BehaviorDifference>,
    ) {
        if golden.token_usage.total_tokens == 0 {
            return;
        }
        let delta = (current.token_usage.total_tokens as f64
            - golden.token_usage.total_tokens as f64)
            / golden.token_usage.total_tokens as f64;

        if delta.abs() > self.config.token_tolerance {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::TokenUsage,
                description: format!("Token usage changed by {:.0}%", delta * 100.0),
                golden_value: Some(golden.token_usage.total_tokens.to_string()),
                current_value: Some(current.token_usage.total_tokens.to_string()),
            });
        }
    }

    fn compare_duration(
        &self,
        golden: &BehaviorSnapshot,
        current: &BehaviorSnapshot,
        differences: &mut Vec<BehaviorDifference>,
    ) {
        if golden.duration_ms == 0 {
            return;
        }
        let delta =
            (current.duration_ms as f64 - golden.duration_ms as f64) / golden.duration_ms as f64;

        if delta.abs() > self.config.duration_tolerance {
            differences.push(BehaviorDifference {
                kind: DifferenceKind::Duration,
                description: format!("Duration changed by {:.0}%", delta * 100.0),
                golden_value: Some(format!("{}ms", golden.duration_ms)),
                current_value: Some(format!("{}ms", current.duration_ms)),
            });
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  GoldenSuite — manages a collection of golden snapshots
// ═══════════════════════════════════════════════════════════════════════════

/// A named test case with a golden snapshot and expected behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenCase {
    /// Human-readable test case name.
    pub name: String,
    /// Description of what this case tests.
    pub description: String,
    /// The golden behavior snapshot.
    pub golden: BehaviorSnapshot,
    /// Whether this case is expected to pass (false = known regression).
    pub expected_pass: bool,
}

/// Collection of golden test cases for regression testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldenSuite {
    /// Suite name.
    pub name: String,
    /// Agent version this suite was captured against.
    pub agent_version: String,
    /// Test cases.
    pub cases: Vec<GoldenCase>,
}

impl GoldenSuite {
    pub fn new(name: &str, agent_version: &str) -> Self {
        Self {
            name: name.to_string(),
            agent_version: agent_version.to_string(),
            cases: Vec::new(),
        }
    }

    /// Add a golden test case.
    pub fn add_case(&mut self, case: GoldenCase) {
        self.cases.push(case);
    }

    /// Run all golden cases against current snapshots and return reports.
    pub fn run(
        &self,
        current_snapshots: &HashMap<String, BehaviorSnapshot>,
        comparator: &BehaviorComparator,
    ) -> Vec<(String, DiffReport)> {
        let mut results = Vec::new();

        for case in &self.cases {
            if let Some(current) = current_snapshots.get(&case.name) {
                let report = comparator.compare(&case.golden, current);
                results.push((case.name.clone(), report));
            }
        }

        results
    }

    /// Serialize the suite to JSON (for persistence).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a suite from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(
        id: &str,
        tool_calls: Vec<ToolCall>,
        output: &str,
        tokens: u32,
        duration: u64,
    ) -> BehaviorSnapshot {
        BehaviorSnapshot {
            id: id.to_string(),
            agent_id: "test-agent".to_string(),
            agent_version: "1.0.0".to_string(),
            input: "test input".to_string(),
            tool_calls,
            output: output.to_string(),
            token_usage: TokenUsage {
                prompt_tokens: tokens / 2,
                completion_tokens: tokens / 2,
                total_tokens: tokens,
            },
            duration_ms: duration,
            metadata: HashMap::new(),
            captured_at: chrono::Utc::now(),
        }
    }

    fn make_tool_call(name: &str, args: &str, success: bool) -> ToolCall {
        ToolCall {
            tool_name: name.to_string(),
            arguments: serde_json::from_str(args).unwrap_or_default(),
            success,
            duration_ms: 100,
        }
    }

    // ── Identical snapshots ──────────────────────────────────────────────

    #[test]
    fn test_identical_snapshots_equivalent() {
        let comparator = BehaviorComparator::with_defaults();
        let golden = make_snapshot(
            "g1",
            vec![make_tool_call("search", r#"{"q":"test"}"#, true)],
            "result",
            1000,
            500,
        );
        let mut current = golden.clone();
        current.id = "c1".to_string(); // different id, same behavior

        let report = comparator.compare(&golden, &current);
        assert!(report.equivalent);
        assert!(report.differences.is_empty());
    }

    // ── Output differs ───────────────────────────────────────────────────

    #[test]
    fn test_output_difference_detected() {
        let comparator = BehaviorComparator::with_defaults();
        let golden = make_snapshot("g1", vec![], "hello", 100, 100);
        let current = make_snapshot("c1", vec![], "world", 100, 100);

        let report = comparator.compare(&golden, &current);
        assert!(!report.equivalent);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::Output));
    }

    // ── Tool call sequence differs ───────────────────────────────────────

    #[test]
    fn test_tool_call_count_change() {
        let comparator = BehaviorComparator::with_defaults();
        let golden = make_snapshot(
            "g1",
            vec![make_tool_call("search", "{}", true)],
            "ok",
            100,
            100,
        );
        let current = make_snapshot(
            "c1",
            vec![
                make_tool_call("search", "{}", true),
                make_tool_call("fetch", "{}", true),
            ],
            "ok",
            200,
            200,
        );

        let report = comparator.compare(&golden, &current);
        assert!(!report.equivalent);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::ToolCallAdded));
    }

    #[test]
    fn test_tool_call_name_change() {
        let comparator = BehaviorComparator::with_defaults();
        let golden = make_snapshot(
            "g1",
            vec![make_tool_call("search", "{}", true)],
            "ok",
            100,
            100,
        );
        let current = make_snapshot(
            "c1",
            vec![make_tool_call("fetch", "{}", true)],
            "ok",
            100,
            100,
        );

        let report = comparator.compare(&golden, &current);
        assert!(!report.equivalent);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::ToolCallSequence));
    }

    #[test]
    fn test_tool_call_result_change() {
        let comparator = BehaviorComparator::with_defaults();
        let golden = make_snapshot(
            "g1",
            vec![make_tool_call("search", "{}", true)],
            "ok",
            100,
            100,
        );
        let current = make_snapshot(
            "c1",
            vec![make_tool_call("search", "{}", false)],
            "ok",
            100,
            100,
        );

        let report = comparator.compare(&golden, &current);
        assert!(!report.equivalent);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::ToolCallResult));
    }

    // ── Token usage tolerance ────────────────────────────────────────────

    #[test]
    fn test_token_usage_within_tolerance() {
        let comparator = BehaviorComparator::with_defaults(); // 20% tolerance
        let golden = make_snapshot("g1", vec![], "ok", 1000, 100);
        let current = make_snapshot("c1", vec![], "ok", 1100, 100); // +10%

        let report = comparator.compare(&golden, &current);
        // 10% is within 20% tolerance, should not flag
        assert!(!report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::TokenUsage));
    }

    #[test]
    fn test_token_usage_exceeds_tolerance() {
        let comparator = BehaviorComparator::with_defaults(); // 20% tolerance
        let golden = make_snapshot("g1", vec![], "ok", 1000, 100);
        let current = make_snapshot("c1", vec![], "ok", 1500, 100); // +50%

        let report = comparator.compare(&golden, &current);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::TokenUsage));
    }

    // ── Duration tolerance ───────────────────────────────────────────────

    #[test]
    fn test_duration_within_tolerance() {
        let comparator = BehaviorComparator::with_defaults(); // 50% tolerance
        let golden = make_snapshot("g1", vec![], "ok", 100, 1000);
        let current = make_snapshot("c1", vec![], "ok", 100, 1400); // +40%

        let report = comparator.compare(&golden, &current);
        assert!(!report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::Duration));
    }

    #[test]
    fn test_duration_exceeds_tolerance() {
        let comparator = BehaviorComparator::with_defaults(); // 50% tolerance
        let golden = make_snapshot("g1", vec![], "ok", 100, 1000);
        let current = make_snapshot("c1", vec![], "ok", 100, 2000); // +100%

        let report = comparator.compare(&golden, &current);
        assert!(report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::Duration));
    }

    // ── Comparison config ────────────────────────────────────────────────

    #[test]
    fn test_case_insensitive_output() {
        let mut config = ComparisonConfig::default();
        config.case_sensitive_output = false;
        let comparator = BehaviorComparator::new(config);

        let golden = make_snapshot("g1", vec![], "Hello World", 100, 100);
        let current = make_snapshot("c1", vec![], "hello world", 100, 100);

        let report = comparator.compare(&golden, &current);
        assert!(report.equivalent);
    }

    #[test]
    fn test_non_strict_arguments() {
        let mut config = ComparisonConfig::default();
        config.strict_arguments = false;
        let comparator = BehaviorComparator::new(config);

        let golden = make_snapshot(
            "g1",
            vec![make_tool_call("search", r#"{"q":"old"}"#, true)],
            "ok",
            100,
            100,
        );
        let current = make_snapshot(
            "c1",
            vec![make_tool_call("search", r#"{"q":"new"}"#, true)],
            "ok",
            100,
            100,
        );

        let report = comparator.compare(&golden, &current);
        // Arguments differ but strict_arguments=false, so no arg diff
        assert!(!report
            .differences
            .iter()
            .any(|d| d.kind == DifferenceKind::ToolCallArguments));
    }

    // ── GoldenSuite ──────────────────────────────────────────────────────

    #[test]
    fn test_golden_suite_run() {
        let mut suite = GoldenSuite::new("regression", "1.0.0");
        suite.add_case(GoldenCase {
            name: "basic_search".to_string(),
            description: "Agent should use search tool".to_string(),
            golden: make_snapshot(
                "g1",
                vec![make_tool_call("search", r#"{"q":"test"}"#, true)],
                "found",
                100,
                100,
            ),
            expected_pass: true,
        });

        let mut current = HashMap::new();
        current.insert(
            "basic_search".to_string(),
            make_snapshot(
                "c1",
                vec![make_tool_call("search", r#"{"q":"test"}"#, true)],
                "found",
                110,
                100,
            ),
        );

        let comparator = BehaviorComparator::with_defaults();
        let results = suite.run(&current, &comparator);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "basic_search");
        assert!(results[0].1.equivalent);
    }

    #[test]
    fn test_golden_suite_json_roundtrip() {
        let mut suite = GoldenSuite::new("test-suite", "2.0.0");
        suite.add_case(GoldenCase {
            name: "case1".to_string(),
            description: "test case".to_string(),
            golden: make_snapshot("g1", vec![], "output", 100, 50),
            expected_pass: true,
        });

        let json = suite.to_json().unwrap();
        let deserialized = GoldenSuite::from_json(&json).unwrap();
        assert_eq!(deserialized.name, "test-suite");
        assert_eq!(deserialized.agent_version, "2.0.0");
        assert_eq!(deserialized.cases.len(), 1);
        assert_eq!(deserialized.cases[0].name, "case1");
    }

    // ── Serde roundtrips ─────────────────────────────────────────────────

    #[test]
    fn test_behavior_snapshot_serde() {
        let snapshot = make_snapshot(
            "s1",
            vec![make_tool_call("search", "{}", true)],
            "result",
            500,
            200,
        );
        let json = serde_json::to_string(&snapshot).unwrap();
        let deserialized: BehaviorSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "s1");
        assert_eq!(deserialized.tool_calls.len(), 1);
        assert_eq!(deserialized.output, "result");
    }

    #[test]
    fn test_diff_report_serde() {
        let report = DiffReport {
            equivalent: false,
            differences: vec![BehaviorDifference {
                kind: DifferenceKind::Output,
                description: "differs".to_string(),
                golden_value: Some("a".to_string()),
                current_value: Some("b".to_string()),
            }],
            summary: DiffSummary {
                total_differences: 1,
                critical_differences: 1,
                token_delta: 0,
                duration_delta_ms: 0,
            },
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: DiffReport = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.equivalent);
        assert_eq!(deserialized.differences.len(), 1);
    }
}
