//! # Self-boundary Reasoning (Metacognitive Agent)
//!
//! 系统维护一份自我模型，知道自己"擅长什么、不擅长什么"，
//! 据此选择亲自做（ReasonDirectly）、调工具（UseTool），
//! 还是交给人类（Escalate）。
//!
//! ## 设计来源
//! all-agentic-architectures #18 Reflexive Metacognitive Agent
//!
//! ## 在 KIAS 中的位置
//! Self-boundary 在 tier_routing **之前**执行：
//! ```text
//! 请求 → Self-boundary → (ReasonDirectly) → tier_routing → 模型选择
//!                       → (UseTool) → 直接调工具
//!                       → (Escalate) → 交给人类
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
// HashMap not needed — using direct struct access

// ─── Response Strategy ─────────────────────────────────────────────────

/// 元认知分析后选择的策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResponseStrategy {
    /// 置信度高 + 低风险 → 直接回答
    ReasonDirectly,
    /// 有匹配工具 → 调用指定工具
    UseTool { tool_name: String },
    /// 高风险或低置信度 → 交给人类
    Escalate { reason: String },
    /// 置信度中等 + 有部分知识 → 回答但标注不确定性
    ReasonWithCaveat { caveat: String },
    /// 元认知评估 → 启用反思策略（自我审视后决定）
    MetacognitiveReview,
}

// ─── Metacognitive Analysis ────────────────────────────────────────────

/// 元认知分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetacognitiveAnalysis {
    /// 置信度 0.0~1.0
    pub confidence: f64,
    /// 选择的策略
    pub strategy: ResponseStrategy,
    /// 推理过程
    pub reasoning: String,
    /// 分析时间
    pub analyzed_at: DateTime<Utc>,
}

// ─── Self Model ────────────────────────────────────────────────────────

/// Agent 自我模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    /// 擅长的知识领域
    pub knowledge_domains: Vec<DomainExpertise>,
    /// 可用工具列表
    pub tools_available: Vec<ToolCapability>,
    /// 置信度阈值（低于此值自动 escalate）
    pub confidence_threshold: f64,
    /// 高风险主题（必须 escalate）
    pub high_risk_topics: Vec<String>,
    /// 历史表现统计
    pub performance_stats: PerformanceStats,
}

/// 领域专业度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainExpertise {
    /// 领域名称
    pub domain: String,
    /// 专业度 0.0~1.0
    pub proficiency: f64,
    /// 该领域历史任务数
    pub task_count: u64,
    /// 该领域成功率
    pub success_rate: f64,
}

/// 工具能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: String,
    /// 适用领域
    pub applicable_domains: Vec<String>,
    /// 历史成功率
    pub reliability: f64,
}

/// 历史表现统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// 总任务数
    pub total_tasks: u64,
    /// 直接回答成功率
    pub direct_success_rate: f64,
    /// 工具调用成功率
    pub tool_success_rate: f64,
    /// escalate 率
    pub escalation_rate: f64,
    /// 自信回答但被纠正的次数
    pub false_confidence_count: u64,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
}

impl Default for PerformanceStats {
    fn default() -> Self {
        Self {
            total_tasks: 0,
            direct_success_rate: 0.8,
            tool_success_rate: 0.9,
            escalation_rate: 0.1,
            false_confidence_count: 0,
            last_updated: Utc::now(),
        }
    }
}

// ─── Self-boundary Reasoner ────────────────────────────────────────────

/// 自我边界推理器
pub struct SelfBoundaryReasoner {
    /// 自我模型
    self_model: SelfModel,
    /// 分析历史
    history: Vec<MetacognitiveAnalysis>,
}

impl SelfBoundaryReasoner {
    /// 创建新的推理器
    pub fn new(self_model: SelfModel) -> Self {
        Self {
            self_model,
            history: Vec::new(),
        }
    }

