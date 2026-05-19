//! # DataQuality Pipeline — 数据质量管道
//!
//! 私有化 AI 的核心竞争力：精选数据 → 高准确率 → 高信任度。
//!
//! ## 质量闭环
//!
//! ```text
//! 原始数据 → 过滤(去重/去噪) → 标注(自动/人工) → 验证(交叉对比)
//!     ↓                                                    ↓
//! 知识库 ←──────────── 质量评分 ←──────────── 执行验证 ←──┘
//!     ↓
//! Agent 使用 → 更准确 → 更多信任 → 更多数据 → 更好
//! ```
//!
//! ## 设计原则
//!
//! 1. **质量可量化**: 每条知识都有 quality_score (0.0 - 1.0)
//! 2. **来源可追溯**: 每条知识记录来源、验证历史
//! 3. **交叉验证**: 多 Agent 输出比对，不一致触发审核
//! 4. **时间衰减**: 旧知识自然降权，强制持续验证
//! 5. **正向循环**: 采纳提升权重，拒绝降低权重

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

// ─── Quality Scoring ────────────────────────────────────────────────────

/// 质量评分引擎
#[derive(Debug, Clone)]
pub struct QualityScorer {
    /// 评分权重配置
    pub weights: QualityWeights,
}

/// 质量评分的各维度权重
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityWeights {
    /// 来源可信度权重
    pub source_trust: f64,
    /// 交叉验证权重
    pub cross_validation: f64,
    /// 用户反馈权重
    pub user_feedback: f64,
    /// 时间衰减权重
    pub time_decay: f64,
    /// 执行成功率权重
    pub execution_success: f64,
}

impl Default for QualityWeights {
    fn default() -> Self {
        Self {
            source_trust: 0.2,
            cross_validation: 0.25,
            user_feedback: 0.3,
            time_decay: 0.1,
            execution_success: 0.15,
        }
    }
}

impl Default for QualityScorer {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityScorer {
    pub fn new() -> Self {
        Self {
            weights: QualityWeights::default(),
        }
    }

    pub fn with_weights(mut self, weights: QualityWeights) -> Self {
        self.weights = weights;
        self
    }

    /// 计算综合质量分
    pub fn compute_score(&self, entry: &KnowledgeEntry) -> f64 {
        let source = entry.source_trust_score;
        let cross_val = entry.cross_validation_score;
        let feedback = entry.user_feedback_score;
        let time = self.time_decay_factor(entry.created_at, entry.last_validated_at);
        let execution = entry.execution_success_rate;

        let raw = source * self.weights.source_trust
            + cross_val * self.weights.cross_validation
            + feedback * self.weights.user_feedback
            + time * self.weights.time_decay
            + execution * self.weights.execution_success;

        raw.clamp(0.0, 1.0)
    }

    /// 时间衰减因子：越久没验证，分数越低
    fn time_decay_factor(&self, _created: SystemTime, last_validated: SystemTime) -> f64 {
        let elapsed = last_validated.elapsed().unwrap_or(Duration::from_secs(0));
        let days = elapsed.as_secs() as f64 / 86400.0;

        // 30天内不衰减，之后每天衰减 1%
        if days <= 30.0 {
            1.0
        } else {
            (1.0 - (days - 30.0) * 0.01).max(0.1)
        }
    }
}

// ─── Knowledge Entry ────────────────────────────────────────────────────

/// 知识条目——质量管道的基本单位
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeEntry {
    /// 唯一标识
    pub id: String,
    /// 知识内容
    pub content: String,
    /// 知识类型
    pub entry_type: KnowledgeType,
    /// 标签
    pub tags: Vec<String>,
    /// 来源
    pub source: KnowledgeSource,
    /// 来源可信度 (0.0 - 1.0)
    pub source_trust_score: f64,
    /// 交叉验证分数 (0.0 - 1.0)
    pub cross_validation_score: f64,
    /// 用户反馈分数 (0.0 - 1.0)
    pub user_feedback_score: f64,
    /// 执行成功率 (0.0 - 1.0)
    pub execution_success_rate: f64,
    /// 综合质量分（由 QualityScorer 计算）
    pub quality_score: f64,
    /// 采纳次数
    pub adoption_count: u32,
    /// 拒绝次数
    pub rejection_count: u32,
    /// 交叉验证次数
    pub validation_count: u32,
    /// 交叉验证成功次数
    pub validation_success_count: u32,
    /// 创建时间
    pub created_at: SystemTime,
    /// 最后验证时间
    pub last_validated_at: SystemTime,
    /// 最后使用时间
    pub last_used_at: Option<SystemTime>,
    /// 是否为负面样本（导致过问题的知识）
    pub is_negative: bool,
}

/// 知识类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeType {
    /// 代码片段
    Code,
    /// 配置模板
    Config,
    /// 架构决策
    Architecture,
    /// 调试经验
    Debugging,
    /// 最佳实践
    BestPractice,
    /// 错误模式（应避免的）
    AntiPattern,
    /// 业务规则
    BusinessRule,
    /// 领域知识
    Domain,
}

