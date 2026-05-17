# Sembr 深度分析：架构、语义匹配、Fire API 与成本优化

> 来源: [Peakstone-Labs/sembr](https://github.com/Peakstone-Labs/sembr) v1.0
> 日期: 2026-05-17
> 作者: KIAS Research Agent
> 前置: docs/research/sembr-analysis.md（初步分析）

---

## 1. 项目概览

**sembr**（semantic + embrace）是 Peakstone Labs 开源的 **Self-hosted Intent Radar**，核心理念是 **Reverse RAG**：

| | 传统 RAG | sembr (Reverse RAG) |
|---|---|---|
| 触发时机 | 用户查询时 | 持续运行，按意图调度 |
| 存储内容 | 文档向量 | 用户意图向量 |
| 搜索方向 | 文档中搜匹配 | 新文章搜匹配意图 |
| 延迟 | 查询时 | 后台任务 |

**技术栈**: Python 3.12 · FastAPI 0.115 · Pydantic v2 · APScheduler 3.11 · aiosqlite (WAL) · Qdrant 1.17 · BGE-M3 · DeepSeek-V4-Flash · Apache-2.0

**资源需求**: 4GB RAM 即可运行（实测 ~1GB），53 个预置信息源

---

## 2. Agent-First 三层架构（深度解析）

sembr 的 "Agent-first" 不仅是 API 设计，而是一整套 **为 AI Agent 消费而设计** 的架构哲学：

### 层 1: Agent 安装层 (INSTALL.md)

```text
Phase 1: 硬件自检（Agent 自主检测 RAM/Disk/Docker）
Phase 2: Docker 环境搭建
Phase 3: 仓库克隆 + 依赖拉取（后台运行，与用户交互并行）
Phase 4: API Key 验证（交互式，Agent 引导用户完成）
Phase 5: 接入模式选择（localhost/LAN/公网）
Phase 6: 服务启动 + 健康检查验证
```

**关键设计**: 安装文档用**指令式语言**写给 Agent 而非人类，Agent 读 INSTALL.md 就能自主完成全部部署。公网部署还有 PUBLIC_INSTALL.md 分支（DNS 检查、Caddy/TLS、ufw 防火墙）。

**KIAS 对标**: KIAS 有 `scripts/start-control-plane.sh` 但缺少 Agent 可读的 INSTALL.md。需要补充。

### 层 2: Agent Skills 包 (agent/sembr/)

5 文件标准格式，教 Agent 如何操作运行中的实例：

| 文件 | 内容 |
|------|------|
| `SKILL.md` | 认证模型、fire 端点决策矩阵、防护规则 |
| `references/endpoints.md` | 31 个端点完整清单（feeds/intents/fire/settings/prompts） |
| `references/schemas.md` | IntentCreate/FeedCreate JSON 结构 |
| `references/recipes.md` | 可复制的 curl + Python httpx 示例 |
| `references/errors.md` | HTTP 状态码表和错误响应格式 |

**关键设计**: Skills 包是**标准格式**（[Agent Skills](https://agentskills.io) 规范），Claude Code 可自动加载（`cp -r agent/sembr ~/.claude/skills/sembr`），其他平台也可直接使用。

**KIAS 对标**: KIAS 有 `crates/skills/` 技能注册表但未对齐此标准格式。

### 层 3: Agent Fire API (/api/external/intents/{id}/fire)

这是三层架构的**核心连接点**——Agent 通过一个同步 HTTP 调用就能获得完整的语义匹配结果。

```python
# sembr 的 external_fire.py 核心流程
POST /api/external/intents/{intent_id}/fire
├── 验证 intent 存在 + 是 cron 模式（非 event 模式）
├── 速率限制检查（1/intent/60s，与内部 fire 共享桶）
├── 构建 ScanOptions（lookback/threshold/feed_ids 可覆盖）
├── scan_once() → Qdrant ANN 搜索
│   ├── write_match_seen=False（不写状态，幂等）
│   └── propagate_qdrant_errors=True（区分"0命中"和"Qdrant宕机"）
├── 0 命中 → 直接返回，跳过 LLM
└── 有命中 → pipeline.compute_summary(matches)
    ├── 成功 → 返回 matches + summary
    └── 失败 → 返回 matches + summary_error（路径已脱敏）
```

**三个 Fire 端点对比**:

| 端点 | 同步/异步 | 写 match_seen | 触发通知 | 用途 |
|------|----------|--------------|---------|------|
| `POST /api/external/intents/{id}/fire` | **同步** | ❌ | ❌ | Agent 诊断，幂等 |
| `POST /intents/{id}/fire` | 异步(202+task_id) | ✅ | ✅ | 运维发邮件摘要 |
| `POST /feeds/{id}/fire?dry_run=true` | 异步 | ❌ | ❌ | 测试新 RSS 源 |

**ExternalFireRequest 结构**:
```python
class ExternalFireRequest(BaseModel):
    lookback_seconds: int | None = None  # 300 - 2,592,000
    threshold: float | None = None       # 0.20 - 0.95（比创建时更宽）
    skip_seen: bool | None = None
    feed_ids: list[int] | None = None    # None=全部, []=无, [1,3]=子集
```

**ExternalFireResponse 结构**:
```python
class ExternalFireResponse(BaseModel):
    intent_id: int
    match_count: int
    matches: list[ExternalFireMatch]  # article_id, score, title, url, published_at, feed_id
    summary: str | None               # LLM 生成的摘要
    summary_error: str | None         # 脱敏后的错误信息（≤200字符，路径/URL已清除）
```

---

## 3. 意图语义匹配（深度解析）

### 3.1 双 Collection Qdrant 设计

```text
intents_current  → 意图向量（全精度，1 point/intent）
news_current     → 文章向量（INT8 标量量化，payload 含 ingested_at_ts + feed_id）
```

- 两个 Collection 都通过 **alias** 访问（`intents_current` / `news_current`），模型升级时可原子切换
- Collection 命名: `news_{model}_{version}`（如 `news_bge-m3_v1`），支持零停机模型迁移
- 每个 payload 都携带 `embedding_model_version`，部分切换可识别

### 3.2 匹配流程

```text
意图创建 → 文本嵌入一次 → 存入 intents_current
                                     ↓
每篇文章 → 嵌入 → 存入 news_current
                         ↓
定时触发（cron/event）→ query_points(
    query=intent_vector,
    score_threshold=0.75,  # 可配置 0.60-0.95
    query_filter=ingested_at_ts > lookback_cutoff AND feed_id IN (...)
)
                         ↓
匹配结果 → LLM 摘要 → 推送
```

### 3.3 两种调度模式

| 模式 | 触发方式 | 配置 | 状态写入 |
|------|---------|------|---------|
| **Cron** | APScheduler 定时 tick | preset(hourly/daily/weekly) + lookback_seconds | 写 match_seen |
| **Event** | 文章嵌入后实时评分 | trigger_count + max_wait_seconds | 不写 match_seen |

**Event 模式细节**:
- 意图向量缓存在进程内 `EventIntentCache`
- 每批文章嵌入后，纯 Python dot product 评分（1024 维，因 BGE-M3 已 L2 归一化，dot = cosine）
- 命中文章按标题相似度（≥0.85 SequenceMatcher）分组缓冲
- 达到 `trigger_count` 或超过 `max_wait_seconds` 时 flush

### 3.4 去重机制（双层）

1. **精确去重**: `MD5(url + title)` 指纹，collector 层跳过已见文章
2. **语义去重**: `match_seen` 表记录 `(intent_id, article_id)`，cron 重扫不重复触发
   - intent 删除时级联清理
   - intent 文本变更时清除该 intent 的 match_seen（重新嵌入后可重新匹配）

---

## 4. Fire API 设计哲学（深度解析）

### 4.1 设计约束

1. **同步返回**: Agent 不需要轮询，一次 HTTP 获得完整结果（通常几十秒）
2. **无副作用**: 不写 match_seen、不触发通知、幂等可重试
3. **参数可覆盖**: lookback/threshold/feed_ids 都可覆盖 intent 存储值
4. **宽阈值范围**: 创建时 0.60-0.95，fire 时 0.20-0.95（诊断用低阈值扫底）
5. **安全脱敏**: 错误信息中路径/URL 被清除，上限 200 字符
6. **速率限制**: 1/intent/60s，与内部 fire 共享桶

### 4.2 防护规则（SKILL.md 明确列出）

```
⚠️ 诊断用 POST /api/external/.../fire，永远不要 POST /intents/{id}/fire（会发邮件！）
⚠️ 不要随意 PUT /intents/{id} 改 text（会清除 match_seen 导致重复触发）
⚠️ 不要 POST /api/settings/save 未经确认（可能触发进程重启）
⚠️ 429 = 睡 ≥60s，不要加重试
⚠️ DASHBOARD_TOKEN 不要提交/存储
```

---

## 5. 成本优化策略（深度解析）

### 5.1 免费嵌入层

```text
BGE-M3 via SiliconFlow → 完全免费，无限调用
1024 维，8192 token 上下文，中英双语
批量大小 = 32（SiliconFlow 单请求限制）
```

**替代方案**: 任何 OpenAI-compatible `/v1/embeddings` 端点（Ollama、mlx-lm、Together、Groq）

### 5.2 按需 LLM

```text
默认: DeepSeek-V4-Flash via SiliconFlow
输入: $0.14 / 1M tokens
输出: $0.28 / 1M tokens
```

**关键优化**: LLM **只在语义匹配命中后**才调用。0 命中直接返回，不消耗 LLM token。

### 5.3 向量量化

```text
news_current: INT8 标量量化
  - 量化向量存 RAM（always_ram=True）
  - 全精度向量存磁盘（on_disk=True）
  - 10M 向量 @ 1024 维 ≈ 600MB RAM

intents_current: 全精度（意图数量少，查询端精度重要）
```

### 5.4 成本对比

```text
sembr 典型日成本: ~$0.014/intent/day
  = 免费嵌入 + 仅命中时 LLM（几美分/日摘要）

对比 Perplexity API 包装:
  10 intents × 24 polls/day × 365 days = 87,600 次 API 调用
  每次 $0.005-0.02 → $438-$1,752/年

sembr: 10 intents × 365 days × $0.014 = $51.10/年
```

---

## 6. KIAS 映射与升级方案

### 6.1 SkillMatcher 向量化升级

**当前状态** (crates/team-engine/src/skill_matcher.rs):
```rust
score = capability_match * 0.6     // 关键词精确匹配
      + availability * 0.2
      + (1.0 - load) * 0.15
      + historical_success * 0.05
```

**问题**: `capability_match` 是精确字符串匹配（`agent.capabilities.get(cap)`），无法处理语义相似的描述。例如 "shell_exec" 不匹配 "command_line_execution"。

**升级方案** (借鉴 sembr 双 Collection):

```text
Phase 1: Agent 能力向量化
  - AgentProfile 注册时，将 capabilities + specializations 文本嵌入为向量
  - 存入 Qdrant agent_capabilities collection
  - 使用 BGE-M3（免费，中英双语）

Phase 2: 任务语义匹配
  - 任务描述文本嵌入 → 与 agent_capabilities 做 ANN 搜索
  - score_threshold 替代精确匹配
  - 保留 load/availability/success_rate 作为排序因子

Phase 3: 双 Collection
  - agent_vectors: Agent 能力向量（全精度）
  - task_vectors: 任务描述向量（可量化）
  - alias 管理支持模型升级

新评分公式:
  score = semantic_match * 0.50      // HNSW ANN 语义匹配
        + capability_exact * 0.15    // 保留精确匹配作为补充
        + availability * 0.15
        + (1.0 - load) * 0.12
        + historical_success * 0.08
```

### 6.2 A2A Fire 端点

**当前状态**: A2ARouter 有 5 种路由策略（Direct/Capability/LoadBalanced/Broadcast/Chain），但没有诊断端点。

**新增 `POST /api/external/tasks/{task_id}/fire`**:

```rust
// crates/api-server/src/handlers/a2a_fire.rs

#[derive(Deserialize)]
struct A2AFireRequest {
    task_description: String,           // 任务描述文本
    required_capabilities: Option<Vec<String>>,  // 可选的精确能力要求
    strategy: Option<RoutingStrategy>,  // 路由策略覆盖
    max_agents: Option<usize>,          // 最大返回数
    threshold: Option<f32>,             // 匹配阈值覆盖 (0.20-0.95)
}

#[derive(Serialize)]
struct A2AFireResponse {
    task_id: String,
    match_count: usize,
    matches: Vec<AgentMatch>,           // agent_id, score, capabilities, load
    routing_suggestion: Option<String>, // 建议的路由策略
    estimated_cost: Option<f64>,        // 预估 token 消耗
}
```

**设计约束** (对齐 sembr):
- 同步返回（Agent 不需要轮询）
- 无副作用（不写 match_seen，不触发任务分配）
- 幂等（相同请求重复调用结果相同）
- 速率限制（1/task/60s）
- 错误脱敏（不暴露内部路径和堆栈）

### 6.3 免费嵌入层接入

```toml
# crates/knowledge/Cargo.toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }  # HTTP client for SiliconFlow API
```

```rust
// crates/knowledge/src/embedder.rs

pub struct SiliconFlowEmbedder {
    client: reqwest::Client,
    api_key: String,
    model: String,  // "BAAI/bge-m3"
    dimension: usize,  // 1024
}

impl SiliconFlowEmbedder {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // POST https://api.siliconflow.cn/v1/embeddings
        // 免费，无限调用
    }

    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // 批量嵌入，batch_size ≤ 32
    }
}
```

### 6.4 Agent 自部署文档

```text
docs/agent-install/
├── INSTALL.md          # 6 阶段 Agent 安装指南
│   ├── Phase 1: 环境检查（Rust/etcd/SQLite）
│   ├── Phase 2: 构建 KIAS（cargo build --release）
│   ├── Phase 3: 配置生成（~/.kias_env）
│   ├── Phase 4: 控制平面启动
│   ├── Phase 5: 节点代理注册
│   └── Phase 6: 健康检查验证
├── SKILLS_BUNDLE.md    # 5 文件标准格式
└── PUBLIC_DEPLOY.md    # 生产部署指南
```

---

## 7. 实施优先级

| 优先级 | 任务 | 复杂度 | 依赖 |
|--------|------|--------|------|
| **P0** | A2A Fire 端点 | 中 | 无（纯新增 handler） |
| **P0** | 免费嵌入层接入 | 低 | SiliconFlow API key |
| **P1** | SkillMatcher 向量化 | 高 | Qdrant 集成 + 嵌入层 |
| **P1** | Agent Skills 包标准化 | 低 | 文档工作 |
| **P2** | Agent INSTALL.md | 低 | 文档工作 |
| **P2** | 双 Collection 设计 | 中 | SkillMatcher 向量化 |

---

## 8. 关键洞察

1. **Reverse RAG 的本质是"查询实体化"**: 意图不是临时查询，而是一等实体——可命名、编辑、调度、版本化。KIAS 的 Agent 匹配也可以这样：任务描述不是一次性查询，而是可复用的匹配模式。

2. **Fire API 的精髓是"无副作用诊断"**: Agent 可以安全地"试探"而不产生任何实际影响。这对于 A2A 协作至关重要——Agent 可以在分配任务前先"预览"哪些 Agent 能处理。

3. **成本优化的核心是"免费嵌入 + 按需 LLM"**: 嵌入层用免费模型（BGE-M3/SiliconFlow），LLM 只在命中后触发。KIAS 的 Agent 匹配也可以遵循此模式——语义匹配免费，实际任务分配才消耗资源。

4. **Agent-first 不只是 API 设计**: 从安装文档（INSTALL.md 写给 Agent 看）到 Skills 包（标准格式可被自动加载）到 Fire API（同步无副作用），每一层都为 Agent 消费而优化。

---

## 参考资料

- [sembr README](https://github.com/Peakstone-Labs/sembr)
- [sembr architecture.md](https://github.com/Peakstone-Labs/sembr/blob/main/docs/architecture.md)
- [sembr matcher module](https://github.com/Peakstone-Labs/sembr/blob/main/docs/modules/matcher.md)
- [sembr api module](https://github.com/Peakstone-Labs/sembr/blob/main/docs/modules/api.md)
- [sembr SKILL.md](https://github.com/Peakstone-Labs/sembr/blob/main/agent/sembr/SKILL.md)
- [sembr external_fire.py](https://github.com/Peakstone-Labs/sembr/blob/main/sembr/api/external_fire.py)
- [sembr CLAUDE.md](https://github.com/Peakstone-Labs/sembr/blob/main/CLAUDE.md)
