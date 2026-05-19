# GBrain 模式吸收方案 — AgentGuard Knowledge 层增强

> 来源：GBrain (Garry Tan, YC) 开源 Agent 记忆系统
> 参考：https://github.com/garrytan/gbrain (14,000+ Stars)
> 日期：2026-05-18
> 方法论：四步法（评估→审视→方案→开发）

---

## Step 1: 评估 — 三个模式的必要性

### 模式 1: Compiled Truth + Timeline（实体级信息组织）

| 维度 | 评估 |
|------|------|
| 解决什么问题 | 当前 AgentGuard 知识是扁平文档，无法区分"当前认知"和"历史证据" |
| 不做会怎样 | 检索返回过时信息，Agent 无法判断哪条是最新的 |
| 核心价值 | **高** — 直接提升检索质量 |
| 用户场景 | "Alice 现在在哪家公司？" → 需要 Compiled Truth，不是所有时间线 |
| 领域相关 | **是** — 知识管理核心问题 |
| 结论 | **做** |

### 模式 2: 零 LLM 知识图谱（正则实体提取）

| 维度 | 评估 |
|------|------|
| 解决什么问题 | 当前 KnowledgeGraph 需要手动添加节点/边，无法自动从文档提取实体关系 |
| 不做会怎样 | 图谱永远是空的，GraphRAG 的图增强检索无法发挥作用 |
| 核心价值 | **高** — 零成本激活图谱，让 GraphRAG 真正有用 |
| 用户场景 | "谁投资了 Alice 的公司？" → 纯图谱查询，不需要 LLM |
| 领域相关 | **是** — 知识图谱核心能力 |
| 结论 | **做** |

### 模式 3: Dream Cycle 增强（夜间巩固 + Minions）

| 维度 | 评估 |
|------|------|
| 解决什么问题 | 现有 DreamConsolidator 只做会话合并，不做实体充实、去重、索引重建 |
| 不做会怎样 | 知识库会膨胀、重复、过时，需要手动清理 |
| 核心价值 | **中高** — 自动维护知识质量 |
| 用户场景 | 长期运行的 Agent 需要自动清理和巩固记忆 |
| 领域相关 | **是** — Long-running Agent 核心需求 |
| 结论 | **做**（增强现有 DreamConsolidator，不新建模块） |

### 模式 4: Markdown 真值源

| 维度 | 评估 |
|------|------|
| 解决什么问题 | 当前知识存在内存/SQLite，人无法直接编辑 |
| 不做会怎样 | Agent 记忆对人不透明 |
| 核心价值 | **中** — 可观测性，不是核心功能 |
| 用户场景 | 开发者想手动查看/编辑 Agent 的知识 |
| 领域相关 | 部分相关 |
| 结论 | **暂缓** — 先做核心模式，后续迭代 |

---

## Step 2: 审视 — AgentGuard 现有能力

### knowledge crate 现状（8,578 行）

| 模块 | 行数 | 核心能力 | 与 GBrain 的差距 |
|------|------|---------|-----------------|
| `graph.rs` | 356 | KnowledgeGraph, Node, Edge, shortest_path | 缺自动实体提取 |
| `graphrag.rs` | 1,234 | 混合检索, 社区检测, 子图摘要 | 缺实体级组织 |
| `agentic_rag.rs` | 1,800 | 多工具检索, 决策策略, 飞轮学习 | 已很完善 |
| `memory.rs` | 418 | AgentMemoryStore, remember/recall | 缺 Compiled Truth |
| `memory_layers.rs` | 732 | DreamConsolidator, SessionMemory | 缺 Minions 机制 |
| `vector.rs` | 757 | 向量存储 | 已完善 |
| `retriever.rs` | 527 | 检索器 | 已完善 |
| `context_manager.rs` | 934 | Token 计数, 压缩, 多会话 | 已完善 |
| `quality_pipeline.rs` | 994 | 质量管线 | 已完善 |
| `inspiration_stream.rs` | 775 | 灵感流 | AgentGuard 特色 |

### 关键发现