/// 知识来源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KnowledgeSource {
    /// 用户手动输入
    UserInput,
    /// Agent 自动生成
    AgentGenerated,
    /// 从文档提取
    DocumentExtract,
    /// 从代码提取
    CodeExtract,
    /// 交叉验证得出
    CrossValidated,
    /// 外部参考
    ExternalReference { url: String },
}

// ─── Cross Validation ───────────────────────────────────────────────────

/// 交叉验证引擎
///
/// 多个 Agent 输出比对，不一致触发审核
#[derive(Debug)]
pub struct CrossValidator {
    /// 验证历史
    pub validations: Vec<ValidationRecord>,
}

/// 验证记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRecord {
    /// 知识条目 ID
    pub entry_id: String,
    /// 验证 Agent 列表
    pub validators: Vec<String>,
    /// 各 Agent 的输出
    pub outputs: Vec<AgentOutput>,
    /// 是否一致
    pub consistent: bool,
    /// 一致性分数 (0.0 - 1.0)
    pub consistency_score: f64,
    /// 验证时间
    pub validated_at: SystemTime,
    /// 最终结论
    pub conclusion: ValidationConclusion,
}

/// Agent 输出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutput {
    pub agent_id: String,
    pub output: String,
    pub confidence: f64,
}

/// 验证结论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationConclusion {
    /// 多数一致，采纳
    Accepted { consensus: String },
    /// 不一致，需要人工审核
    NeedsHumanReview { reason: String },
    /// 全部一致，高置信度
    HighConfidence { consensus: String },
    /// 全部不一致，标记为可疑
    Suspicious { reason: String },
}

impl Default for CrossValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossValidator {
    pub fn new() -> Self {
        Self {
            validations: Vec::new(),
        }
    }

    /// 执行交叉验证
    pub fn validate(&mut self, entry_id: &str, outputs: Vec<AgentOutput>) -> ValidationRecord {
        let consistent = self.check_consistency(&outputs);
        let consistency_score = self.compute_consistency_score(&outputs);

        let conclusion = if outputs.len() < 2 {
            ValidationConclusion::NeedsHumanReview {
                reason: "验证者不足 2 个".to_string(),
            }
        } else if consistency_score >= 0.9 {
            ValidationConclusion::HighConfidence {
                consensus: outputs[0].output.clone(),
            }
        } else if consistency_score >= 0.6 {
            let consensus = self.find_consensus(&outputs);
            ValidationConclusion::Accepted { consensus }
        } else if consistency_score >= 0.3 {
            ValidationConclusion::NeedsHumanReview {
                reason: format!("一致性分数过低: {:.2}", consistency_score),
            }
        } else {
            ValidationConclusion::Suspicious {
                reason: "所有验证者输出不一致".to_string(),
            }
        };

        let record = ValidationRecord {
            entry_id: entry_id.to_string(),
            validators: outputs.iter().map(|o| o.agent_id.clone()).collect(),
            outputs,
            consistent,
            consistency_score,
            validated_at: SystemTime::now(),
            conclusion,
        };

        self.validations.push(record.clone());
        record
    }

