//! # Execution Cost Predictor
//!
//! Pre-dispatch token and latency estimation using historical task similarity.
//! Finds similar past tasks by description matching and aggregates their
//! cost/duration statistics to produce predictions.
//!
//! ## Approach
//!
//! Uses a combination of:
//! - **Jaccard similarity** on word tokens for fast matching
//! - **N-gram overlap** for partial phrase matching
//! - **Task type** exact match boost
//!
//! ## Usage
//!
//! ```rust
//! use kias_agent_view::cost_predictor::*;
//! use kias_agent_view::task_history::*;
//!
//! let mut predictor = CostPredictor::new(PredictorConfig::default());
//!
//! // Feed historical data
//! predictor.ingest(TaskRecord::new("t1", "a1", "code-review", "Review PR #42", TaskOutcome::Success, 5000).with_tokens(1200));
//! predictor.ingest(TaskRecord::new("t2", "a1", "code-review", "Review PR #100", TaskOutcome::Success, 6000).with_tokens(1500));
//!
//! // Predict
//! let prediction = predictor.predict("Review PR #200", "code-review");
//! assert!(prediction.estimated_tokens.is_some());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::task_history::{TaskOutcome, TaskRecord};

// ── Configuration ───────────────────────────────────────────────────────

/// Configuration for the cost predictor.
#[derive(Debug, Clone)]
pub struct PredictorConfig {
    /// Maximum number of historical records to keep per task type.
    pub max_history_per_type: usize,
    /// Minimum similarity score to consider a match (0.0 - 1.0).
    pub min_similarity: f64,
    /// Number of top-k similar tasks to use for prediction.
    pub top_k: usize,
    /// Weight for task type exact match (boost factor).
    pub type_match_boost: f64,
    /// Whether to use n-gram matching in addition to word tokens.
    pub use_ngrams: bool,
    /// N-gram size (2 = bigrams, 3 = trigrams).
    pub ngram_size: usize,
}

impl Default for PredictorConfig {
    fn default() -> Self {
        Self {
            max_history_per_type: 1000,
            min_similarity: 0.1,
            top_k: 5,
            type_match_boost: 1.5,
            use_ngrams: true,
            ngram_size: 2,
        }
    }
}

// ── Prediction Result ───────────────────────────────────────────────────

/// Cost and latency prediction for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostPrediction {
    /// Estimated token consumption.
    pub estimated_tokens: Option<u64>,
    /// Token estimate confidence (0.0 - 1.0).
    pub token_confidence: f64,
    /// Estimated duration in milliseconds.
    pub estimated_duration_ms: Option<u64>,
    /// Duration estimate confidence (0.0 - 1.0).
    pub duration_confidence: f64,
    /// Number of similar historical tasks found.
    pub similar_task_count: usize,
    /// The most similar tasks used for prediction.
    pub similar_tasks: Vec<SimilarTask>,
    /// Predicted success rate based on similar tasks.
    pub success_rate: f64,
}

/// A similar historical task used for prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarTask {
    pub task_id: String,
    pub task_type: String,
    pub description: String,
    pub similarity_score: f64,
    pub tokens_consumed: u64,
    pub duration_ms: u64,
    pub outcome: TaskOutcome,
}

// ── Cost Predictor ──────────────────────────────────────────────────────

/// Predicts execution cost and latency based on historical task similarity.
pub struct CostPredictor {
    config: PredictorConfig,
    /// Historical records grouped by task type.
    history: HashMap<String, Vec<TaskRecord>>,
    /// Total records ingested.
    total_ingested: usize,
}

impl CostPredictor {
    pub fn new(config: PredictorConfig) -> Self {
        Self {
            config,
            history: HashMap::new(),
            total_ingested: 0,
        }
    }

    /// Ingest a historical task record for future predictions.
    pub fn ingest(&mut self, record: TaskRecord) {
        let type_key = record.task_type.clone();
        let entries = self.history.entry(type_key).or_default();

        // Evict oldest if at capacity
        if entries.len() >= self.config.max_history_per_type {
            entries.remove(0);
        }

        entries.push(record);
        self.total_ingested += 1;
    }

    /// Ingest multiple records at once.
    pub fn ingest_batch(&mut self, records: Vec<TaskRecord>) {
        for record in records {
            self.ingest(record);
        }
    }

