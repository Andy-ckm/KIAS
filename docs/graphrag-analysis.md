# Microsoft GraphRAG 深度分析 — AgentGuard 集成方案

> 论文: "From Local to Global: A Graph RAG Approach to Query-Focused Summarization"
> 来源: Microsoft Research, 2024

## 一、GraphRAG 核心思想

### 1.1 传统 RAG 的问题

```
传统 RAG (Vector RAG):
  文档 → 分块 → 向量化 → 相似度搜索 → 返回 top-k

局限:
  ❌ 局部性: 只能找到最相似的单个 chunk
  ❌ 缺全局视角: 无法回答"整体主题是什么"
  ❌ 缺关系推理: 无法回答"A 和 B 有什么关系"
  ❌ 缺层次理解: 无法回答"高层总结是什么"
```

### 1.2 GraphRAG 的解决方案

```
GraphRAG 三层架构:

Layer 1: 实体-关系图谱 (Knowledge Graph)
  文档 → LLM 提取实体+关系 → 构建图
  节点: 实体 (人物、概念、组织、事件...)
  边: 关系 (属于、创建、影响、包含...)

Layer 2: 社区检测 (Community Detection)
  图 → Leiden 算法 → 层次化社区
  每个社区 = 一组紧密相关的实体+关系
  层次结构: 细粒度社区 → 粗粒度社区

Layer 3: 社区摘要 (Community Summaries)
  每个社区 → LLM 生成摘要
  存储: 全局上下文的结构化表示
```

### 1.3 两种查询模式

```
Local Search (局部查询):
  用户问题 → 实体识别 → 图遍历 → 相关社区 → 上下文 → LLM 回答
  适用: 具体事实问题 ("谁创建了 X?")

Global Search (全局查询):
  用户问题 → 所有社区摘要 → Map-Reduce → LLM 综合回答
  适用: 全局性问题 ("整体趋势是什么?")
```

## 二、GraphRAG vs AgentGuard RAG 对比

| 维度 | GraphRAG | AgentGuard 当前 RAG |
|------|----------|---------------|
| **检索方式** | 图遍历 + 社区摘要 | 关键词 + 向量混合 |
| **知识表示** | 实体-关系图谱 | 扁平文档 chunks |
| **全局理解** | ✅ 社区摘要提供全局视角 | ❌ 只有局部 chunks |
| **关系推理** | ✅ 图遍历可推理关系 | ❌ 无关系表示 |
| **层次结构** | ✅ Leiden 社区层次 | ❌ 无层次 |
| **查询类型** | Local + Global | 只有 Local |
| **成本** | 高（需要 LLM 提取实体） | 低（向量化即可） |
| **延迟** | 高（图构建耗时） | 低（实时搜索） |

## 三、AgentGuard 集成方案（渐进式）

### Phase 1: 增强现有 RAG（低成本，高收益）

```
当前: 关键词 + 向量混合搜索
增强: 
  1. 加入 LLM 查询改写（Query Rewriting）
     用户问题 → LLM 改写为多个子问题 → 分别搜索 → 合并结果
  
  2. 加入上下文窗口扩展
     匹配 chunk → 扩展前后各 1 chunk → 提供更多上下文
  
  3. 加入重排序（Re-ranking）
     初始 top-k → Cross-encoder 重排序 → 更精准的 top-n
```

### Phase 2: 轻量级知识图谱（中等成本）

```
目标: 不用完整的 GraphRAG，提取核心价值

实现:
  1. 实体提取（简化版）
     文档 chunks → 关键实体提取 → 存储为 metadata
     不用 LLM，用 NER 模型或规则提取
  
  2. 关系共现（简化版）
     同一 chunk 中出现的实体 → 共现关系
     权重 = 共现频率
  
  3. 实体索引
     实体 → 相关 chunks 列表
     查询时: 识别实体 → 找相关 chunks → 搜索
```

### Phase 3: 社区检测（完整 GraphRAG）

```
目标: 完整的全局查询能力

实现:
  1. LLM 实体-关系提取
     文档 → LLM 提取 (实体, 关系, 实体) 三元组
  
  2. 图构建
     NetworkX 图 → 节点=实体，边=关系
  
  3. Leiden 社区检测
     图 → Leiden 算法 → 层次化社区
  
  4. 社区摘要
     每个社区 → LLM 生成摘要
  
  5. 双模查询
     Local: 实体→图遍历→上下文
     Global: 社区摘要→Map-Reduce→综合
```

## 四、AgentGuard 实施计划

### 4.1 Phase 1 实施（本周）