    /// 检查输出是否一致（所有输出相同）
    fn check_consistency(&self, outputs: &[AgentOutput]) -> bool {
        if outputs.len() < 2 {
            return false;
        }
        let first = &outputs[0].output;
        outputs.iter().all(|o| &o.output == first)
    }

    /// 计算一致性分数（基于输出相似度）
    fn compute_consistency_score(&self, outputs: &[AgentOutput]) -> f64 {
        if outputs.len() < 2 {
            return 0.0;
        }

        let mut match_count = 0;
        let mut total_pairs = 0;

        for i in 0..outputs.len() {
            for j in (i + 1)..outputs.len() {
                total_pairs += 1;
                let similarity = self.text_similarity(&outputs[i].output, &outputs[j].output);
                // 加权：高置信度的输出权重更大
                let weight = (outputs[i].confidence + outputs[j].confidence) / 2.0;
                if similarity > 0.8 {
                    match_count += 1;
                } else if similarity > 0.5 {
                    match_count += (0.5 * weight) as i32;
                }
            }
        }

        if total_pairs == 0 {
            0.0
        } else {
            match_count as f64 / total_pairs as f64
        }
    }

    /// 简单文本相似度（Jaccard）
    fn text_similarity(&self, a: &str, b: &str) -> f64 {
        let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();

        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();

        if union == 0 {
            0.0
        } else {
            intersection as f64 / union as f64
        }
    }

    /// 找到多数共识
    fn find_consensus(&self, outputs: &[AgentOutput]) -> String {
        // 按置信度加权投票
        let mut best = &outputs[0];
        for output in outputs.iter().skip(1) {
            if output.confidence > best.confidence {
                best = output;
            }
        }
        best.output.clone()
    }
}

// ─── Quality Pipeline ───────────────────────────────────────────────────

/// 数据质量管道——管理知识的全生命周期
pub struct QualityPipeline {
    /// 知识库
    pub entries: HashMap<String, KnowledgeEntry>,
    /// 质量评分器
    pub scorer: QualityScorer,
    /// 交叉验证器
    pub validator: CrossValidator,
    /// 统计
    pub stats: PipelineStats,
}

/// 管道统计
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineStats {
    pub total_entries: usize,
    pub high_quality_count: usize,   // score >= 0.8
    pub medium_quality_count: usize, // 0.5 <= score < 0.8
    pub low_quality_count: usize,    // score < 0.5
    pub negative_count: usize,
    pub total_validations: usize,
    pub total_adoptions: u64,
    pub total_rejections: u64,
    pub average_quality_score: f64,
}

