use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tracing::{info, warn, debug, error};

use kias_common::KiasError;

/// Configuration for the self-evaluation loop.
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// Minimum quality score (0..1) to consider a run successful.
    pub quality_threshold: f64,
    /// Maximum number of iterations the loop will attempt.
    pub max_iterations: usize,
    /// Whether the loop should automatically apply fix strategies.
    pub enable_auto_fix: bool,
    /// Timeout for each evaluation step, in seconds.
    pub timeout_secs: u64,
    /// Maximum size of the history buffer.
    pub history_max_size: usize,
    /// Tolerance for numeric comparisons in regression tests.
    pub numeric_tolerance: f64,
}

impl EvalConfig {
    /// Creates a new configuration with default values.
    pub fn new() -> Self {
        EvalConfig {
            quality_threshold: 0.8,
            max_iterations: 10,
            enable_auto_fix: true,
            timeout_secs: 30,
            history_max_size: 100,
            numeric_tolerance: 1e-6,
        }
    }

    /// Validates the configuration parameters.
    pub fn validate(&self) -> Result<(), KiasError> {
        if self.quality_threshold > 1.0 || self.quality_threshold < 0.0 {
            return Err(KiasError::from("quality_threshold must be between 0 and 1"));
        }
        if self.max_iterations == 0 {
            return Err(KiasError::from("max_iterations must be > 0"));
        }
        if self.timeout_secs == 0 {
            return Err(KiasError::from("timeout_secs must be > 0"));
        }
        Ok(())
    }
}

/// Severity level of a detected failure pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// A detected failure pattern observed during evaluation.
#[derive(Debug, Clone)]
pub struct FailurePattern {
    /// Unique identifier for this pattern.
    pub pattern_id: u64,
    /// Human readable description.
    pub description: String,
    /// How many times this pattern has been observed.
    pub frequency: usize,
    /// Severity of the pattern.
    pub severity: Severity,
    /// Suggested fix strategies for this pattern.
    pub suggestions: Vec<String>,
}

/// Represents a single evaluation result.
#[derive(Debug, Clone)]
pub struct EvaluationResult {
    /// Unique identifier for this evaluation run.
    pub run_id: u64,
    /// Timestamp when the evaluation started.
    pub timestamp: Instant,
    /// The raw output string produced by the system.
    pub output: String,
    /// The expected output, if known.
    pub expected: Option<String>,
    /// Computed quality score between 0 and 1.
    pub score: f64,
    /// Whether the result met the quality threshold.
    pub success: bool,
    /// List of pattern IDs that were detected in this run.
    pub detected_patterns: Vec<u64>,
    /// Optional diagnostics message.
    pub diagnostics: Option<String>,
}

/// Auto‑fix strategy that can be applied to improve quality.
#[derive(Debug, Clone)]
pub struct FixStrategy {
    /// Unique identifier for this strategy.
    pub strategy_id: u64,
    /// Descriptive name of the strategy.
    pub name: String,
    /// The kind of adjustment performed.
    pub variant: FixStrategyVariant,
}

#[derive(Debug, Clone)]
pub enum FixStrategyVariant {
    /// Simply retry the evaluation.
    Retry { max_attempts: usize },
    /// Use a simpler algorithm or model.
    Simplify,
    /// Adjust a numeric parameter.
    AdjustParameter { param: String, new_value: f64 },
    /// Fallback to a known baseline configuration.
    FallbackToBaseline,
    /// Increase the timeout for the next evaluation.
    IncreaseTimeout { extra_secs: u64 },
    /// Custom user defined fix.
    Custom { description: String },
}

/// A regression test case consisting of an input and expected output.
#[derive(Debug, Clone)]
pub struct RegressionTest {
    /// Unique identifier for the test.
    pub test_id: u64,
    /// Name describing the test.
    pub name: String,
    /// Input string for the test.
    pub input: String,
    /// Expected output string.
    pub expected_output: String,
}

/// Result of a single regression test.
#[derive(Debug)]
pub struct TestResult {
    /// Identifier of the test.
    pub test_id: u64,
    /// Whether the test passed.
    pub passed: bool,
    /// Details about the result, e.g., mismatch info.
    pub details: String,
}

/// Overall report after running all regression tests.
#[derive(Debug)]
pub struct RegressionReport {
    /// Results for each test.
    pub test_results: Vec<TestResult>,
    /// True if all tests passed.
    pub overall_pass: bool,
    /// Additional notes about the regression run.
    pub notes: String,
}