    /// Predict cost and latency for a new task.
    pub fn predict(&self, description: &str, task_type: &str) -> CostPrediction {
        let desc_tokens = tokenize(description);
        let desc_ngrams = if self.config.use_ngrams {
            ngrams(description, self.config.ngram_size)
        } else {
            HashSet::new()
        };

        // Find similar tasks across all types
        let mut scored: Vec<(f64, &TaskRecord)> = Vec::new();

        for (type_key, records) in &self.history {
            for record in records {
                let mut sim = jaccard_similarity(&desc_tokens, &tokenize(&record.description));

                // N-gram bonus
                if self.config.use_ngrams {
                    let rec_ngrams = ngrams(&record.description, self.config.ngram_size);
                    let ngram_sim = jaccard_similarity_set(&desc_ngrams, &rec_ngrams);
                    sim = sim * 0.7 + ngram_sim * 0.3;
                }

                // Type match boost
                if type_key == task_type {
                    sim *= self.config.type_match_boost;
                }

                // Cap at 1.0
                sim = sim.min(1.0);

                if sim >= self.config.min_similarity {
                    scored.push((sim, record));
                }
            }
        }

        // Sort by similarity descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top-k
        let top: Vec<_> = scored.into_iter().take(self.config.top_k).collect();

        if top.is_empty() {
            return CostPrediction {
                estimated_tokens: None,
                token_confidence: 0.0,
                estimated_duration_ms: None,
                duration_confidence: 0.0,
                similar_task_count: 0,
                similar_tasks: Vec::new(),
                success_rate: 0.0,
            };
        }

        // Weighted average based on similarity
        let total_weight: f64 = top.iter().map(|(sim, _)| sim).sum();

        let weighted_tokens: f64 = top
            .iter()
            .map(|(sim, rec)| *sim * rec.tokens_consumed as f64)
            .sum();
        let estimated_tokens = (weighted_tokens / total_weight) as u64;

        let weighted_duration: f64 = top
            .iter()
            .map(|(sim, rec)| *sim * rec.duration_ms as f64)
            .sum();
        let estimated_duration = (weighted_duration / total_weight) as u64;

        let success_count = top
            .iter()
            .filter(|(_, rec)| rec.outcome == TaskOutcome::Success)
            .count();
        let success_rate = success_count as f64 / top.len() as f64;

        // Confidence based on similarity scores and sample size
        let avg_sim: f64 = total_weight / top.len() as f64;
        let sample_factor = (top.len() as f64 / self.config.top_k as f64).min(1.0);
        let confidence = (avg_sim * sample_factor).min(1.0);

        let similar_tasks: Vec<SimilarTask> = top
            .iter()
            .map(|(sim, rec)| SimilarTask {
                task_id: rec.task_id.clone(),
                task_type: rec.task_type.clone(),
                description: rec.description.clone(),
                similarity_score: (*sim * 1000.0).round() / 1000.0,
                tokens_consumed: rec.tokens_consumed,
                duration_ms: rec.duration_ms,
                outcome: rec.outcome.clone(),
            })
            .collect();

        CostPrediction {
            estimated_tokens: Some(estimated_tokens),
            token_confidence: confidence,
            estimated_duration_ms: Some(estimated_duration),
            duration_confidence: confidence,
            similar_task_count: top.len(),
            similar_tasks,
            success_rate,
        }
    }

    /// Get statistics about the stored history.
    pub fn stats(&self) -> PredictorStats {
        let mut by_type = HashMap::new();
        let mut total_tokens = 0u64;
        let mut total_duration = 0u64;

        for (task_type, records) in &self.history {
            by_type.insert(task_type.clone(), records.len());
            for rec in records {
                total_tokens += rec.tokens_consumed;
                total_duration += rec.duration_ms;
            }
        }

        PredictorStats {
            total_ingested: self.total_ingested,
            by_type,
            avg_tokens: if self.total_ingested > 0 {
                total_tokens / self.total_ingested as u64
            } else {
                0
            },
            avg_duration_ms: if self.total_ingested > 0 {
                total_duration / self.total_ingested as u64
            } else {
                0
            },
        }
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.history.clear();
        self.total_ingested = 0;
    }
}

/// Statistics about the predictor's stored history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictorStats {
    pub total_ingested: usize,
    pub by_type: HashMap<String, usize>,
    pub avg_tokens: u64,
    pub avg_duration_ms: u64,
}

// ── Text Similarity ─────────────────────────────────────────────────────

fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && w.len() > 1)
        .map(|w| w.to_string())
        .collect()
}

fn ngrams(text: &str, n: usize) -> HashSet<String> {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    if chars.len() < n {
        return HashSet::new();
    }
    chars.windows(n).map(|w| w.iter().collect()).collect()
}

fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn jaccard_similarity_set(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    jaccard_similarity(a, b)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_record(
        task_id: &str,
        task_type: &str,
        desc: &str,
        tokens: u64,
        duration: u64,
    ) -> TaskRecord {
        TaskRecord::new(
            task_id,
            "agent-1",
            task_type,
            desc,
            TaskOutcome::Success,
            duration,
        )
        .with_tokens(tokens)
    }

    #[test]
    fn test_predictor_empty() {
        let predictor = CostPredictor::new(PredictorConfig::default());
        let pred = predictor.predict("do something", "test");
        assert!(pred.estimated_tokens.is_none());
        assert_eq!(pred.similar_task_count, 0);
    }

    #[test]
    fn test_predictor_exact_match() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        predictor.ingest(make_record(
            "t1",
            "code-review",
            "Review PR #42",
            1200,
            5000,
        ));
        predictor.ingest(make_record(
            "t2",
            "code-review",
            "Review PR #100",
            1500,
            6000,
        ));

        let pred = predictor.predict("Review PR #200", "code-review");
        assert!(pred.estimated_tokens.is_some());
        assert!(pred.similar_task_count > 0);
        assert!(pred.token_confidence > 0.0);
    }

    #[test]
    fn test_predictor_type_match_boost() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        predictor.ingest(make_record(
            "t1",
            "deploy",
            "Deploy to production",
            5000,
            30000,
        ));
        predictor.ingest(make_record(
            "t2",
            "test",
            "Deploy test environment",
            2000,
            10000,
        ));

        let pred = predictor.predict("Deploy service", "deploy");
        // Type-matched task should dominate
        assert!(pred.estimated_tokens.unwrap() > 3000);
    }

    #[test]
    fn test_predictor_with_failures() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        predictor.ingest(make_record(
            "t1",
            "build",
            "Compile Rust project",
            3000,
            20000,
        ));
        predictor.ingest(
            TaskRecord::new(
                "t2",
                "agent-1",
                "build",
                "Compile Rust project",
                TaskOutcome::Failure,
                5000,
            )
            .with_tokens(500),
        );
        predictor.ingest(make_record(
            "t3",
            "build",
            "Compile Rust project",
            3200,
            22000,
        ));

        let pred = predictor.predict("Compile Rust code", "build");
        assert!(pred.success_rate < 1.0);
        assert!(pred.success_rate > 0.0);
    }

    #[test]
    fn test_predictor_stats() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        predictor.ingest(make_record("t1", "a", "task 1", 100, 1000));
        predictor.ingest(make_record("t2", "b", "task 2", 200, 2000));

        let stats = predictor.stats();
        assert_eq!(stats.total_ingested, 2);
        assert_eq!(stats.avg_tokens, 150);
        assert_eq!(stats.avg_duration_ms, 1500);
    }

    #[test]
    fn test_predictor_clear() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        predictor.ingest(make_record("t1", "a", "task 1", 100, 1000));
        predictor.clear();

        let stats = predictor.stats();
        assert_eq!(stats.total_ingested, 0);
    }

    #[test]
    fn test_jaccard_similarity() {
        let a: HashSet<String> = vec!["hello", "world"]
            .into_iter()
            .map(String::from)
            .collect();
        let b: HashSet<String> = vec!["hello", "rust"]
            .into_iter()
            .map(String::from)
            .collect();
        let sim = jaccard_similarity(&a, &b);
        assert!((sim - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_jaccard_identical() {
        let a: HashSet<String> = vec!["hello", "world"]
            .into_iter()
            .map(String::from)
            .collect();
        let sim = jaccard_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_jaccard_empty() {
        let a: HashSet<String> = HashSet::new();
        let sim = jaccard_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_ngrams() {
        let grams = ngrams("hello", 2);
        assert!(grams.contains("he"));
        assert!(grams.contains("el"));
        assert!(grams.contains("ll"));
        assert!(grams.contains("lo"));
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Review PR #42 for deployment");
        assert!(tokens.contains("review"));
        assert!(tokens.contains("pr"));
        assert!(tokens.contains("deployment"));
        assert!(tokens.contains("for")); // 3 chars > 1
    }

    #[test]
    fn test_predictor_batch_ingest() {
        let mut predictor = CostPredictor::new(PredictorConfig::default());
        let records = vec![
            make_record("t1", "test", "Run unit tests", 500, 3000),
            make_record("t2", "test", "Run integration tests", 800, 5000),
            make_record("t3", "test", "Run E2E tests", 1200, 10000),
        ];
        predictor.ingest_batch(records);
        assert_eq!(predictor.stats().total_ingested, 3);
    }

    #[test]
    fn test_predictor_no_similar_tasks() {
        let mut predictor = CostPredictor::new(PredictorConfig {
            min_similarity: 0.99, // Very high threshold
            ..Default::default()
        });
        predictor.ingest(make_record("t1", "deploy", "Deploy to prod", 5000, 30000));

        let pred = predictor.predict("Completely different task", "test");
        assert!(pred.estimated_tokens.is_none());
        assert_eq!(pred.similar_task_count, 0);
    }
}
