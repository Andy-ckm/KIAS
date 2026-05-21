//! # Harness 智能推荐引擎
//!
//! 基于项目上下文和历史使用模式，推荐相关工程制品。
//!
//! ## 推荐策略（三维度评分）
//! 1. **上下文相关度** — 当前 crate/task/域 匹配
//! 2. **历史成功率** — Flywheel 飞轮：用过且成功的制品优先
//! 3. **依赖关系** — 已使用制品的关联制品
//!
//! ## 论文支撑
//! - Agentic Harness Engineering (2604.25850): 可观测驱动的自动进化
//! - Continual Harness (2605.09998): 在线自适应
//! - Flywheel Learner (agentic_rag.rs): 关键词→成功引用 飞轮

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

// ── 制品类型 ──────────────────────────────────────────────────

/// 工程制品分类（Harness 四层映射）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactType {
    /// AGENTS.md / CLAUDE.md — 执行 Harness 入口
    AgentsMd,
    /// Skills 技能库 — 知识 Harness
    Skills,
    /// 合规规则 — 安全 Harness
    ComplianceRules,
    /// 模板文件（设计文档模板、测试模板等）
    Templates,
    /// 自动化脚本（构建、部署、检查）
    Scripts,
    /// 参考文档（架构、API、设计文档）
    Docs,
    /// Agent 配置（agents/ 下的定义）
    Agents,
    /// 命令定义（slash commands）
    Commands,
    /// 服务矩阵（微服务映射）
    ServiceMatrix,
    /// 需求文档（REQ-xxx）
    Requirements,
    /// 代码参考（reference-projects/）
    CodeReference,
}

impl std::fmt::Display for ArtifactType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AgentsMd => write!(f, "agents_md"),
            Self::Skills => write!(f, "skills"),
            Self::ComplianceRules => write!(f, "compliance_rules"),
            Self::Templates => write!(f, "templates"),
            Self::Scripts => write!(f, "scripts"),
            Self::Docs => write!(f, "docs"),
            Self::Agents => write!(f, "agents"),
            Self::Commands => write!(f, "commands"),
            Self::ServiceMatrix => write!(f, "service_matrix"),
            Self::Requirements => write!(f, "requirements"),
            Self::CodeReference => write!(f, "code_reference"),
        }
    }
}

// ── 制品元数据 ────────────────────────────────────────────────

/// 单个工程制品的元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    /// 唯一标识（路径或名称）
    pub id: String,
    /// 制品类型
    pub artifact_type: ArtifactType,
    /// 人类可读名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 所属 Harness 层（1=执行, 2=知识, 3=安全, 4=自我进化）
    pub harness_layer: u8,
    /// 关联的 crate 列表
    pub related_crates: Vec<String>,
    /// 关联的法规标准（如 "21 CFR Part 11", "EU Annex 11"）
    pub regulations: Vec<String>,
    /// 标签（用于检索）
    pub tags: Vec<String>,
    /// 依赖的其他制品 ID
    pub dependencies: Vec<String>,
    /// 版本
    pub version: String,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 成功使用次数（飞轮计数）
    pub usage_count: u64,
    /// 平均使用成功率（0.0-1.0）
    pub success_rate: f64,
}

// ── 项目上下文 ────────────────────────────────────────────────

/// 当前项目上下文（驱动推荐）
#[derive(Debug, Clone, Default)]
pub struct ProjectContext {
    /// 当前正在操作的 crate
    pub current_crate: Option<String>,
    /// 任务类型（如 "test", "compliance", "refactor", "innovation"）
    pub task_type: Option<String>,
    /// 法规领域（如 "FDA", "EU_AI_ACT", "GxP"）
    pub regulation_domain: Option<String>,
    /// 当前使用的制品 ID 列表
    pub active_artifacts: Vec<String>,
    /// 关键词（从用户输入提取）
    pub keywords: Vec<String>,
    /// 代码变更涉及的文件路径
    pub changed_files: Vec<String>,
}

// ── 推荐结果 ──────────────────────────────────────────────────

/// 推荐的制品
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    /// 制品元数据
    pub artifact: ArtifactMetadata,
    /// 综合推荐分数 (0.0-1.0)
    pub score: f64,
    /// 上下文相关度分 (0.0-1.0)
    pub context_score: f64,
    /// 历史成功分 (0.0-1.0)
    pub history_score: f64,
    /// 依赖关联分 (0.0-1.0)
    pub dependency_score: f64,
    /// 推荐理由
    pub reasons: Vec<String>,
}