    /// 评估一个任务，决定策略
    ///
    /// # 流程
    /// 1. 检查高风险主题（关键词匹配 → 强制 escalate）
    /// 2. 匹配知识领域（计算置信度）
    /// 3. 匹配可用工具（如果有匹配工具 → UseTool）
    /// 4. 根据置信度和阈值决定策略
    pub fn evaluate(&mut self, task_description: &str) -> MetacognitiveAnalysis {
        let task_lower = task_description.to_lowercase();

        // Step 1: 高风险主题检查
        for topic in &self.self_model.high_risk_topics {
            if task_lower.contains(&topic.to_lowercase()) {
                let analysis = MetacognitiveAnalysis {
                    confidence: 0.0,
                    strategy: ResponseStrategy::Escalate {
                        reason: format!("High-risk topic detected: '{}'", topic),
                    },
                    reasoning: format!(
                        "Task matches high-risk topic '{}'. \
                         Per safety policy, this must be escalated to a human expert.",
                        topic
                    ),
                    analyzed_at: Utc::now(),
                };
                self.history.push(analysis.clone());
                return analysis;
            }
        }

        // Step 2: 知识领域匹配
        let domain_match = self.match_domains(&task_lower);
        let confidence = domain_match.as_ref().map(|d| d.proficiency).unwrap_or(0.3);

        // Step 3: 工具匹配
        let tool_match = self.match_tools(&task_lower);

        // Step 4: 策略决定
        // 高置信度 → 直接推理（即使有工具也优先自己做）
        // 中置信度 + 有工具 → 调工具
        // 中置信度 + 无工具 → 标注不确定性
        // 低置信度 → escalate
        let strategy = if confidence >= self.self_model.confidence_threshold {
            ResponseStrategy::ReasonDirectly
        } else if let Some(tool) = tool_match {
            ResponseStrategy::UseTool {
                tool_name: tool.name.clone(),
            }
        } else if confidence >= self.self_model.confidence_threshold * 0.7 {
            ResponseStrategy::ReasonWithCaveat {
                caveat: format!(
                    "Moderate confidence ({:.0}%). Domain: {}. Cross-check recommended.",
                    confidence * 100.0,
                    domain_match
                        .as_ref()
                        .map(|d| d.domain.as_str())
                        .unwrap_or("general")
                ),
            }
        } else {
            ResponseStrategy::Escalate {
                reason: format!(
                    "Low confidence ({:.0}%) and no matching tools. \
                     Task may be outside agent capabilities.",
                    confidence * 100.0
                ),
            }
        };

        let analysis = MetacognitiveAnalysis {
            confidence,
            strategy,
            reasoning: format!(
                "Domain match: {}. Tool match: {}. Threshold: {:.0}%. Confidence: {:.0}%.",
                domain_match
                    .as_ref()
                    .map(|d| d.domain.as_str())
                    .unwrap_or("none"),
                tool_match
                    .as_ref()
                    .map(|t| t.name.as_str())
                    .unwrap_or("none"),
                self.self_model.confidence_threshold * 100.0,
                confidence * 100.0
            ),
            analyzed_at: Utc::now(),
        };

        self.history.push(analysis.clone());
        analysis
    }