impl Default for QualityPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl QualityPipeline {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            scorer: QualityScorer::new(),
            validator: CrossValidator::new(),
            stats: PipelineStats::default(),
        }
    }

    /// 添加知识条目
    pub fn add_entry(&mut self, mut entry: KnowledgeEntry) -> String {
        entry.quality_score = self.scorer.compute_score(&entry);
        let id = entry.id.clone();
        self.entries.insert(id.clone(), entry);
        self.update_stats();
        id
    }

    /// 记录用户采纳
    pub fn record_adoption(&mut self, entry_id: &str) {
        if let Some(entry) = self.entries.get_mut(entry_id) {
            entry.adoption_count += 1;
            entry.user_feedback_score = (entry.user_feedback_score + 0.1).min(1.0);
            entry.last_used_at = Some(SystemTime::now());
            entry.quality_score = self.scorer.compute_score(entry);
            self.stats.total_adoptions += 1;
            self.update_stats();
        }
    }

    /// 记录用户拒绝
    pub fn record_rejection(&mut self, entry_id: &str) {
        if let Some(entry) = self.entries.get_mut(entry_id) {
            entry.rejection_count += 1;
            entry.user_feedback_score = (entry.user_feedback_score - 0.2).max(0.0);
            entry.quality_score = self.scorer.compute_score(entry);
            self.stats.total_rejections += 1;
            self.update_stats();
        }
    }

    /// 记录执行结果
    pub fn record_execution(&mut self, entry_id: &str, success: bool) {
        if let Some(entry) = self.entries.get_mut(entry_id) {
            let total = entry.adoption_count + entry.rejection_count + 1;
            if success {
                entry.execution_success_rate =
                    (entry.execution_success_rate * (total - 1) as f64 + 1.0) / total as f64;
            } else {
                entry.execution_success_rate =
                    (entry.execution_success_rate * (total - 1) as f64) / total as f64;
            }
            entry.quality_score = self.scorer.compute_score(entry);
            self.update_stats();
        }
    }

    /// 标记为负面样本
    pub fn mark_negative(&mut self, entry_id: &str, reason: &str) {
        if let Some(entry) = self.entries.get_mut(entry_id) {
            entry.is_negative = true;
            entry.quality_score = 0.0;
            tracing::warn!(
                entry_id = entry_id,
                reason = reason,
                "Knowledge entry marked as negative sample"
            );
            self.update_stats();
        }
    }

    /// 执行交叉验证
    pub fn cross_validate(
        &mut self,
        entry_id: &str,
        outputs: Vec<AgentOutput>,
    ) -> ValidationRecord {
        let record = self.validator.validate(entry_id, outputs);

        if let Some(entry) = self.entries.get_mut(entry_id) {
            entry.validation_count += 1;
            match &record.conclusion {
                ValidationConclusion::HighConfidence { .. }
                | ValidationConclusion::Accepted { .. } => {
                    entry.validation_success_count += 1;
                    entry.cross_validation_score = (entry.cross_validation_score + 0.15).min(1.0);
                }
                ValidationConclusion::Suspicious { .. } => {
                    entry.cross_validation_score = (entry.cross_validation_score - 0.3).max(0.0);
                }
                _ => {}
            }
            entry.last_validated_at = SystemTime::now();
            entry.quality_score = self.scorer.compute_score(entry);
        }

        self.stats.total_validations += 1;
        self.update_stats();
        record
    }

    /// 获取高质量知识（score >= threshold）
    pub fn get_high_quality(&self, threshold: f64) -> Vec<&KnowledgeEntry> {
        let mut entries: Vec<&KnowledgeEntry> = self
            .entries
            .values()
            .filter(|e| e.quality_score >= threshold && !e.is_negative)
            .collect();
        entries.sort_by(|a, b| {
            b.quality_score
                .partial_cmp(&a.quality_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries
    }

    /// 按标签搜索高质量知识
    pub fn search_by_tag(&self, tag: &str, min_quality: f64) -> Vec<&KnowledgeEntry> {
        self.entries
            .values()
            .filter(|e| {
                e.tags.contains(&tag.to_string())
                    && e.quality_score >= min_quality
                    && !e.is_negative
            })
            .collect()
    }

    /// 清理低质量条目
    pub fn cleanup_low_quality(&mut self, threshold: f64) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| e.quality_score >= threshold || e.is_negative);
        let removed = before - self.entries.len();
        if removed > 0 {
            self.update_stats();
            tracing::info!(removed = removed, "Cleaned up low quality entries");
        }
        removed
    }

    /// 更新统计
    fn update_stats(&mut self) {
        let entries: Vec<&KnowledgeEntry> = self.entries.values().collect();
        let total = entries.len();

        self.stats.total_entries = total;
        self.stats.high_quality_count = entries.iter().filter(|e| e.quality_score >= 0.8).count();
        self.stats.medium_quality_count = entries
            .iter()
            .filter(|e| e.quality_score >= 0.5 && e.quality_score < 0.8)
            .count();
        self.stats.low_quality_count = entries.iter().filter(|e| e.quality_score < 0.5).count();
        self.stats.negative_count = entries.iter().filter(|e| e.is_negative).count();

        if total > 0 {
            self.stats.average_quality_score =
                entries.iter().map(|e| e.quality_score).sum::<f64>() / total as f64;
        }
    }

    /// 获取统计
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// 生成质量报告
    pub fn generate_report(&self) -> String {
        let s = &self.stats;
        format!(
            "AgentGuard 数据质量报告\n\
             ════════════════\n\
             总知识条目: {}\n\
             高质量 (≥0.8): {} ({:.1}%)\n\
             中质量 (0.5-0.8): {} ({:.1}%)\n\
             低质量 (<0.5): {} ({:.1}%)\n\
             负面样本: {}\n\
             平均质量分: {:.3}\n\
             总采纳: {}\n\
             总拒绝: {}\n\
             总交叉验证: {}",
            s.total_entries,
            s.high_quality_count,
            pct(s.high_quality_count, s.total_entries),
            s.medium_quality_count,
            pct(s.medium_quality_count, s.total_entries),
            s.low_quality_count,
            pct(s.low_quality_count, s.total_entries),
            s.negative_count,
            s.average_quality_score,
            s.total_adoptions,
            s.total_rejections,
            s.total_validations,
        )
    }
}