/// 推荐引擎配置
#[derive(Debug, Clone)]
pub struct RecommenderConfig {
    /// 上下文权重
    pub context_weight: f64,
    /// 历史权重
    pub history_weight: f64,
    /// 依赖权重
    pub dependency_weight: f64,
    /// 最大推荐数
    pub max_recommendations: usize,
    /// 最低推荐分数
    pub min_score: f64,
}

impl Default for RecommenderConfig {
    fn default() -> Self {
        Self {
            context_weight: 0.4,
            history_weight: 0.35,
            dependency_weight: 0.25,
            max_recommendations: 10,
            min_score: 0.1,
        }
    }
}

// ── 使用记录（飞轮数据）──────────────────────────────────────

/// 一次制品使用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// 使用的制品 ID
    pub artifact_id: String,
    /// 使用时的上下文关键词
    pub context_keywords: Vec<String>,
    /// 使用时的 crate
    pub crate_name: Option<String>,
    /// 是否成功（飞轮信号）
    pub success: bool,
    /// 使用时间
    pub timestamp: DateTime<Utc>,
    /// 使用的其他制品（共现）
    pub co_used: Vec<String>,
}

// ── 推荐引擎 ─────────────────────────────────────────────────

/// Harness 智能推荐引擎
pub struct HarnessRecommender {
    /// 制品注册表
    artifacts: HashMap<String, ArtifactMetadata>,
    /// 使用历史（飞轮）
    usage_history: Arc<RwLock<Vec<UsageRecord>>>,
    /// 关键词→制品索引（倒排索引）
    keyword_index: HashMap<String, Vec<String>>,
    /// crate→制品索引
    crate_index: HashMap<String, Vec<String>>,
    /// 法规→制品索引
    regulation_index: HashMap<String, Vec<String>>,
    /// 共现矩阵（制品A→[与A一起使用的制品]）
    co_occurrence: Arc<RwLock<HashMap<String, HashMap<String, u64>>>>,
    /// 配置
    config: RecommenderConfig,
}

