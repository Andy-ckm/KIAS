//! # Agent Self-Distillation
//!
//! 自动从 Agent 执行历史中提取可复用 Skill。
//!
//! ## 自蒸馏流程（自进化链路核心环节）
//!
//! ```text
//! 执行日志 → 模式检测 → Skill 生成 → 验证 → 入库
//! ```
//!
//! ## 设计原则（钱学森系统工程）
//!
//! 1. **数据驱动**：从真实执行数据中提取，不是凭空创造
//! 2. **频率为王**：重复出现的模式更值得抽象为 Skill
//! 3. **渐进式**：先提取简单模式，逐步增加复杂度
//! 4. **可验证**：生成的 Skill 必须通过测试

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// A recorded execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Unique record ID
    pub id: String,
    /// Agent that performed the action
    pub agent_id: String,
    /// Tool/skill that was called
    pub tool: String,
    /// Input parameters
    pub input: serde_json::Value,
    /// Output result
    pub output: serde_json::Value,
    /// Whether the execution succeeded
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// When it happened
    pub timestamp: SystemTime,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// A detected pattern in execution history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPattern {
    /// Pattern ID
    pub id: String,
    /// Tool sequence (tool names in order)
    pub sequence: Vec<String>,
    /// How many times this pattern was observed
    pub frequency: usize,
    /// Average success rate
    pub success_rate: f64,
    /// Average total duration
    pub avg_duration_ms: u64,
    /// Example executions that match this pattern
    pub examples: Vec<String>,
    /// Detected parameter mappings (common input → output chains)
    pub data_flow: Vec<DataFlowEdge>,
}

/// Data flow between steps in a pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFlowEdge {
    /// Source step index
    pub from_step: usize,
    /// Source field path (e.g., "output.files")
    pub from_field: String,
    /// Target step index
    pub to_step: usize,
    /// Target field path (e.g., "input.content")
    pub to_field: String,
}

/// A distilled skill candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledSkill {
    /// Skill name (auto-generated)
    pub name: String,
    /// Description
    pub description: String,
    /// Source pattern
    pub pattern_id: String,
    /// Skill definition (YAML-serializable)
    pub definition: SkillDefinition,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Test cases extracted from examples
    pub test_cases: Vec<TestCase>,
    /// Whether this skill has been validated
    pub validated: bool,
}

/// Auto-generated skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// Skill type (composite, tool, llm)
    pub skill_type: String,
    /// Steps in the skill
    pub steps: Vec<SkillStep>,
    /// Input schema
    pub inputs: HashMap<String, String>,
    /// Output schema
    pub outputs: HashMap<String, String>,
}

/// A step in a distilled skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    /// Step name
    pub name: String,
    /// Tool to call
    pub tool: String,
    /// Input mapping (from previous steps or skill inputs)
    pub input_mapping: HashMap<String, String>,
    /// Expected output fields
    pub output_fields: Vec<String>,
}

/// Test case for a distilled skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    /// Test name
    pub name: String,
    /// Input values
    pub inputs: serde_json::Value,
    /// Expected output (from real execution)
    pub expected_output: serde_json::Value,
}

/// Configuration for the distillation process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistillationConfig {
    /// Minimum pattern frequency to consider
    pub min_frequency: usize,
    /// Minimum success rate
    pub min_success_rate: f64,
    /// Minimum confidence to generate a skill
    pub min_confidence: f64,
    /// Maximum sequence length to detect
    pub max_sequence_length: usize,
    /// Minimum examples to include
    pub min_examples: usize,
}

impl Default for DistillationConfig {
    fn default() -> Self {
        Self {
            min_frequency: 3,
            min_success_rate: 0.8,
            min_confidence: 0.6,
            max_sequence_length: 10,
            min_examples: 2,
        }
    }
}

/// The distillation engine
pub struct DistillationEngine {
    config: DistillationConfig,
}

impl DistillationEngine {
    /// Create a new distillation engine
    pub fn new(config: DistillationConfig) -> Self {
        Self { config }
    }

    /// Detect sequential patterns from execution records
    pub fn detect_patterns(&self, records: &[ExecutionRecord]) -> Vec<ExecutionPattern> {
        let sequences = self.extract_sequences(records);
        let frequent = self.find_frequent_subsequences(&sequences);
        let patterns: Vec<ExecutionPattern> = frequent
            .into_iter()
            .filter(|p| {
                p.frequency >= self.config.min_frequency
                    && p.success_rate >= self.config.min_success_rate
                    && p.sequence.len() <= self.config.max_sequence_length
            })
            .collect();
        patterns
    }