/// Metrics collected across multiple evaluation runs.
#[derive(Debug, Clone)]
pub struct EvalMetrics {
    /// Total number of evaluations performed.
    pub total_evaluations: usize,
    /// Number of successful evaluations (score >= threshold).
    pub successful: usize,
    /// Number of failed evaluations.
    pub failed: usize,
    /// Running average quality score.
    pub average_score: f64,
    /// Highest observed score.
    pub max_score: f64,
    /// Lowest observed score.
    pub min_score: f64,
    /// Counts of each severity level observed.
    pub severity_counts: HashMap<Severity, usize>,
}

impl EvalMetrics {
    /// Creates a fresh metrics tracker.
    pub fn new() -> Self {
        EvalMetrics {
            total_evaluations: 0,
            successful: 0,
            failed: 0,
            average_score: 0.0,
            max_score: 0.0,
            min_score: 1.0,
            severity_counts: HashMap::new(),
        }
    }

    /// Updates the metrics with a new evaluation result.
    pub fn update(&mut self, result: &EvaluationResult) {
        self.total_evaluations += 1;
        if result.success {
            self.successful += 1;
        } else {
            self.failed += 1;
        }

        // Update average using incremental mean.
        let n = self.total_evaluations as f64;
        self.average_score = (self.average_score * (n - 1.0) + result.score) / n;

        if result.score > self.max_score {
            self.max_score = result.score;
        }
        if result.score < self.min_score {
            self.min_score = result.score;
        }

        // Update severity counts based on detected patterns.
        for pid in &result.detected_patterns {
            if let Some(sev) = severity_from_pattern_id(*pid) {
                *self.severity_counts.entry(sev).or_insert(0) += 1;
            }
        }
    }
}

/// Retrieves a severity for a pattern id.
///
/// In a real system this would be a lookup in a database.
/// Here we define a simple mapping for demonstration.
fn severity_from_pattern_id(id: u64) -> Option<Severity> {
    match id {
        0 => Some(Severity::Low),
        1 => Some(Severity::Low),
        2 => Some(Severity::Medium),
        3 => Some(Severity::Medium),
        4 => Some(Severity::High),
        5 => Some(Severity::Critical),
        _ => None,
    }
}

/// Internal state of the evaluation loop.
struct LoopState {
    /// Current run identifier.
    next_run_id: u64,
    /// Next identifier for patterns.
    next_pattern_id: u64,
    /// Next identifier for strategies.
    next_strategy_id: u64,
    /// Timestamp when the loop started.
    started_at: Instant,
    /// Number of iterations performed.
    iterations: usize,
}

impl LoopState {
    fn new() -> Self {
        LoopState {
            next_run_id: 1,
            next_pattern_id: 0,
            next_strategy_id: 0,
            started_at: Instant::now(),
            iterations: 0,
        }
    }
}

/// The main self‑evaluation loop that monitors, diagnoses and attempts to fix failures.
#[derive(Debug)]
pub struct SelfEvalLoop {
    /// Configuration for the loop.
    config: EvalConfig,
    /// Internal mutable state.
    state: Mutex<LoopState>,
    /// Historical evaluation records.
    history: VecDeque<EvaluationResult>,
    /// Known failure patterns.
    patterns: Vec<FailurePattern>,
    /// Available fix strategies.
    strategies: Vec<FixStrategy>,
    /// Aggregated metrics.
    metrics: Mutex<EvalMetrics>,
    /// Regression test suite.
    regression_tests: Vec<RegressionTest>,
    /// Optional custom parameter store for auto‑fix.
    params: Mutex<HashMap<String, f64>>,
}

impl SelfEvalLoop {
    /// Creates a new self‑evaluation loop.
    ///
    /// # Errors
    /// Returns an error if the supplied configuration is invalid.
    pub fn new(config: EvalConfig) -> Result<Self, KiasError> {
        config.validate()?;
        let state = LoopState::new();
        let metrics = EvalMetrics::new();
        let patterns = default_failure_patterns();
        let strategies = default_fix_strategies();
        let regression_tests = Vec::new();

        Ok(SelfEvalLoop {
            config,
            state: Mutex::new(state),
            history: VecDeque::new(),
            patterns,
            strategies,
            metrics: Mutex::new(metrics),
            regression_tests,
            params: Mutex::new(HashMap::new()),
        })
    }