```rust
// crates/knowledge/src/query_rewrite.rs
pub struct QueryRewriter {
    llm: LlmEngine,
}

impl QueryRewriter {
    /// LLM 改写用户查询为多个子问题
    pub async fn rewrite(&self, query: &str) -> Vec<String> {
        // "AgentGuard 的调度算法有哪些优势?"
        // → ["AgentGuard 调度算法是什么", "调度算法的优势", "与其他调度器对比"]
    }
}

// crates/knowledge/src/reranker.rs
pub struct CrossEncoderReranker {
    model: EmbeddingModel,
}

impl CrossEncoderReranker {
    /// 对初始搜索结果重排序
    pub async fn rerank(&self, query: &str, docs: Vec<Document>) -> Vec<Document> {
        // 计算 query-doc 相关性分数 → 重新排序
    }
}

// crates/knowledge/src/context_expander.rs
pub struct ContextExpander {
    chunk_store: ChunkStore,
}

impl ContextExpander {
    /// 扩展匹配 chunk 的上下文窗口
    pub async fn expand(&self, matched_chunk: &Chunk, window: usize) -> Vec<Chunk> {
        // 返回 matched_chunk 前后各 window 个 chunks
    }
}
```

### 4.2 Phase 2 实施（下周）

```rust
// crates/knowledge/src/entity.rs
pub struct Entity {
    pub name: String,
    pub entity_type: EntityType,  // Person, Concept, Organization, Event
    pub source_chunks: Vec<String>,
    pub metadata: HashMap<String, String>,
}

pub struct Relation {
    pub source: String,
    pub target: String,
    pub relation_type: String,
    pub weight: f64,
    pub evidence_chunks: Vec<String>,
}

// crates/knowledge/src/entity_extractor.rs
pub struct EntityExtractor {
    ner_model: Option<NerModel>,
    rules: Vec<ExtractionRule>,
}

impl EntityExtractor {
    /// 提取实体（规则 + NER 混合）
    pub fn extract(&self, text: &str) -> Vec<Entity> {
        // 1. 规则提取（大写词、引号词、专有名词）
        // 2. NER 模型提取
        // 3. 去重合并
    }
}

// crates/knowledge/src/entity_index.rs
pub struct EntityIndex {
    entities: HashMap<String, Entity>,
    relations: HashMap<(String, String), Relation>,
}

impl EntityIndex {
    /// 实体感知搜索
    pub fn search_with_entities(&self, query: &str, chunks: Vec<Chunk>) -> Vec<Chunk> {
        // 1. 识别查询中的实体
        // 2. 找实体相关的额外 chunks
        // 3. 合并排序返回
    }
}
```

### 4.3 Phase 3 实施（第三周）

```rust
// crates/knowledge/src/graph/community.rs
pub struct Community {
    pub id: String,
    pub level: usize,
    pub entities: Vec<String>,
    pub relations: Vec<Relation>,
    pub summary: String,
}

pub struct CommunityDetector;

impl CommunityDetector {
    /// Leiden 社区检测
    pub fn detect(&self, graph: &KnowledgeGraph) -> Vec<Community> {
        // 1. 构建邻接矩阵
        // 2. Leiden 算法
        // 3. 层次化社区
    }
}

// crates/knowledge/src/graph/global_search.rs
pub struct GlobalSearch {
    communities: Vec<Community>,
}

impl GlobalSearch {
    /// 全局查询: 社区摘要 → Map-Reduce
    pub async fn search(&self, query: &str) -> String {
        // Map: 每个社区摘要 → LLM 评估相关性
        // Reduce: 合并相关社区摘要 → LLM 综合回答
    }
}
```

## 五、成本-收益分析

| Phase | 投入 | 收益 | 风险 |
|-------|------|------|------|
| Phase 1 | 2 天 | 查询质量提升 30-50% | 低 |
| Phase 2 | 3 天 | 关系推理能力 | 中（NER 准确率）|
| Phase 3 | 5 天 | 全局查询能力 | 高（LLM 成本）|

## 六、决策

**采用渐进式集成**：
1. Phase 1 立即实施（查询改写 + 重排序 + 上下文扩展）
2. Phase 2 评估 Phase 1 效果后决定
3. Phase 3 只在需要全局查询时实施

**不直接移植 GraphRAG 的原因**：
- GraphRAG 索引成本高（大量 LLM 调用）
- AgentGuard 主要是代码/技术文档，实体关系相对简单
- 渐进式更符合钱学森"从定性到定量"方法论

---

*参考: Microsoft GraphRAG (https://github.com/microsoft/graphrag)*
*论文: "From Local to Global: A Graph RAG Approach to Query-Focused Summarization"*