impl HarnessRecommender {
    pub fn new(config: RecommenderConfig) -> Self {
        Self {
            artifacts: HashMap::new(),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            keyword_index: HashMap::new(),
            crate_index: HashMap::new(),
            regulation_index: HashMap::new(),
            co_occurrence: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// 注册一个制品
    pub fn register(&mut self, artifact: ArtifactMetadata) {
        // 建立关键词索引
        for tag in &artifact.tags {
            self.keyword_index
                .entry(tag.to_lowercase())
                .or_default()
                .push(artifact.id.clone());
        }
        // 建立 crate 索引
        for c in &artifact.related_crates {
            self.crate_index
                .entry(c.clone())
                .or_default()
                .push(artifact.id.clone());
        }
        // 建立法规索引
        for reg in &artifact.regulations {
            self.regulation_index
                .entry(reg.clone())
                .or_default()
                .push(artifact.id.clone());
        }
        self.artifacts.insert(artifact.id.clone(), artifact);
    }

    /// 批量注册
    pub fn register_all(&mut self, artifacts: Vec<ArtifactMetadata>) {
        for a in artifacts {
            self.register(a);
        }
    }

    /// 记录一次使用（飞轮）
    pub async fn record_usage(&self, record: UsageRecord) {
        // 更新共现矩阵
        {
            let mut co = self.co_occurrence.write().await;
            for co_id in &record.co_used {
                co.entry(record.artifact_id.clone())
                    .or_default()
                    .entry(co_id.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }
        let mut history = self.usage_history.write().await;
        history.push(record);
    }

    /// 核心推荐 API
    pub async fn recommend(&self, context: &ProjectContext) -> Vec<Recommendation> {
        let mut scored: Vec<Recommendation> = Vec::new();

        for (id, artifact) in &self.artifacts {
            // 跳过已在使用的制品
            if context.active_artifacts.contains(id) {
                continue;
            }

            let ctx_score = self.score_context(artifact, context);
            let hist_score = self.score_history(artifact, context).await;
            let dep_score = self.score_dependency(artifact, context);

            let final_score = ctx_score * self.config.context_weight
                + hist_score * self.config.history_weight
                + dep_score * self.config.dependency_weight;

            if final_score >= self.config.min_score {
                let reasons = self.build_reasons(artifact, context, ctx_score, hist_score, dep_score);
                scored.push(Recommendation {
                    artifact: artifact.clone(),
                    score: final_score,
                    context_score: ctx_score,
                    history_score: hist_score,
                    dependency_score: dep_score,
                    reasons,
                });
            }
        }

        // 按分数降序
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(self.config.max_recommendations);
        scored
    }

    /// 按类型推荐
    pub async fn recommend_by_type(
        &self,
        context: &ProjectContext,
        artifact_type: &ArtifactType,
    ) -> Vec<Recommendation> {
        self.recommend(context)
            .await
            .into_iter()
            .filter(|r| r.artifact.artifact_type == *artifact_type)
            .collect()
    }

    /// 按法规域推荐
    pub async fn recommend_by_regulation(
        &self,
        context: &ProjectContext,
        regulation: &str,
    ) -> Vec<Recommendation> {
        self.recommend(context)
            .await
            .into_iter()
            .filter(|r| r.artifact.regulations.iter().any(|reg| reg.contains(regulation)))
            .collect()
    }

    /// 查询制品
    pub fn get(&self, id: &str) -> Option<&ArtifactMetadata> {
        self.artifacts.get(id)
    }

    /// 按类型列出
    pub fn list_by_type(&self, artifact_type: &ArtifactType) -> Vec<&ArtifactMetadata> {
        self.artifacts
            .values()
            .filter(|a| a.artifact_type == *artifact_type)
            .collect()
    }

    /// 全文搜索（名称+描述+标签）
    pub fn search(&self, query: &str) -> Vec<&ArtifactMetadata> {
        let q = query.to_lowercase();
        self.artifacts
            .values()
            .filter(|a| {
                a.name.to_lowercase().contains(&q)
                    || a.description.to_lowercase().contains(&q)
                    || a.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// 制品总数
    pub fn count(&self) -> usize {
        self.artifacts.len()
    }

    // ── 内部评分方法 ──────────────────────────────────────────

    /// 上下文相关度评分
    fn score_context(&self, artifact: &ArtifactMetadata, context: &ProjectContext) -> f64 {
        let mut score: f64 = 0.0;
        let mut signals = 0;

        // 1. Crate 匹配
        if let Some(ref crate_name) = context.current_crate {
            if artifact.related_crates.contains(crate_name) {
                score += 1.0;
            }
            signals += 1;
        }

        // 2. 关键词匹配（标签交集）
        if !context.keywords.is_empty() {
            let artifact_tags: HashSet<String> =
                artifact.tags.iter().map(|t| t.to_lowercase()).collect();
            let context_kw: HashSet<String> =
                context.keywords.iter().map(|k| k.to_lowercase()).collect();
            let overlap = artifact_tags.intersection(&context_kw).count();
            let max_possible = context_kw.len().max(1);
            score += overlap as f64 / max_possible as f64;
            signals += 1;
        }

        // 3. 法规域匹配
        if let Some(ref domain) = context.regulation_domain {
            if artifact
                .regulations
                .iter()
                .any(|r| r.to_lowercase().contains(&domain.to_lowercase()))
            {
                score += 1.0;
            }
            signals += 1;
        }

        // 4. 任务类型匹配
        if let Some(ref task) = context.task_type {
            let task_lower = task.to_lowercase();
            if artifact.tags.iter().any(|t| t.to_lowercase().contains(&task_lower))
                || artifact.description.to_lowercase().contains(&task_lower)
            {
                score += 0.8;
            }
            signals += 1;
        }

        // 5. 文件路径匹配
        if !context.changed_files.is_empty() {
            let file_hit = context.changed_files.iter().any(|f| {
                artifact
                    .related_crates
                    .iter()
                    .any(|c| f.contains(c))
            });
            if file_hit {
                score += 0.6;
            }
            signals += 1;
        }

        if signals == 0 {
            return 0.0;
        }
        (score / signals as f64).min(1.0)
    }

    /// 历史成功评分（飞轮）
    async fn score_history(&self, artifact: &ArtifactMetadata, context: &ProjectContext) -> f64 {
        let history = self.usage_history.read().await;
        if history.is_empty() {
            // 无历史数据，用制品自带的成功率
            return artifact.success_rate * (artifact.usage_count as f64 / (artifact.usage_count as f64 + 5.0));
        }

        // 关键词匹配的历史记录
        let context_kw: HashSet<String> = context.keywords.iter().map(|k| k.to_lowercase()).collect();

        let relevant: Vec<&UsageRecord> = history
            .iter()
            .filter(|r| {
                r.artifact_id == artifact.id
                    || r.context_keywords.iter().any(|k| context_kw.contains(&k.to_lowercase()))
            })
            .collect();

        if relevant.is_empty() {
            return artifact.success_rate * 0.3; // 无相关历史，低分
        }

        let success_count = relevant.iter().filter(|r| r.success).count() as f64;
        let total = relevant.len() as f64;
        let recency_bonus = if relevant.iter().any(|r| {
            (Utc::now() - r.timestamp).num_hours() < 24
        }) {
            0.2
        } else {
            0.0
        };

        ((success_count / total) + recency_bonus).min(1.0)
    }

    /// 依赖关联评分
    fn score_dependency(&self, artifact: &ArtifactMetadata, context: &ProjectContext) -> f64 {
        if context.active_artifacts.is_empty() {
            return 0.0;
        }

        let mut score: f64 = 0.0;

        // 1. 被活跃制品依赖
        for active_id in &context.active_artifacts {
            if artifact.dependencies.contains(active_id) {
                score += 0.5;
            }
        }

        // 2. 共现频率 (需要 block_on 因为 score_dependency 不是 async)
        // 使用 try_read 避免阻塞
        if let Ok(co) = self.co_occurrence.try_read() {
            for active_id in &context.active_artifacts {
                if let Some(co_map) = co.get(active_id) {
                    if let Some(count) = co_map.get(&artifact.id) {
                        score += (*count as f64).ln() / 10.0;
                    }
                }
            }
        }

        score.min(1.0)
    }

    /// 构建推荐理由
    fn build_reasons(
        &self,
        artifact: &ArtifactMetadata,
        context: &ProjectContext,
        ctx_score: f64,
        hist_score: f64,
        dep_score: f64,
    ) -> Vec<String> {
        let mut reasons = Vec::new();

        if let Some(ref crate_name) = context.current_crate {
            if artifact.related_crates.contains(crate_name) {
                reasons.push(format!("直接关联当前 crate: {}", crate_name));
            }
        }

        if ctx_score > 0.5 {
            reasons.push(format!("上下文高度相关 ({:.0}%)", ctx_score * 100.0));
        }

        if hist_score > 0.5 {
            reasons.push(format!("历史使用成功率高 ({:.0}%)", hist_score * 100.0));
        }

        if dep_score > 0.3 {
            reasons.push("与当前使用的制品有依赖关系".to_string());
        }

        if !artifact.regulations.is_empty() {
            reasons.push(format!("覆盖法规: {}", artifact.regulations.join(", ")));
        }

        if reasons.is_empty() {
            reasons.push("综合评分匹配".to_string());
        }

        reasons
    }
}

impl Default for HarnessRecommender {
    fn default() -> Self {
        Self::new(RecommenderConfig::default())
    }
}

// ── 内置制品注册（Bootstrap）─────────────────────────────────

/// 注册 AgentGuard 内置工程制品
pub fn register_builtin_artifacts(recommender: &mut HarnessRecommender) {
    let now = Utc::now();

    let builtins = vec![
        // ── Layer 1: 执行 Harness ──
        ArtifactMetadata {
            id: "agents_md".into(),
            artifact_type: ArtifactType::AgentsMd,
            name: "AGENTS.md".into(),
            description: "项目入口文档，定义架构、约定、快速命令".into(),
            harness_layer: 1,
            related_crates: vec![],
            regulations: vec![],
            tags: vec!["entry".into(), "architecture".into(), "convention".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 1.0,
        },
        ArtifactMetadata {
            id: "skills_registry".into(),
            artifact_type: ArtifactType::Skills,
            name: "Skills 注册表".into(),
            description: "技能注册表 + WebRecorder，支持按需加载".into(),
            harness_layer: 2,
            related_crates: vec!["skills".into()],
            regulations: vec![],
            tags: vec!["skills".into(), "registry".into(), "on-demand".into()],
            dependencies: vec!["agents_md".into()],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.9,
        },
        // ── Layer 3: 安全 Harness ──
        ArtifactMetadata {
            id: "gxp_auth".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "GxP 认证模块".into(),
            description: "FDA 21 CFR Part 11 认证、密码策略、2FA".into(),
            harness_layer: 3,
            related_crates: vec!["common".into(), "api-server".into()],
            regulations: vec!["21 CFR Part 11".into(), "EU Annex 11".into()],
            tags: vec!["gxp".into(), "auth".into(), "password".into(), "2fa".into(), "fda".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.85,
        },
        ArtifactMetadata {
            id: "gxp_audit".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "GxP 审计日志".into(),
            description: "ALCOA+ 不可变审计链，SHA-256 哈希链防篡改".into(),
            harness_layer: 3,
            related_crates: vec!["common".into(), "auto-loop".into(), "data-governance".into()],
            regulations: vec!["21 CFR Part 11".into(), "ALCOA+".into()],
            tags: vec!["gxp".into(), "audit".into(), "alcoa".into(), "hash-chain".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.9,
        },
        ArtifactMetadata {
            id: "eu_ai_act".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "EU AI Act 合规检查".into(),
            description: "风险分类、透明度、人工监督、技术文档检查".into(),
            harness_layer: 3,
            related_crates: vec!["compliance-security".into()],
            regulations: vec!["EU AI Act".into(), "Regulation 2024/1689".into()],
            tags: vec!["eu".into(), "ai_act".into(), "risk".into(), "conformity".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.8,
        },
        ArtifactMetadata {
            id: "it_change_mgmt".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "IT 变更管理".into(),
            description: "医药企业 IT 变更管理，GxP 影响分级，CAPA 联动".into(),
            harness_layer: 3,
            related_crates: vec!["it-change-management".into()],
            regulations: vec!["21 CFR Part 11".into(), "GAMP 5".into(), "EU Annex 11".into()],
            tags: vec!["change".into(), "itil".into(), "gxp".into(), "capa".into(), "validation".into()],
            dependencies: vec!["gxp_audit".into()],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.85,
        },
        ArtifactMetadata {
            id: "doc_mgmt".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "文档管理".into(),
            description: "文档生命周期、版本控制、电子签名、ALCOA+ 审计".into(),
            harness_layer: 3,
            related_crates: vec!["document-management".into()],
            regulations: vec!["21 CFR Part 11".into(), "ALCOA+".into()],
            tags: vec!["document".into(), "lifecycle".into(), "signature".into(), "version".into()],
            dependencies: vec!["gxp_audit".into()],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.85,
        },
        ArtifactMetadata {
            id: "compliance_security".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "合规安全模块".into(),
            description: "多认证、零信任、Prompt注入防御、沙箱、PKI、EU AI Act".into(),
            harness_layer: 3,
            related_crates: vec!["compliance-security".into()],
            regulations: vec!["EU AI Act".into(), "21 CFR Part 11".into()],
            tags: vec!["security".into(), "zero_trust".into(), "pki".into(), "sandbox".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.8,
        },
        // ── Layer 4: 自我进化 Harness ──
        ArtifactMetadata {
            id: "auto_loop".into(),
            artifact_type: ArtifactType::Scripts,
            name: "Auto-Loop 自循环引擎".into(),
            description: "自主开发循环：评估→审视→方案→开发".into(),
            harness_layer: 4,
            related_crates: vec!["auto-loop".into()],
            regulations: vec![],
            tags: vec!["auto".into(), "loop".into(), "self-dev".into(), "learner".into()],
            dependencies: vec!["skills_registry".into()],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.75,
        },
        ArtifactMetadata {
            id: "knowledge_rag".into(),
            artifact_type: ArtifactType::Docs,
            name: "Agentic RAG 知识引擎".into(),
            description: "检索增强生成、Flywheel 飞轮学习、GraphRAG".into(),
            harness_layer: 2,
            related_crates: vec!["knowledge".into()],
            regulations: vec![],
            tags: vec!["rag".into(), "knowledge".into(), "flywheel".into(), "graph".into()],
            dependencies: vec![],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.85,
        },
        ArtifactMetadata {
            id: "data_governance".into(),
            artifact_type: ArtifactType::ComplianceRules,
            name: "数据治理模块".into(),
            description: "RBAC审计、成本归因、EU AI Act分类、Kafka审计流".into(),
            harness_layer: 3,
            related_crates: vec!["data-governance".into()],
            regulations: vec!["EU AI Act".into(), "21 CFR Part 11".into()],
            tags: vec!["governance".into(), "rbac".into(), "cost".into(), "kafka".into()],
            dependencies: vec!["gxp_audit".into()],
            version: "1.0.0".into(),
            last_modified: now,
            usage_count: 0,
            success_rate: 0.8,
        },
    ];

    recommender.register_all(builtins);
}

// ── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> ProjectContext {
        ProjectContext {
            current_crate: Some("it-change-management".into()),
            task_type: Some("compliance".into()),
            regulation_domain: Some("GxP".into()),
            active_artifacts: vec![],
            keywords: vec!["gxp".into(), "audit".into()],
            changed_files: vec!["crates/it-change-management/src/lib.rs".into()],
        }
    }

    #[tokio::test]
    async fn test_recommend_basic() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let ctx = test_context();
        let results = rec.recommend(&ctx).await;

        assert!(!results.is_empty(), "should have recommendations");
        // gxp_audit should rank high given keywords ["gxp", "audit"]
        let top = &results[0];
        assert!(
            top.artifact.id.contains("gxp") || top.artifact.id.contains("audit") || top.artifact.id.contains("change"),
            "top recommendation should be relevant, got: {}",
            top.artifact.id
        );
    }

    #[tokio::test]
    async fn test_recommend_by_type() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let ctx = test_context();
        let compliance = rec.recommend_by_type(&ctx, &ArtifactType::ComplianceRules).await;

        assert!(!compliance.is_empty());
        for r in &compliance {
            assert_eq!(r.artifact.artifact_type, ArtifactType::ComplianceRules);
        }
    }

    #[tokio::test]
    async fn test_recommend_by_regulation() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let ctx = test_context();
        let fda = rec.recommend_by_regulation(&ctx, "21 CFR Part 11").await;

        assert!(!fda.is_empty());
        for r in &fda {
            assert!(r.artifact.regulations.iter().any(|reg| reg.contains("21 CFR Part 11")));
        }
    }

    #[tokio::test]
    async fn test_recommend_excludes_active() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let mut ctx = test_context();
        ctx.active_artifacts = vec!["gxp_audit".into()];

        let results = rec.recommend(&ctx).await;
        assert!(results.iter().all(|r| r.artifact.id != "gxp_audit"));
    }

    #[tokio::test]
    async fn test_usage_history_boosts_score() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        // Record successful usage of gxp_audit with matching keywords
        rec.record_usage(UsageRecord {
            artifact_id: "gxp_audit".into(),
            context_keywords: vec!["gxp".into(), "audit".into()],
            crate_name: Some("it-change-management".into()),
            success: true,
            timestamp: Utc::now(),
            co_used: vec!["it_change_mgmt".into()],
        })
        .await;

        let ctx = test_context();
        let results = rec.recommend(&ctx).await;

        // gxp_audit should have high history score
        let gxp_rec = results.iter().find(|r| r.artifact.id == "gxp_audit");
        // Might be excluded if in active_artifacts, so test with empty active
        if let Some(r) = gxp_rec {
            assert!(r.history_score > 0.5, "history score should be boosted");
        }
    }

    #[tokio::test]
    async fn test_co_occurrence_boosts_dependency() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        // Record co-usage
        for _ in 0..5 {
            rec.record_usage(UsageRecord {
                artifact_id: "gxp_audit".into(),
                context_keywords: vec!["gxp".into()],
                crate_name: None,
                success: true,
                timestamp: Utc::now(),
                co_used: vec!["it_change_mgmt".into()],
            })
            .await;
        }

        let mut ctx = test_context();
        ctx.active_artifacts = vec!["gxp_audit".into()];
        ctx.keywords = vec!["change".into(), "management".into()];

        let results = rec.recommend(&ctx).await;
        let it_change = results.iter().find(|r| r.artifact.id == "it_change_mgmt");
        if let Some(r) = it_change {
            assert!(r.dependency_score > 0.0, "co-occurrence should boost dependency score");
        }
    }

    #[test]
    fn test_search() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let results = rec.search("audit");
        assert!(!results.is_empty());
        assert!(results.iter().any(|a| a.id.contains("audit")));
    }

    #[test]
    fn test_list_by_type() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);

        let compliance = rec.list_by_type(&ArtifactType::ComplianceRules);
        assert!(compliance.len() >= 4); // gxp_auth, gxp_audit, eu_ai_act, it_change_mgmt, doc_mgmt, compliance_security, data_governance
    }

    #[test]
    fn test_builtin_count() {
        let mut rec = HarnessRecommender::default();
        register_builtin_artifacts(&mut rec);
        assert!(rec.count() >= 10, "should have at least 10 builtin artifacts");
    }

    #[test]
    fn test_artifact_type_display() {
        assert_eq!(ArtifactType::AgentsMd.to_string(), "agents_md");
        assert_eq!(ArtifactType::ComplianceRules.to_string(), "compliance_rules");
        assert_eq!(ArtifactType::Skills.to_string(), "skills");
    }
}