    /// Adds a regression test to the suite.
    ///
    /// # Errors
    /// Returns an error if the test could not be registered.
    pub fn add_regression_test(&mut self, test: RegressionTest) -> Result<(), KiasError> {
        // Basic validation: ensure input and expected are non‑empty.
        if test.input.is_empty() {
            return Err(KiasError::from("regression test input cannot be empty"));
        }
        if test.expected_output.is_empty() {
            return Err(KiasError::from("regression test expected output cannot be empty"));
        }
        self.regression_tests.push(test);
        Ok(())
    }

    /// Performs a single evaluation of `output` against an optional `expected` output.
    ///
    /// The method computes a quality score, checks for known failure patterns, and records the result.
    ///
    /// # Errors
    /// Returns an error if the evaluation could not be performed.
    pub fn evaluate_output(&self, output: &str, expected: Option<&str>) -> Result<EvaluationResult, KiasError> {
        // Compute quality score.
        let score = compute_quality_score(output, expected);
        let success = score >= self.config.quality_threshold;

        // Detect failure patterns based on keywords in output.
        let detected_ids = self.detect_failure_patterns(output);

        // Build diagnostics message if patterns were found.
        let diagnostics = if detected_ids.is_empty() {
            None
        } else {
            let descriptions: Vec<String> = detected_ids
                .iter()
                .filter_map(|id| self.patterns.iter().find(|p| p.pattern_id == *id).map(|p| p.description.clone()))
                .collect();
            Some(format!("Detected patterns: {}", descriptions.join("; ")))
        };

        // Acquire run id.
        let run_id = {
            let mut state = self.state.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
            let id = state.next_run_id;
            state.next_run_id += 1;
            id
        };

        let timestamp = Instant::now();
        let result = EvaluationResult {
            run_id,
            timestamp,
            output: output.to_string(),
            expected: expected.map(String::from),
            score,
            success,
            detected_patterns: detected_ids,
            diagnostics,
        };

        // Update metrics.
        {
            let mut metrics = self.metrics.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
            metrics.update(&result);
        }

        // Record in history.
        self.push_history(result.clone());

        tracing::info!(
            run_id = result.run_id,
            score = result.score,
            success = result.success,
            "Evaluation completed"
        );

        Ok(result)
    }

    /// Pushes a new evaluation result onto the history, pruning old entries if needed.
    fn push_history(&self, result: EvaluationResult) {
        let max = self.config.history_max_size;
        if self.history.len() >= max {
            // Remove the oldest entry.
            self.history.pop_front();
        }
        self.history.push_back(result);
    }

    /// Scans the output string for known failure patterns.
    fn detect_failure_patterns(&self, output: &str) -> Vec<u64> {
        let mut found = Vec::new();
        for pattern in &self.patterns {
            if pattern.description.to_lowercase().split_whitespace().any(|word| output.to_lowercase().contains(word)) {
                found.push(pattern.pattern_id);
            }
        }
        found
    }

    /// Identifies recurring failure patterns from recent history.
    ///
    /// The method looks at the last N evaluations and tallies pattern frequencies.
    pub fn identify_failure_patterns(&self, _top_n: usize) -> Result<Vec<FailurePattern>, KiasError> {
        // Acquire history snapshot.
        let history = self.history.iter().collect::<Vec<_>>();
        if history.is_empty() {
            tracing::warn!("No history available for pattern identification");
            return Ok(Vec::new());
        }

        // Count occurrences of each pattern.
        let mut frequency_map: HashMap<u64, usize> = HashMap::new();
        for record in &history {
            for pid in &record.detected_patterns {
                *frequency_map.entry(*pid).or_insert(0) += 1;
            }
        }

        // Build result vector for patterns that appear more than once.
        let mut results: Vec<FailurePattern> = self
            .patterns
            .iter()
            .filter(|p| {
                frequency_map
                    .get(&p.pattern_id)
                    .map_or(false, |cnt| *cnt > 1)
            })
            .cloned()
            .collect();

        // Update frequency based on current tallies.
        for r in &mut results {
            if let Some(cnt) = frequency_map.get(&r.pattern_id) {
                r.frequency = *cnt;
            }
        }

        // Sort by frequency descending.
        results.sort_by(|a, b| b.frequency.cmp(&a.frequency));

        tracing::info!(count = results.len(), "Identified failure patterns");
        Ok(results)
    }