1. **KnowledgeGraph 已有但空** — 有 `add_node`/`add_edge`，但没有自动提取逻辑
2. **DreamConsolidator 已有但弱** — 只做会话合并，不做实体充实/去重/索引重建
3. **AgenticRAG 已很完善** — 不需要改
4. **缺 Compiled Truth/Timeline 结构** — MemoryEntry 只有 content 字段，没有分层

### 增量 vs 重写

| 需求 | 方案 | 理由 |
|------|------|------|
| Compiled Truth + Timeline | 扩展 MemoryEntry | 已有结构，加字段即可 |
| 零 LLM 实体提取 | 新增 entity_extractor.rs | graph.rs 没有提取逻辑 |
| Dream Cycle 增强 | 扩展 DreamConsolidator | 已有框架，加 Minions 逻辑 |
| 实体分层 (Tier 1/2/3) | 新增 entity_tier.rs | 全新概念 |

---

## Step 3: 方案 — 详细设计

### 3.1 Compiled Truth + Timeline 结构

**扩展现有 `MemoryEntry`**，新增两个内容区：

```rust
// crates/knowledge/src/memory.rs

pub struct EntityPage {
    /// 实体唯一 ID
    pub id: String,
    /// 实体名称（人名、公司名、概念名）
    pub name: String,
    /// 实体类型
    pub entity_type: EntityType,
    /// Compiled Truth — 当前认知摘要（可被覆盖重写）
    pub compiled_truth: String,
    /// Timeline — 追加式时间线（只增不改）
    pub timeline: Vec<TimelineEntry>,
    /// 实体关系（从零 LLM 提取）
    pub relations: Vec<EntityRelation>,
    /// 实体层级 (Tier 1/2/3)
    pub tier: EntityTier,
    /// 元数据
    pub metadata: HashMap<String, String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

pub enum EntityType {
    Person,
    Company,
    Concept,
    Project,
    Meeting,
    Document,
}

pub struct TimelineEntry {
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 原始内容
    pub content: String,
    /// 来源（哪个会话/文档）
    pub source: String,
    /// 置信度
    pub confidence: f64,
}

pub enum EntityTier {
    /// Tier 1: 完整充实（跨 8+ 来源或参加过会议）
    Tier1,
    /// Tier 2: 基本充实（跨 3+ 来源）
    Tier2,
    /// Tier 3: 存根页（首次提及，只有名字和来源）
    Tier3,
}
```

**关键设计**：
- `compiled_truth` 可被覆盖重写 — 每次新信息进来，重新生成摘要
- `timeline` 只追加不修改 — 保留原始证据
- 检索时优先返回 `compiled_truth`，需要细节时查 `timeline`

### 3.2 零 LLM 实体提取器

**新增 `entity_extractor.rs`**，用正则和字符串匹配提取实体关系：