    /// Extract tool sequences grouped by agent
    fn extract_sequences<'a>(
        &self,
        records: &'a [ExecutionRecord],
    ) -> Vec<Vec<&'a ExecutionRecord>> {
        let mut by_agent: HashMap<&str, Vec<&ExecutionRecord>> = HashMap::new();
        for record in records {
            by_agent.entry(&record.agent_id).or_default().push(record);
        }

        let mut sequences = Vec::new();
        for (_, mut agent_records) in by_agent {
            agent_records.sort_by_key(|r| r.timestamp);
            // Split into sessions (gaps > 5 minutes)
            let mut current_session: Vec<&ExecutionRecord> = Vec::new();
            for record in agent_records {
                if let Some(last) = current_session.last() {
                    if let (Ok(dur), _) = (record.timestamp.duration_since(last.timestamp), ()) {
                        if dur.as_secs() > 300 {
                            if current_session.len() >= 2 {
                                sequences.push(current_session);
                            }
                            current_session = Vec::new();
                        }
                    }
                }
                current_session.push(record);
            }
            if current_session.len() >= 2 {
                sequences.push(current_session);
            }
        }

        sequences
    }

    /// Find frequent subsequences using sliding window
    fn find_frequent_subsequences(
        &self,
        sequences: &[Vec<&ExecutionRecord>],
    ) -> Vec<ExecutionPattern> {
        let mut pattern_counts: HashMap<Vec<String>, PatternAccumulator> = HashMap::new();

        for session in sequences {
            let tools: Vec<String> = session.iter().map(|r| r.tool.clone()).collect();

            // Count all subsequences of length 2..=max
            for len in 2..=self.config.max_sequence_length.min(tools.len()) {
                for window in tools.windows(len) {
                    let key = window.to_vec();
                    let acc = pattern_counts.entry(key).or_default();
                    acc.count += 1;
                    acc.success_count += session
                        .iter()
                        .skip(acc.count - 1)
                        .take(len)
                        .filter(|r| r.success)
                        .count();
                    if acc.examples.len() < self.config.min_examples {
                        acc.examples.push(session[0].id.clone());
                    }
                }
            }
        }

        pattern_counts
            .into_iter()
            .map(|(sequence, acc)| {
                let total_steps = acc.count * sequence.len();
                let success_rate = if total_steps > 0 {
                    acc.success_count as f64 / total_steps as f64
                } else {
                    0.0
                };

                ExecutionPattern {
                    id: format!("pat-{}", Self::hash_sequence(&sequence)),
                    sequence: sequence.clone(),
                    frequency: acc.count,
                    success_rate,
                    avg_duration_ms: 0, // Would need more data
                    examples: acc.examples,
                    data_flow: Vec::new(), // Would need deeper analysis
                }
            })
            .collect()
    }

    /// Generate skill candidates from patterns
    pub fn distill(&self, patterns: &[ExecutionPattern]) -> Vec<DistilledSkill> {
        patterns
            .iter()
            .filter(|p| {
                p.frequency >= self.config.min_frequency
                    && p.success_rate >= self.config.min_success_rate
            })
            .map(|pattern| self.pattern_to_skill(pattern))
            .filter(|skill| skill.confidence >= self.config.min_confidence)
            .collect()
    }

    /// Convert a pattern to a skill definition
    fn pattern_to_skill(&self, pattern: &ExecutionPattern) -> DistilledSkill {
        let steps: Vec<SkillStep> = pattern
            .sequence
            .iter()
            .enumerate()
            .map(|(i, tool)| SkillStep {
                name: format!("step_{}", i),
                tool: tool.clone(),
                input_mapping: if i > 0 {
                    let mut map = HashMap::new();
                    map.insert(
                        "input".to_string(),
                        format!("${{{}.output}}", pattern.sequence[i - 1]),
                    );
                    map
                } else {
                    HashMap::new()
                },
                output_fields: vec!["output".to_string()],
            })
            .collect();

        let confidence = self.calculate_confidence(pattern);

        DistilledSkill {
            name: format!(
                "auto-{}",
                pattern
                    .sequence
                    .join("-")
                    .to_lowercase()
                    .replace(['_', '.'], "-")
            ),
            description: format!(
                "Auto-distilled from {} observations. Sequence: {}",
                pattern.frequency,
                pattern.sequence.join(" → ")
            ),
            pattern_id: pattern.id.clone(),
            definition: SkillDefinition {
                skill_type: "composite".to_string(),
                steps,
                inputs: HashMap::new(),
                outputs: HashMap::new(),
            },
            confidence,
            test_cases: Vec::new(),
            validated: false,
        }
    }

    /// Calculate confidence score
    fn calculate_confidence(&self, pattern: &ExecutionPattern) -> f64 {
        let freq_score = (pattern.frequency as f64).ln() / 10.0;
        let success_score = pattern.success_rate;
        let len_penalty = if pattern.sequence.len() > 5 {
            0.1 * (pattern.sequence.len() - 5) as f64
        } else {
            0.0
        };

        (0.4 * freq_score + 0.5 * success_score - len_penalty).clamp(0.0, 1.0)
    }

    /// Hash a sequence for ID generation
    fn hash_sequence(seq: &[String]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        seq.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

#[derive(Default)]
struct PatternAccumulator {
    count: usize,
    success_count: usize,
    examples: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_record(id: &str, agent: &str, tool: &str, success: bool) -> ExecutionRecord {
        ExecutionRecord {
            id: id.to_string(),
            agent_id: agent.to_string(),
            tool: tool.to_string(),
            input: serde_json::json!({}),
            output: serde_json::json!({"result": "ok"}),
            success,
            duration_ms: 100,
            timestamp: SystemTime::now(),
            tags: vec![],
        }
    }

    fn make_records_with_offset(
        id: &str,
        agent: &str,
        tool: &str,
        success: bool,
        offset_secs: u64,
    ) -> ExecutionRecord {
        ExecutionRecord {
            id: id.to_string(),
            agent_id: agent.to_string(),
            tool: tool.to_string(),
            input: serde_json::json!({}),
            output: serde_json::json!({"result": "ok"}),
            success,
            duration_ms: 100,
            timestamp: SystemTime::now() + Duration::from_secs(offset_secs),
            tags: vec![],
        }
    }

    #[test]
    fn test_distillation_config_default() {
        let config = DistillationConfig::default();
        assert_eq!(config.min_frequency, 3);
        assert_eq!(config.min_success_rate, 0.8);
        assert!(config.max_sequence_length > 0);
    }

    #[test]
    fn test_detect_simple_pattern() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 2,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        // Same agent does [read, lint, format] 3 times
        let mut records = Vec::new();
        for i in 0..3 {
            let base = i * 100;
            records.push(make_records_with_offset(
                &format!("r{}-1", i),
                "agent-1",
                "file.read",
                true,
                base,
            ));
            records.push(make_records_with_offset(
                &format!("r{}-2", i),
                "agent-1",
                "code.lint",
                true,
                base + 1,
            ));
            records.push(make_records_with_offset(
                &format!("r{}-3", i),
                "agent-1",
                "code.format",
                true,
                base + 2,
            ));
        }

        let patterns = engine.detect_patterns(&records);
        assert!(!patterns.is_empty());

        // Should find [file.read, code.lint, code.format] pattern
        let full_pattern = patterns
            .iter()
            .find(|p| p.sequence == vec!["file.read", "code.lint", "code.format"]);
        assert!(full_pattern.is_some());
        assert_eq!(full_pattern.unwrap().frequency, 3);
    }

    #[test]
    fn test_distill_generates_skill() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 2,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        let pattern = ExecutionPattern {
            id: "test-pat".to_string(),
            sequence: vec!["file.read".to_string(), "code.lint".to_string()],
            frequency: 5,
            success_rate: 0.95,
            avg_duration_ms: 200,
            examples: vec!["ex1".to_string()],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        assert_eq!(skills.len(), 1);
        assert!(skills[0].name.contains("file-read"));
        assert_eq!(skills[0].definition.steps.len(), 2);
        assert!(skills[0].confidence > 0.0);
    }

    #[test]
    fn test_low_frequency_filtered() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 10,
            ..Default::default()
        });

        let pattern = ExecutionPattern {
            id: "rare".to_string(),
            sequence: vec!["a".to_string()],
            frequency: 2,
            success_rate: 1.0,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        assert!(skills.is_empty());
    }

    #[test]
    fn test_low_success_rate_filtered() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 1,
            min_success_rate: 0.9,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        let pattern = ExecutionPattern {
            id: "unreliable".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            frequency: 5,
            success_rate: 0.3,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        assert!(skills.is_empty());
    }

    #[test]
    fn test_skill_step_mapping() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 1,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        let pattern = ExecutionPattern {
            id: "chain".to_string(),
            sequence: vec![
                "fetch".to_string(),
                "process".to_string(),
                "save".to_string(),
            ],
            frequency: 10,
            success_rate: 0.95,
            avg_duration_ms: 300,
            examples: vec![],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        let skill = &skills[0];

        // First step has no input mapping
        assert!(skill.definition.steps[0].input_mapping.is_empty());

        // Second step maps from first
        assert!(skill.definition.steps[1]
            .input_mapping
            .contains_key("input"));

        // Third step maps from second
        assert!(skill.definition.steps[2]
            .input_mapping
            .contains_key("input"));
    }

    #[test]
    fn test_empty_records() {
        let engine = DistillationEngine::new(DistillationConfig::default());
        let patterns = engine.detect_patterns(&[]);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_single_record_no_pattern() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 1,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        let records = vec![make_record("r1", "a1", "tool.x", true)];
        let patterns = engine.detect_patterns(&records);
        // Single record can't form a sequence of length >= 2
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_confidence_calculation() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 1,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 10,
            min_examples: 1,
        });

        // High frequency, high success → high confidence
        let good = ExecutionPattern {
            id: "good".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            frequency: 20,
            success_rate: 0.95,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        // Low frequency, low success → low confidence
        let bad = ExecutionPattern {
            id: "bad".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            frequency: 2,
            success_rate: 0.5,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        let good_skills = engine.distill(&[good]);
        let bad_skills = engine.distill(&[bad]);

        if !good_skills.is_empty() && !bad_skills.is_empty() {
            assert!(good_skills[0].confidence > bad_skills[0].confidence);
        }
    }

    #[test]
    fn test_session_gap_splitting() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 2,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        // Two sessions with a gap > 5 minutes
        let records = vec![
            make_records_with_offset("r1", "a1", "tool.a", true, 0),
            make_records_with_offset("r2", "a1", "tool.b", true, 1),
            // 10 minute gap
            make_records_with_offset("r3", "a1", "tool.a", true, 600),
            make_records_with_offset("r4", "a1", "tool.b", true, 601),
        ];

        let patterns = engine.detect_patterns(&records);
        // Should detect [tool.a, tool.b] pattern from 2 sessions
        let ab_pattern = patterns
            .iter()
            .find(|p| p.sequence == vec!["tool.a", "tool.b"]);
        assert!(ab_pattern.is_some());
        assert_eq!(ab_pattern.unwrap().frequency, 2);
    }

    #[test]
    fn test_hash_sequence_deterministic() {
        let seq1 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let seq2 = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let seq3 = vec!["x".to_string(), "y".to_string()];

        // Same sequence should produce same hash
        assert_eq!(
            DistillationEngine::hash_sequence(&seq1),
            DistillationEngine::hash_sequence(&seq2)
        );
        // Different sequences should produce different hashes
        assert_ne!(
            DistillationEngine::hash_sequence(&seq1),
            DistillationEngine::hash_sequence(&seq3)
        );
    }

    #[test]
    fn test_distill_filters_low_frequency() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 5,
            min_success_rate: 0.5,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        // Pattern with frequency 3 should be filtered out (min_frequency is 5)
        let pattern = ExecutionPattern {
            id: "low-freq".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            frequency: 3,
            success_rate: 0.9,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        assert!(skills.is_empty());
    }

    #[test]
    fn test_distill_filters_low_success_rate() {
        let engine = DistillationEngine::new(DistillationConfig {
            min_frequency: 2,
            min_success_rate: 0.8,
            min_confidence: 0.1,
            max_sequence_length: 5,
            min_examples: 1,
        });

        // Pattern with success_rate 0.5 should be filtered out (min_success_rate is 0.8)
        let pattern = ExecutionPattern {
            id: "low-success".to_string(),
            sequence: vec!["a".to_string(), "b".to_string()],
            frequency: 10,
            success_rate: 0.5,
            avg_duration_ms: 100,
            examples: vec![],
            data_flow: vec![],
        };

        let skills = engine.distill(&[pattern]);
        assert!(skills.is_empty());
    }
}