    /// Proposes a list of fix strategies based on the identified failure patterns.
    ///
    /// # Errors
    /// Returns an error if strategy proposal fails.
    pub fn propose_fix_strategy(&self, patterns: &[FailurePattern]) -> Result<Vec<FixStrategy>, KiasError> {
        if patterns.is_empty() {
            tracing::info!("No patterns provided, no fix strategies generated");
            return Ok(Vec::new());
        }

        let mut proposals: Vec<FixStrategy> = Vec::new();

        // Acquire next strategy id.
        let next_id = {
            let mut state = self.state.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
            let id = state.next_strategy_id;
            state.next_strategy_id += 1;
            id
        };

        for pattern in patterns {
            // Map pattern id to suggested strategies.
            let variant = match pattern.pattern_id {
                0 | 1 => FixStrategyVariant::Retry { max_attempts: 3 },
                2 | 3 => FixStrategyVariant::Simplify,
                4 => FixStrategyVariant::AdjustParameter {
                    param: "learning_rate".to_string(),
                    new_value: 0.001,
                },
                5 => FixStrategyVariant::IncreaseTimeout {
                    extra_secs: self.config.timeout_secs,
                },
                _ => FixStrategyVariant::Custom {
                    description: format!("Generic fix for pattern {}", pattern.pattern_id),
                },
            };

            proposals.push(FixStrategy {
                strategy_id: next_id,
                name: format!("AutoFix for pattern {}", pattern.pattern_id),
                variant,
            });
        }

        tracing::info!(strategy_count = proposals.len(), "Proposed fix strategies");
        Ok(proposals)
    }

    /// Applies a fix strategy, updating the loop's configuration or parameters accordingly.
    ///
    /// # Errors
    /// Returns an error if the strategy could not be applied.
    pub fn apply_fix_strategy(&mut self, strategy: FixStrategy) -> Result<(), KiasError> {
        // Acquire exclusive access to config and parameters.
        let mut params = self.params.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;

        match strategy.variant {
            FixStrategyVariant::Retry { max_attempts } => {
                tracing::info!(max_attempts = max_attempts, "Applying retry strategy");
                // In a real system we would store retry count somewhere.
                params.insert("retry_max_attempts".to_string(), max_attempts as f64);
            }
            FixStrategyVariant::Simplify => {
                tracing::info!("Applying simplification strategy");
                params.insert("simplify".to_string(), 1.0);
            }
            FixStrategyVariant::AdjustParameter { ref param, new_value } => {
                tracing::info!(param = param, new_value = new_value, "Applying parameter adjustment");
                params.insert(param.clone(), new_value);
            }
            FixStrategyVariant::FallbackToBaseline => {
                tracing::info!("Falling back to baseline configuration");
                // Reset parameters to baseline values.
                params.insert("learning_rate".to_string(), 0.01);
                params.insert("batch_size".to_string(), 32.0);
            }
            FixStrategyVariant::IncreaseTimeout { extra_secs } => {
                tracing::info!(extra_secs = extra_secs, "Increasing timeout");
                let current = self.config.timeout_secs;
                self.config.timeout_secs = current.saturating_add(extra_secs);
            }
            FixStrategyVariant::Custom { ref description } => {
                tracing::info!(description = description, "Applying custom strategy");
                params.insert("custom_fix".to_string(), 1.0);
            }
        }

        Ok(())
    }

    /// Verifies that the current implementation still passes the regression test suite.
    ///
    /// Each test is executed by running the evaluation loop with the test's input and comparing
    /// the produced output with the expected output.
    ///
    /// # Errors
    /// Returns an error if the regression verification cannot be performed.
    pub fn verify_regression(&self) -> Result<RegressionReport, KiasError> {
        if self.regression_tests.is_empty() {
            tracing::warn!("No regression tests defined");
            return Ok(RegressionReport {
                test_results: Vec::new(),
                overall_pass: true,
                notes: "No tests to run".to_string(),
            });
        }

        let mut results: Vec<TestResult> = Vec::new();
        let mut passed_count = 0usize;

        for test in &self.regression_tests {
            // Run evaluation on the test input.
            let eval_result = self.evaluate_output(&test.input, Some(&test.expected_output))?;
            let passed = eval_result.success;
            if passed {
                passed_count += 1;
            }

            let details = if passed {
                format!("Output matched expected with score {}", eval_result.score)
            } else {
                format!(
                    "Score {} below threshold {}",
                    eval_result.score, self.config.quality_threshold
                )
            };

            results.push(TestResult {
                test_id: test.test_id,
                passed,
                details,
            });
        }

        let overall_pass = passed_count == results.len();
        let notes = if overall_pass {
            format!("All {} tests passed", results.len())
        } else {
            format!("{}/{} tests passed", passed_count, results.len())
        };

        tracing::info!(overall_pass = overall_pass, "Regression verification complete");
        Ok(RegressionReport {
            test_results: results,
            overall_pass,
            notes,
        })
    }