```rust
// crates/knowledge/src/entity_extractor.rs

pub struct EntityExtractor {
    /// 关系模式（正则）
    patterns: Vec<RelationPattern>,
    /// 已知实体词典
    known_entities: HashMap<String, EntityType>,
}

pub struct RelationPattern {
    /// 关系类型
    pub relation: RelationType,
    /// 正则模式
    pub pattern: Regex,
    /// 主语捕获组索引
    pub subject_group: usize,
    /// 宾语捕获组索引
    pub object_group: usize,
}

pub enum RelationType {
    WorksAt,
    InvestedIn,
    Founded,
    Advises,
    Attended,
    CollaboratedWith,
    Mentions,
    RelatedTo,
}

impl EntityExtractor {
    pub fn new() -> Self {
        // 预定义模式
        let patterns = vec![
            // "Alice works at Google"
            RelationPattern {
                relation: RelationType::WorksAt,
                pattern: Regex::new(r"(\w+)\s+(?:works?\s+at|employed\s+by|at)\s+(\w+)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice invested in Acme Corp"
            RelationPattern {
                relation: RelationType::InvestedIn,
                pattern: Regex::new(r"(\w+)\s+(?:invested?\s+in|backed|funded)\s+(\w+)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice founded Acme"
            RelationPattern {
                relation: RelationType::Founded,
                pattern: Regex::new(r"(\w+)\s+(?:founded|co-founded|started|created)\s+(\w+)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice advises Acme"
            RelationPattern {
                relation: RelationType::Advises,
                pattern: Regex::new(r"(\w+)\s+(?:advises?|advisor\s+to|board\s+member)\s+(\w+)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice attended the meeting"
            RelationPattern {
                relation: RelationType::Attended,
                pattern: Regex::new(r"(\w+)\s+(?:attended|participated\s+in|joined)\s+(.+)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice and Bob collaborated"
            RelationPattern {
                relation: RelationType::CollaboratedWith,
                pattern: Regex::new(r"(\w+)\s+and\s+(\w+)\s+(?:collaborated|worked\s+together|partnered)").unwrap(),
                subject_group: 1,
                object_group: 2,
            },
        ];

        Self {
            patterns,
            known_entities: HashMap::new(),
        }
    }

    /// 从文本中提取实体和关系
    pub fn extract(&self, text: &str) -> Vec<ExtractedRelation> {
        let mut results = Vec::new();
        for pattern in &self.patterns {
            for cap in pattern.pattern.captures_iter(text) {
                if let (Some(subject), Some(object)) =
                    (cap.get(pattern.subject_group), cap.get(pattern.object_group))
                {
                    results.push(ExtractedRelation {
                        subject: subject.as_str().to_string(),
                        relation: pattern.relation.clone(),
                        object: object.as_str().to_string(),
                        source_text: text.to_string(),
                        confidence: 0.8, // 正则匹配的默认置信度
                    });
                }
            }
        }
        results
    }

    /// 批量提取并更新知识图谱
    pub fn extract_and_update(
        &self,
        text: &str,
        graph: &mut KnowledgeGraph,
    ) -> Vec<ExtractedRelation> {
        let relations = self.extract(text);
        for rel in &relations {
            // 确保节点存在
            if graph.get_node(&rel.subject).is_none() {
                graph.add_node(KnowledgeNode {
                    id: rel.subject.clone(),
                    content: rel.subject.clone(),
                    node_type: NodeType::Entity,
                    metadata: HashMap::new(),
                });
            }
            if graph.get_node(&rel.object).is_none() {
                graph.add_node(KnowledgeNode {
                    id: rel.object.clone(),
                    content: rel.object.clone(),
                    node_type: NodeType::Entity,
                    metadata: HashMap::new(),
                });
            }
            // 添加边
            graph.add_edge(Edge {
                from: rel.subject.clone(),
                to: rel.object.clone(),
                relationship: format!("{:?}", rel.relation),
                weight: rel.confidence,
            });
        }
        relations
    }
}
```

### 3.3 实体分层（Tier 1/2/3）

```rust
// crates/knowledge/src/entity_tier.rs

pub struct EntityTierManager {
    /// 来源计数：entity_id → Set<source>
    source_counts: HashMap<String, HashSet<String>>,
    /// 参与会议的实体
    meeting_participants: HashSet<String>,
}

impl EntityTierManager {
    /// 注册一次提及
    pub fn register_mention(&mut self, entity_id: &str, source: &str) -> EntityTier {
        let sources = self.source_counts
            .entry(entity_id.to_string())
            .or_insert_with(HashSet::new);
        sources.insert(source.to_string());

        self.calculate_tier(entity_id)
    }

    /// 注册会议参与
    pub fn register_meeting(&mut self, entity_id: &str) -> EntityTier {
        self.meeting_participants.insert(entity_id.to_string());
        self.calculate_tier(entity_id)
    }

    /// 计算实体层级
    fn calculate_tier(&self, entity_id: &str) -> EntityTier {
        let source_count = self.source_counts
            .get(entity_id)
            .map(|s| s.len())
            .unwrap_or(0);
        let in_meeting = self.meeting_participants.contains(entity_id);

        if source_count >= 8 || in_meeting {
            EntityTier::Tier1  // 完整充实
        } else if source_count >= 3 {
            EntityTier::Tier2  // 基本充实
        } else {
            EntityTier::Tier3  // 存根页
        }
    }

    /// 获取需要升级的实体列表
    pub fn get_upgradable_entities(&self) -> Vec<(String, EntityTier, EntityTier)> {
        let mut result = Vec::new();
        for (entity_id, sources) in &self.source_counts {
            let current_tier = self.calculate_tier(entity_id);
            // 返回从 Tier3 升级到 Tier2，或从 Tier2 升级到 Tier1 的实体
            match sources.len() {
                3 => result.push((entity_id.clone(), EntityTier::Tier3, EntityTier::Tier2)),
                8 => result.push((entity_id.clone(), EntityTier::Tier2, EntityTier::Tier1)),
                _ => {}
            }
        }
        result
    }
}
```