    /// 匹配知识领域（关键词级匹配）
    fn match_domains(&self, task_lower: &str) -> Option<&DomainExpertise> {
        self.self_model
            .knowledge_domains
            .iter()
            .filter(|d| {
                // 按空格拆分领域名，任意一个词出现在任务中即匹配
                let domain_lower = d.domain.to_lowercase();
                domain_lower
                    .split_whitespace()
                    .any(|word| task_lower.contains(word))
            })
            .max_by(|a, b| {
                a.proficiency
                    .partial_cmp(&b.proficiency)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// 匹配工具（完整词匹配，避免子串误匹配）
    fn match_tools(&self, task_lower: &str) -> Option<&ToolCapability> {
        let words: Vec<&str> = task_lower.split_whitespace().collect();
        self.self_model
            .tools_available
            .iter()
            .filter(|t| {
                t.applicable_domains
                    .iter()
                    .any(|d| words.iter().any(|w| *w == d.to_lowercase().as_str()))
            })
            .max_by(|a, b| {
                a.reliability
                    .partial_cmp(&b.reliability)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// 更新自我模型（任务完成后反馈）
    pub fn record_outcome(&mut self, strategy: &ResponseStrategy, success: bool) {
        self.self_model.performance_stats.total_tasks += 1;

        match strategy {
            ResponseStrategy::ReasonDirectly => {
                let stats = &mut self.self_model.performance_stats;
                let n = stats.total_tasks as f64;
                stats.direct_success_rate =
                    (stats.direct_success_rate * (n - 1.0) + if success { 1.0 } else { 0.0 }) / n;
                if !success {
                    stats.false_confidence_count += 1;
                }
            }
            ResponseStrategy::UseTool { .. } => {
                let stats = &mut self.self_model.performance_stats;
                let n = stats.total_tasks as f64;
                stats.tool_success_rate =
                    (stats.tool_success_rate * (n - 1.0) + if success { 1.0 } else { 0.0 }) / n;
            }
            ResponseStrategy::Escalate { .. } => {
                let stats = &mut self.self_model.performance_stats;
                let n = stats.total_tasks as f64;
                stats.escalation_rate = (stats.escalation_rate * (n - 1.0) + 1.0) / n;
            }
            ResponseStrategy::ReasonWithCaveat { .. } => {
                // 与 ReasonDirectly 相同的处理
                let stats = &mut self.self_model.performance_stats;
                let n = stats.total_tasks as f64;
                stats.direct_success_rate =
                    (stats.direct_success_rate * (n - 1.0) + if success { 1.0 } else { 0.0 }) / n;
            }
            ResponseStrategy::MetacognitiveReview => {
                // 元认知反思策略 — 跟踪反思成功率
                let stats = &mut self.self_model.performance_stats;
                let n = stats.total_tasks as f64;
                stats.direct_success_rate =
                    (stats.direct_success_rate * (n - 1.0) + if success { 1.0 } else { 0.0 }) / n;
            }
        }

        self.self_model.performance_stats.last_updated = Utc::now();
    }

    /// 获取自我模型引用
    pub fn self_model(&self) -> &SelfModel {
        &self.self_model
    }

    /// 获取分析历史
    pub fn history(&self) -> &[MetacognitiveAnalysis] {
        &self.history
    }

    /// 获取统计摘要
    pub fn stats(&self) -> BoundaryStats {
        let total = self.history.len();
        let direct = self
            .history
            .iter()
            .filter(|a| matches!(a.strategy, ResponseStrategy::ReasonDirectly))
            .count();
        let tool = self
            .history
            .iter()
            .filter(|a| matches!(a.strategy, ResponseStrategy::UseTool { .. }))
            .count();
        let escalated = self
            .history
            .iter()
            .filter(|a| matches!(a.strategy, ResponseStrategy::Escalate { .. }))
            .count();
        let caveat = self
            .history
            .iter()
            .filter(|a| matches!(a.strategy, ResponseStrategy::ReasonWithCaveat { .. }))
            .count();

        BoundaryStats {
            total_evaluations: total,
            direct_count: direct,
            tool_count: tool,
            escalated_count: escalated,
            caveat_count: caveat,
            avg_confidence: if total > 0 {
                self.history.iter().map(|a| a.confidence).sum::<f64>() / total as f64
            } else {
                0.0
            },
        }
    }
}

/// 边界推理统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryStats {
    pub total_evaluations: usize,
    pub direct_count: usize,
    pub tool_count: usize,
    pub escalated_count: usize,
    pub caveat_count: usize,
    pub avg_confidence: f64,
}

// ─── Default Self Model Builder ────────────────────────────────────────

impl SelfModel {
    /// 从 KIAS 现有模块构建默认自我模型
    pub fn kias_default() -> Self {
        Self {
            knowledge_domains: vec![
                DomainExpertise {
                    domain: "Rust programming".into(),
                    proficiency: 0.9,
                    task_count: 500,
                    success_rate: 0.95,
                },
                DomainExpertise {
                    domain: "Agent architecture".into(),
                    proficiency: 0.85,
                    task_count: 200,
                    success_rate: 0.9,
                },
                DomainExpertise {
                    domain: "GxP compliance".into(),
                    proficiency: 0.7,
                    task_count: 50,
                    success_rate: 0.8,
                },
                DomainExpertise {
                    domain: "System design".into(),
                    proficiency: 0.85,
                    task_count: 300,
                    success_rate: 0.9,
                },
            ],
            tools_available: vec![
                ToolCapability {
                    name: "file_operations".into(),
                    description: "Read/write/edit files".into(),
                    applicable_domains: vec!["file".into(), "code".into(), "config".into()],
                    reliability: 0.99,
                },
                ToolCapability {
                    name: "shell_exec".into(),
                    description: "Execute shell commands".into(),
                    applicable_domains: vec!["build".into(), "test".into(), "deploy".into()],
                    reliability: 0.95,
                },
                ToolCapability {
                    name: "web_search".into(),
                    description: "Search the web".into(),
                    applicable_domains: vec![
                        "research".into(),
                        "reference".into(),
                        "search".into(),
                        "benchmark".into(),
                    ],
                    reliability: 0.85,
                },
            ],
            confidence_threshold: 0.7,
            high_risk_topics: vec![
                "medical diagnosis".into(),
                "legal advice".into(),
                "financial trading".into(),
                "production data deletion".into(),
                "security vulnerability".into(),
            ],
            performance_stats: PerformanceStats::default(),
        }
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_reasoner() -> SelfBoundaryReasoner {
        SelfBoundaryReasoner::new(SelfModel::kias_default())
    }

    #[test]
    fn test_high_risk_topic_escalates() {
        let mut reasoner = test_reasoner();
        let analysis = reasoner.evaluate("Please provide medical diagnosis for chest pain");
        assert!(matches!(
            analysis.strategy,
            ResponseStrategy::Escalate { .. }
        ));
        assert_eq!(analysis.confidence, 0.0);
    }

    #[test]
    fn test_known_domain_direct() {
        let mut reasoner = test_reasoner();
        let analysis = reasoner.evaluate("Write a Rust function to parse JSON");
        assert!(matches!(
            analysis.strategy,
            ResponseStrategy::ReasonDirectly
        ));
        assert!(analysis.confidence >= 0.7);
    }

    #[test]
    fn test_tool_match() {
        let mut reasoner = test_reasoner();
        // "Search for latest Rust async runtime benchmarks"
        // Domain: "Rust programming" matches via "rust" (proficiency 0.9 >= 0.7)
        // → ReasonDirectly (高置信度优先)
        let analysis = reasoner.evaluate("Search for latest Rust async runtime benchmarks");
        assert!(matches!(
            analysis.strategy,
            ResponseStrategy::ReasonDirectly
        ));
    }

    #[test]
    fn test_unknown_domain_escalates() {
        let mut reasoner = test_reasoner();
        let analysis = reasoner.evaluate("Explain quantum entanglement in detail");
        // Low confidence → escalate or caveat
        assert!(analysis.confidence < 0.7);
    }

    #[test]
    fn test_performance_stats_update() {
        let mut reasoner = test_reasoner();
        reasoner.evaluate("Write Rust code");
        reasoner.record_outcome(&ResponseStrategy::ReasonDirectly, true);

        let stats = &reasoner.self_model().performance_stats;
        assert_eq!(stats.total_tasks, 1);
        assert_eq!(stats.false_confidence_count, 0);
    }

    #[test]
    fn test_false_confidence_tracked() {
        let mut reasoner = test_reasoner();
        reasoner.record_outcome(&ResponseStrategy::ReasonDirectly, false);

        let stats = &reasoner.self_model().performance_stats;
        assert_eq!(stats.false_confidence_count, 1);
    }

    #[test]
    fn test_boundary_stats() {
        let mut reasoner = test_reasoner();
        reasoner.evaluate("Write Rust code");
        reasoner.evaluate("Provide medical diagnosis");
        reasoner.evaluate("Unknown topic xyz");

        let stats = reasoner.stats();
        assert_eq!(stats.total_evaluations, 3);
        assert!(stats.direct_count >= 1);
        assert!(stats.escalated_count >= 1); // at least medical
    }

    #[test]
    fn test_history_recorded() {
        let mut reasoner = test_reasoner();
        reasoner.evaluate("Test task 1");
        reasoner.evaluate("Test task 2");
        assert_eq!(reasoner.history().len(), 2);
    }

    #[test]
    fn test_self_model_kias_default_domain_count() {
        let model = SelfModel::kias_default();
        assert_eq!(model.knowledge_domains.len(), 4);
        assert!(model
            .knowledge_domains
            .iter()
            .any(|d| d.domain == "Rust programming"));
        assert!(model
            .knowledge_domains
            .iter()
            .any(|d| d.domain == "Agent architecture"));
        assert!(model
            .knowledge_domains
            .iter()
            .any(|d| d.domain == "GxP compliance"));
        assert!(model
            .knowledge_domains
            .iter()
            .any(|d| d.domain == "System design"));
    }

    #[test]
    fn test_self_model_kias_default_tool_count() {
        let model = SelfModel::kias_default();
        assert_eq!(model.tools_available.len(), 3);
        assert!(model
            .tools_available
            .iter()
            .any(|t| t.name == "file_operations"));
        assert!(model.tools_available.iter().any(|t| t.name == "shell_exec"));
        assert!(model.tools_available.iter().any(|t| t.name == "web_search"));
    }

    #[test]
    fn test_self_model_kias_default_high_risk_topics() {
        let model = SelfModel::kias_default();
        assert_eq!(model.high_risk_topics.len(), 5);
        assert!(model
            .high_risk_topics
            .contains(&"medical diagnosis".to_string()));
        assert!(model.high_risk_topics.contains(&"legal advice".to_string()));
    }

    #[test]
    fn test_performance_stats_default_values() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.direct_success_rate, 0.8);
        assert_eq!(stats.tool_success_rate, 0.9);
        assert_eq!(stats.escalation_rate, 0.1);
        assert_eq!(stats.false_confidence_count, 0);
    }

    #[test]
    fn test_record_outcome_use_tool_updates_tool_success_rate() {
        let mut reasoner = test_reasoner();
        let initial_rate = reasoner.self_model().performance_stats.tool_success_rate;
        reasoner.record_outcome(
            &ResponseStrategy::UseTool {
                tool_name: "test_tool".to_string(),
            },
            true,
        );
        let stats = &reasoner.self_model().performance_stats;
        assert_eq!(stats.total_tasks, 1);
        // After a success, tool_success_rate should increase or stay high
        assert!(stats.tool_success_rate >= initial_rate);
    }

    #[test]
    fn test_record_outcome_escalate_updates_escalation_rate() {
        let mut reasoner = test_reasoner();
        let initial_rate = reasoner.self_model().performance_stats.escalation_rate;
        reasoner.record_outcome(
            &ResponseStrategy::Escalate {
                reason: "test".to_string(),
            },
            false,
        );
        let stats = &reasoner.self_model().performance_stats;
        assert_eq!(stats.total_tasks, 1);
        // Escalation should increase escalation_rate
        assert!(stats.escalation_rate >= initial_rate);
    }

    #[test]
    fn test_high_risk_topic_case_insensitive() {
        let mut reasoner = test_reasoner();
        // "medical diagnosis" is a high-risk topic
        let analysis = reasoner.evaluate("I need MEDICAL DIAGNOSIS help");
        assert!(matches!(
            analysis.strategy,
            ResponseStrategy::Escalate { .. }
        ));
    }

    #[test]
    fn test_stats_empty_history() {
        let reasoner = test_reasoner();
        let stats = reasoner.stats();
        assert_eq!(stats.total_evaluations, 0);
        assert_eq!(stats.direct_count, 0);
        assert_eq!(stats.tool_count, 0);
        assert_eq!(stats.escalated_count, 0);
        assert_eq!(stats.caveat_count, 0);
        assert_eq!(stats.avg_confidence, 0.0);
    }

    #[test]
    fn test_evaluate_builds_domain_confidence() {
        let mut reasoner = test_reasoner();
        // "GxP compliance" domain (proficiency=0.7) should be matched
        let analysis = reasoner.evaluate("Check GxP compliance requirements");
        assert!(analysis.confidence >= 0.7);
        assert!(matches!(
            analysis.strategy,
            ResponseStrategy::ReasonDirectly
        ));
    }
}