    /// Executes the self‑evaluation loop over a collection of outputs.
    ///
    /// For each output it evaluates quality, identifies patterns, proposes fixes,
    /// and optionally applies them. The method returns a vector of evaluation results.
    ///
    /// # Errors
    /// Returns an error if any step of the loop fails.
    pub fn run_loop(&mut self, outputs: Vec<(String, Option<String>)>) -> Result<Vec<EvaluationResult>, KiasError> {
        let mut all_results: Vec<EvaluationResult> = Vec::new();
        let max_iterations = self.config.max_iterations;

        // Acquire state to count iterations.
        {
            let mut state = self.state.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
            state.iterations = 0;
        }

        for (output, expected) in outputs {
            // Check iteration limit.
            {
                let state = self.state.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
                if state.iterations >= max_iterations {
                    tracing::warn!(
                        max_iterations = max_iterations,
                        "Maximum iterations reached, stopping loop"
                    );
                    break;
                }
            }

            // Perform evaluation.
            let result = self.evaluate_output(&output, expected.as_deref())?;
            all_results.push(result.clone());

            // Increment iteration counter.
            {
                let mut state = self.state.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
                state.iterations += 1;
            }

            // If the result succeeded, continue.
            if result.success {
                tracing::info!(run_id = result.run_id, "Evaluation succeeded, skipping fix");
                continue;
            }

            // Failure detected – identify patterns.
            let patterns = self.identify_failure_patterns(5)?;
            if patterns.is_empty() {
                tracing::info!(run_id = result.run_id, "No failure patterns identified");
                continue;
            }

            // Propose strategies.
            let strategies = self.propose_fix_strategy(&patterns)?;

            // Apply strategies if auto‑fix is enabled.
            if self.config.enable_auto_fix {
                for strategy in strategies {
                    if let Err(e) = self.apply_fix_strategy(strategy) {
                        tracing::warn!(error = ?e, "Failed to apply a fix strategy");
                    }
                }
            } else {
                tracing::info!(run_id = result.run_id, "Auto‑fix disabled, skipping strategy application");
            }
        }

        // Final regression verification.
        let report = self.verify_regression()?;
        tracing::info!(
            overall_pass = report.overall_pass,
            "Loop completed, regression verification finished"
        );

        Ok(all_results)
    }

    /// Returns a snapshot of the current evaluation metrics.
    pub fn metrics_snapshot(&self) -> Result<EvalMetrics, KiasError> {
        let metrics = self.metrics.lock().map_err(|e| KiasError::from(format!("lock error: {}", e)))?;
        Ok(metrics.clone())
    }

    /// Returns the number of evaluations currently stored in the history.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

/// Computes a simple quality score based on similarity between output and expected.
///
/// The algorithm uses token overlap and length penalties.
fn compute_quality_score(output: &str, expected: Option<&str>) -> f64 {
    match expected {
        None => {
            // If no reference is available, give a modest score based on non‑emptiness.
            if output.trim().is_empty() {
                0.0
            } else {
                0.5
            }
        }
        Some(exp) => {
            // Compute simple token-based Jaccard similarity.
            let output_tokens: HashSet<&str> = output.split_whitespace().collect();
            let expected_tokens: HashSet<&str> = exp.split_whitespace().collect();
            if output_tokens.is_empty() && expected_tokens.is_empty() {
                return 1.0;
            }
            let intersection: usize = output_tokens.intersection(&expected_tokens).count();
            let union: usize = output_tokens.union(&expected_tokens).count();
            if union == 0 {
                0.0
            } else {
                intersection as f64 / union as f64
            }
        }
    }
}

/// Returns the default set of known failure patterns.
fn default_failure_patterns() -> Vec<FailurePattern> {
    vec![
        FailurePattern {
            pattern_id: 0,
            description: "timeout".to_string(),
            frequency: 0,
            severity: Severity::Low,
            suggestions: vec!["Increase timeout".to_string(), "Retry".to_string()],
        },
        FailurePattern {
            pattern_id: 1,
            description: "resource exhaustion".to_string(),
            frequency: 0,
            severity: Severity::Medium,
            suggestions: vec!["Reduce batch size".to_string()],
        },
        FailurePattern {
            pattern_id: 2,
            description: "assertion failure".to_string(),
            frequency: 0,
            severity: Severity::High,
            suggestions: vec!["Simplify logic".to_string()],
        },</think>