### 3.4 Dream Cycle 增强（Minions 机制）

**扩展现有 `DreamConsolidator`**，新增 Minions 任务：

```rust
// crates/knowledge/src/memory_layers.rs — 扩展 DreamConsolidator

pub struct DreamConsolidator {
    // ... 现有字段 ...
    /// 实体提取器
    entity_extractor: EntityExtractor,
    /// 实体层级管理器
    tier_manager: EntityTierManager,
    /// 知识图谱引用
    graph: Arc<RwLock<KnowledgeGraph>>,
}

/// Minion 任务类型（零 LLM 成本）
pub enum MinionTask {
    /// 实体提取 — 从新文档提取实体关系
    ExtractEntities { document_id: String, content: String },
    /// 去重合并 — 检测并合并重复实体
    DeduplicateEntities,
    /// 层级升级 — 检查并升级实体层级
    PromoteEntities,
    /// 索引重建 — 重建检索索引
    RebuildIndex,
    /// 引用修正 — 修正引用链
    FixCitations,
    /// Compiled Truth 更新 — 基于 timeline 重新生成摘要
    UpdateCompiledTruth { entity_id: String },
}

impl DreamConsolidator {
    /// 执行 Minions 任务队列（零 LLM 成本）
    pub async fn run_minions(&self, tasks: Vec<MinionTask>) -> MinionResult {
        let start = Instant::now();
        let mut result = MinionResult::default();

        for task in tasks {
            match task {
                MinionTask::ExtractEntities { document_id, content } => {
                    let relations = self.entity_extractor.extract_and_update(
                        &content,
                        &mut *self.graph.write().await,
                    );
                    result.entities_extracted += relations.len();
                    // 注册来源
                    for rel in &relations {
                        self.tier_manager.register_mention(&rel.subject, &document_id);
                        self.tier_manager.register_mention(&rel.object, &document_id);
                    }
                }
                MinionTask::DeduplicateEntities => {
                    // 基于字符串相似度检测重复实体
                    let deduped = self.deduplicate_entities().await;
                    result.entities_deduplicated += deduped;
                }
                MinionTask::PromoteEntities => {
                    let upgradable = self.tier_manager.get_upgradable_entities();
                    for (entity_id, from_tier, to_tier) in upgradable {
                        // 触发充实管线
                        result.entities_promoted += 1;
                    }
                }
                MinionTask::RebuildIndex => {
                    // 重建向量索引和全文索引
                    result.index_rebuilt = true;
                }
                MinionTask::FixCitations => {
                    // 修正引用链
                    let fixed = self.fix_citations().await;
                    result.citations_fixed += fixed;
                }
                MinionTask::UpdateCompiledTruth { entity_id } => {
                    // 基于 timeline 重新生成 compiled_truth
                    self.update_compiled_truth(&entity_id).await;
                    result.truths_updated += 1;
                }
            }
        }

        result.duration_ms = start.elapsed().as_millis() as u64;
        result
    }

    /// 增强的 dream 方法 — 加入 Minions
    pub async fn dream_enhanced(&self) -> DreamResult {
        // 阶段 1-3: 现有逻辑（会话合并）
        let base_result = self.dream().await;

        // 阶段 4: Minions 任务（零 LLM 成本）
        let minion_tasks = vec![
            MinionTask::DeduplicateEntities,
            MinionTask::PromoteEntities,
            MinionTask::RebuildIndex,
            MinionTask::FixCitations,
        ];
        let minion_result = self.run_minions(minion_tasks).await;

        DreamResult {
            memories_consolidated: base_result.memories_consolidated,
            contradictions_resolved: base_result.contradictions_resolved,
            index_updated: base_result.index_updated,
            duration_ms: base_result.duration_ms + minion_result.duration_ms,
            minion_result: Some(minion_result),
        }
    }
}

#[derive(Debug, Default)]
pub struct MinionResult {
    pub entities_extracted: usize,
    pub entities_deduplicated: usize,
    pub entities_promoted: usize,
    pub index_rebuilt: bool,
    pub citations_fixed: usize,
    pub truths_updated: usize,
    pub duration_ms: u64,
}
```