fn pct(part: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: &str, tags: Vec<&str>) -> KnowledgeEntry {
        KnowledgeEntry {
            id: id.to_string(),
            content: "test content".to_string(),
            entry_type: KnowledgeType::Code,
            tags: tags.into_iter().map(|s| s.to_string()).collect(),
            source: KnowledgeSource::AgentGenerated,
            source_trust_score: 0.8,
            cross_validation_score: 0.5,
            user_feedback_score: 0.5,
            execution_success_rate: 0.5,
            quality_score: 0.0,
            adoption_count: 0,
            rejection_count: 0,
            validation_count: 0,
            validation_success_count: 0,
            created_at: SystemTime::now(),
            last_validated_at: SystemTime::now(),
            last_used_at: None,
            is_negative: false,
        }
    }

    #[test]
    fn test_quality_scorer_basic() {
        let scorer = QualityScorer::new();
        let entry = make_entry("e1", vec!["rust"]);
        let score = scorer.compute_score(&entry);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_quality_scorer_weights() {
        let weights = QualityWeights {
            source_trust: 0.5,
            cross_validation: 0.2,
            user_feedback: 0.1,
            time_decay: 0.1,
            execution_success: 0.1,
        };
        let scorer = QualityScorer::new().with_weights(weights);
        let entry = make_entry("e1", vec![]);
        let score = scorer.compute_score(&entry);
        // Higher source_trust weight should make score closer to source_trust_score
        assert!(score > 0.3);
    }

    #[test]
    fn test_add_entry() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec!["rust"]);
        pipeline.add_entry(entry);
        assert_eq!(pipeline.stats().total_entries, 1);
        assert!(pipeline.entries.get("e1").unwrap().quality_score > 0.0);
    }

    #[test]
    fn test_adoption_improves_score() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);
        let before = pipeline.entries.get("e1").unwrap().quality_score;

        pipeline.record_adoption("e1");
        let after = pipeline.entries.get("e1").unwrap().quality_score;
        assert!(after > before);
    }

    #[test]
    fn test_rejection_degrades_score() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);
        let before = pipeline.entries.get("e1").unwrap().quality_score;

        pipeline.record_rejection("e1");
        let after = pipeline.entries.get("e1").unwrap().quality_score;
        assert!(after < before);
    }

    #[test]
    fn test_execution_success_improves_rate() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);

        pipeline.record_execution("e1", true);
        pipeline.record_execution("e1", true);
        pipeline.record_execution("e1", true);

        let rate = pipeline.entries.get("e1").unwrap().execution_success_rate;
        assert!(rate > 0.5);
    }

    #[test]
    fn test_mark_negative() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);

        pipeline.mark_negative("e1", "caused crash");
        let entry = pipeline.entries.get("e1").unwrap();
        assert!(entry.is_negative);
        assert_eq!(entry.quality_score, 0.0);
        assert_eq!(pipeline.stats().negative_count, 1);
    }

    #[test]
    fn test_cross_validation_high_confidence() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);

        let outputs = vec![
            AgentOutput {
                agent_id: "a1".to_string(),
                output: "result is 42".to_string(),
                confidence: 0.9,
            },
            AgentOutput {
                agent_id: "a2".to_string(),
                output: "result is 42".to_string(),
                confidence: 0.95,
            },
            AgentOutput {
                agent_id: "a3".to_string(),
                output: "result is 42".to_string(),
                confidence: 0.85,
            },
        ];

        let record = pipeline.cross_validate("e1", outputs);
        assert!(matches!(
            record.conclusion,
            ValidationConclusion::HighConfidence { .. }
        ));

        let entry = pipeline.entries.get("e1").unwrap();
        assert!(entry.cross_validation_score > 0.5);
        assert_eq!(entry.validation_count, 1);
        assert_eq!(entry.validation_success_count, 1);
    }

    #[test]
    fn test_cross_validation_suspicious() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);

        let outputs = vec![
            AgentOutput {
                agent_id: "a1".to_string(),
                output: "answer is A".to_string(),
                confidence: 0.9,
            },
            AgentOutput {
                agent_id: "a2".to_string(),
                output: "answer is B completely different".to_string(),
                confidence: 0.8,
            },
            AgentOutput {
                agent_id: "a3".to_string(),
                output: "answer is C totally wrong".to_string(),
                confidence: 0.7,
            },
        ];

        let record = pipeline.cross_validate("e1", outputs);
        assert!(matches!(
            record.conclusion,
            ValidationConclusion::Suspicious { .. }
        ));
    }

    #[test]
    fn test_cross_validation_needs_review() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);

        // Only 1 validator
        let outputs = vec![AgentOutput {
            agent_id: "a1".to_string(),
            output: "result".to_string(),
            confidence: 0.5,
        }];

        let record = pipeline.cross_validate("e1", outputs);
        assert!(matches!(
            record.conclusion,
            ValidationConclusion::NeedsHumanReview { .. }
        ));
    }

    #[test]
    fn test_get_high_quality() {
        let mut pipeline = QualityPipeline::new();

        let mut e1 = make_entry("e1", vec![]);
        e1.source_trust_score = 0.95;
        e1.cross_validation_score = 0.95;
        e1.user_feedback_score = 0.95;
        e1.execution_success_rate = 0.95;
        pipeline.add_entry(e1);

        let mut e2 = make_entry("e2", vec![]);
        e2.source_trust_score = 0.1;
        e2.cross_validation_score = 0.1;
        e2.user_feedback_score = 0.1;
        e2.execution_success_rate = 0.1;
        pipeline.add_entry(e2);

        let high = pipeline.get_high_quality(0.8);
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].id, "e1");
    }

    #[test]
    fn test_search_by_tag() {
        let mut pipeline = QualityPipeline::new();
        pipeline.add_entry(make_entry("e1", vec!["rust", "code"]));
        pipeline.add_entry(make_entry("e2", vec!["python", "data"]));
        pipeline.add_entry(make_entry("e3", vec!["rust", "test"]));

        let rust_entries = pipeline.search_by_tag("rust", 0.0);
        assert_eq!(rust_entries.len(), 2);
    }

    #[test]
    fn test_cleanup_low_quality() {
        let mut pipeline = QualityPipeline::new();

        let mut e1 = make_entry("good", vec![]);
        e1.source_trust_score = 0.9;
        e1.user_feedback_score = 0.9;
        pipeline.add_entry(e1);

        let mut e2 = make_entry("bad", vec![]);
        e2.source_trust_score = 0.05;
        e2.user_feedback_score = 0.05;
        e2.execution_success_rate = 0.05;
        pipeline.add_entry(e2);

        let removed = pipeline.cleanup_low_quality(0.3);
        assert_eq!(removed, 1);
        assert_eq!(pipeline.stats().total_entries, 1);
    }

    #[test]
    fn test_generate_report() {
        let mut pipeline = QualityPipeline::new();
        pipeline.add_entry(make_entry("e1", vec![]));
        pipeline.record_adoption("e1");

        let report = pipeline.generate_report();
        assert!(report.contains("AgentGuard 数据质量报告"));
        assert!(report.contains("总知识条目: 1"));
        assert!(report.contains("总采纳: 1"));
    }

    #[test]
    fn test_quality_stats_categorization() {
        let mut pipeline = QualityPipeline::new();

        // High quality
        let mut e1 = make_entry("high", vec![]);
        e1.source_trust_score = 0.95;
        e1.cross_validation_score = 0.95;
        e1.user_feedback_score = 0.95;
        e1.execution_success_rate = 0.95;
        pipeline.add_entry(e1);

        // Low quality
        let mut e2 = make_entry("low", vec![]);
        e2.source_trust_score = 0.1;
        e2.cross_validation_score = 0.1;
        e2.user_feedback_score = 0.1;
        e2.execution_success_rate = 0.1;
        pipeline.add_entry(e2);

        let stats = pipeline.stats();
        assert_eq!(stats.high_quality_count, 1);
        assert_eq!(stats.low_quality_count, 1);
    }

    #[test]
    fn test_adoption_rejection_loop() {
        let mut pipeline = QualityPipeline::new();
        let entry = make_entry("e1", vec![]);
        pipeline.add_entry(entry);
        let initial = pipeline.entries.get("e1").unwrap().quality_score;

        // Multiple adoptions
        for _ in 0..5 {
            pipeline.record_adoption("e1");
        }
        let after_adoption = pipeline.entries.get("e1").unwrap().quality_score;
        assert!(after_adoption > initial);

        // Rejection brings it down
        pipeline.record_rejection("e1");
        let after_rejection = pipeline.entries.get("e1").unwrap().quality_score;
        assert!(after_rejection < after_adoption);
    }

    #[test]
    fn test_negative_sample_excluded_from_search() {
        let mut pipeline = QualityPipeline::new();
        pipeline.add_entry(make_entry("e1", vec!["rust"]));
        pipeline.mark_negative("e1", "bad data");

        let results = pipeline.search_by_tag("rust", 0.0);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_cross_validation_improves_score() {
        let mut pipeline = QualityPipeline::new();
        pipeline.add_entry(make_entry("e1", vec![]));
        let before = pipeline.entries.get("e1").unwrap().cross_validation_score;

        // Successful validation
        let outputs = vec![
            AgentOutput {
                agent_id: "a1".to_string(),
                output: "correct answer".to_string(),
                confidence: 0.9,
            },
            AgentOutput {
                agent_id: "a2".to_string(),
                output: "correct answer".to_string(),
                confidence: 0.85,
            },
        ];
        pipeline.cross_validate("e1", outputs);

        let after = pipeline.entries.get("e1").unwrap().cross_validation_score;
        assert!(after > before);
    }

    #[test]
    fn test_full_lifecycle() {
        let mut pipeline = QualityPipeline::new();

        // 1. Add entry
        let entry = make_entry("e1", vec!["rust", "scheduler"]);
        pipeline.add_entry(entry);

        // 2. Cross-validate with consistent outputs
        let outputs = vec![
            AgentOutput {
                agent_id: "reviewer-1".to_string(),
                output: "code is correct".to_string(),
                confidence: 0.9,
            },
            AgentOutput {
                agent_id: "reviewer-2".to_string(),
                output: "code is correct".to_string(),
                confidence: 0.85,
            },
        ];
        let val_result = pipeline.cross_validate("e1", outputs);
        assert!(matches!(
            val_result.conclusion,
            ValidationConclusion::HighConfidence { .. }
        ));

        // 3. User adopts
        pipeline.record_adoption("e1");
        pipeline.record_adoption("e1");

        // 4. Execution succeeds
        pipeline.record_execution("e1", true);

        // 5. Quality should be high
        let entry = pipeline.entries.get("e1").unwrap();
        assert!(entry.quality_score > 0.7);
        assert!(!entry.is_negative);

        // 6. Report
        let report = pipeline.generate_report();
        assert!(report.contains("高质量"));
    }
}