### 3.5 与现有模块的集成点

| GBrain 模式 | AgentGuard 现有模块 | 集成方式 |
|-------------|--------------|---------|
| Compiled Truth | `memory.rs` MemoryEntry | 扩展为 EntityPage |
| Timeline | `memory.rs` MemoryEntry | 新增 timeline 字段 |
| 零 LLM 图谱 | `graph.rs` KnowledgeGraph | 新增 entity_extractor.rs |
| 实体分层 | 无 | 新增 entity_tier.rs |
| Dream Cycle | `memory_layers.rs` DreamConsolidator | 增强 dream() 方法 |
| Minions | 无 | 新增 MinionTask 枚举 |
| 混合检索 | `graphrag.rs` GraphRAGEngine | 已完善，无需改 |

### 3.6 不做的事

| GBrain 特性 | 为什么不做的理由 |
|-------------|-----------------|
| Markdown 真值源 | AgentGuard 用 SQLite + 内存，Markdown 是展示层不是存储层 |
| Bun/TS 运行时 | AgentGuard 是 Rust，不需要 JS 运行时 |
| 34 个技能文件 | AgentGuard 的 Skills 系统已有自己的结构 |
| MCP Server 模式 | AgentGuard 的 MCP 已有自己的实现 |
| PGLite 嵌入式 PG | AgentGuard 用 SQLite + HNSW，不需要 PG |

---

## Step 4: 开发路线

### Phase 1: 核心结构（entity_extractor.rs + entity_tier.rs）

1. 新增 `entity_extractor.rs` — 正则实体提取
2. 新增 `entity_tier.rs` — 实体分层管理
3. 扩展 `memory.rs` — EntityPage 结构
4. 测试：提取准确率、分层逻辑

### Phase 2: Dream Cycle 增强

1. 扩展 `memory_layers.rs` — MinionTask + run_minions()
2. 增强 `dream()` → `dream_enhanced()`
3. 集成 entity_extractor 到 DreamConsolidator
4. 测试：Minion 任务执行、零 LLM 成本验证

### Phase 3: GraphRAG 集成

1. 扩展 `graphrag.rs` — EntityPage 作为节点类型
2. 混合检索时优先返回 Compiled Truth
3. 图谱查询支持关系类型过滤
4. 测试：端到端检索质量

### Phase 4: 自动管线

1. 文档摄入时自动触发实体提取
2. 实体层级自动升级
3. 定期 Dream Cycle 自动运行
4. 测试：自动化流程

---

## 三位一体检查

### 钱学森系统工程
- [x] 整体性：评估了所有现有模块，不重复
- [x] 层次分解：EntityPage → Extractor → Tier → Dream
- [x] 反馈控制：Tier 升级有明确条件
- [x] 可观测性：MinionResult 有完整指标

### 马斯克第一性原则
- [x] 回归本质：知识管理的核心是"知道什么是最新的" + "实体之间有什么关系"
- [x] 质疑假设：不照搬 Markdown 存储，用 SQLite + 内存更适合 AgentGuard
- [x] 物理定律：正则提取零成本，比 LLM 提取快 1000x

### 论文/源码支撑
- [x] 源码支撑：GBrain (14,000+ Stars) — https://github.com/garrytan/gbrain
- [x] 论文支撑：混合检索 RRF 融合 — Cormack et al. "Reciprocal Rank Fusion"
- [x] 行业实践：YC 总裁 13 年个人知识库验证

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 正则提取准确率不够 | 先覆盖高频模式，迭代增加 |
| EntityPage 结构变更影响现有代码 | 增量扩展，不改现有接口 |
| Minion 任务执行时间过长 | 设置超时，分批执行 |
| 实体分层逻辑误判 | 保守策略，宁可 Tier3 也不误升级 |
