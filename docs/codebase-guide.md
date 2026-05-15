# KIAS 代码库详解

> 系统代码库完整说明，用于故障排除和 Bug 修复

## 1. 项目结构总览

```
kias/
├── Cargo.toml                    # Rust 工作空间配置
├── Cargo.lock                    # 依赖锁定文件
├── AGENTS.md                     # AI Agent 上下文配置
├── README.md                     # 用户使用说明
├── Makefile                      # 构建、测试、检查命令
├── docker/                       # Docker 配置
│   ├── Dockerfile.control-plane  # 控制平面镜像
│   ├── Dockerfile.node-agent     # 节点代理镜像
│   └── docker-compose.yaml       # 本地开发环境
├── helm/                         # K8S Helm Chart
│   └── kias/
│       ├── Chart.yaml
│       ├── values.yaml
│       └── templates/
├── crates/                       # Rust 组件
│   ├── common/                   # 公共库
│   ├── api-server/               # API 服务
│   ├── scheduler/                # 调度器
│   ├── controller/               # 控制器
│   ├── agentsight/               # 可观测组件
│   ├── cache-hub/                # 缓存优化
│   └── knowledge/                # 知识管理
├── dashboard/                    # React 前端
│   ├── package.json
│   ├── src/
│   └── public/
├── scripts/                      # 脚本
│   ├── build.sh                  # 构建脚本
│   ├── start-control-plane.sh    # 启动控制平面
│   ├── start-node-agent.sh       # 启动节点代理
│   └── verify.sh                 # 验证脚本
├── tests/                        # 测试
│   ├── unit/                     # 单元测试
│   ├── integration/              # 集成测试
│   └── e2e/                      # 端到端测试
├── docs/                         # 文档
│   ├── architecture.md           # 架构设计
│   ├── development.md            # 开发文档
│   ├── api.md                    # API 文档
│   └── acceptance-criteria.md    # 验收标准
└── config/                       # 配置
    ├── default.toml              # 默认配置
    ├── local.toml                # 本地配置
    └── production.toml           # 生产配置
```

## 2. 核心模块详解

### 2.1 common（公共库）

**路径**：`crates/common/`

**职责**：提供所有模块共用的工具函数、错误类型、配置结构

**文件结构**：
```
crates/common/
├── Cargo.toml
├── src/
│   ├── lib.rs           # 库入口
│   ├── error.rs         # 错误类型定义
│   ├── config.rs        # 配置结构
│   ├── logging.rs       # 日志初始化
│   ├── metrics.rs       # 指标定义
│   └── utils.rs         # 工具函数
```

**关键类型**：
```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum KiasError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Insufficient resources: {0}")]
    InsufficientResources(String),
    
    #[error("Cache miss: {0}")]
    CacheMiss(String),
    
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

// config.rs
#[derive(Debug, Deserialize)]
pub struct KiasConfig {
    pub api_server: ApiServerConfig,
    pub scheduler: SchedulerConfig,
    pub controller: ControllerConfig,
    pub agentsight: AgentSightConfig,
    pub cache_hub: CacheHubConfig,
    pub knowledge: KnowledgeConfig,
}
```

**故障排除**：
- **配置加载失败**：检查 `config/` 目录下是否有对应环境的配置文件
- **日志不输出**：检查 `RUST_LOG` 环境变量是否设置

---

### 2.2 api-server（API 服务）

**路径**：`crates/api-server/`

**职责**：接收客户端请求，认证授权，路由分发

**文件结构**：
```
crates/api-server/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库入口
│   ├── config.rs        # 配置加载
│   ├── error.rs         # 错误处理
│   ├── handlers/        # 请求处理器
│   │   ├── mod.rs
│   │   ├── agents.rs    # Agent 相关 API
│   │   ├── nodes.rs     # Node 相关 API
│   │   ├── knowledge.rs # 知识管理 API
│   │   └── health.rs    # 健康检查 API
│   ├── routes/          # 路由定义
│   │   ├── mod.rs
│   │   └── api.rs       # API 路由
│   ├── middleware/       # 中间件
│   │   ├── mod.rs
│   │   ├── auth.rs      # 认证中间件
│   │   └── logging.rs   # 日志中间件
│   └── models/          # 数据模型
│       ├── mod.rs
│       ├── agent.rs     # Agent 模型
│       ├── node.rs      # Node 模型
│       └── request.rs   # 请求/响应模型
```

**关键代码**：
```rust
// main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::init();
    
    // 加载配置
    let config = KiasConfig::load()?;
    
    // 创建应用
    let app = create_app(config).await?;
    
    // 启动服务器
    let addr = SocketAddr::from(([0, 0, 0, 0], config.api_server.port));
    tracing::info!("Listening on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

// handlers/agents.rs
pub async fn create_agent(
    State(state): State<AppState>,
    Json(req): Json<CreateAgentRequest>,
) -> Result<Json<Agent>, KiasError> {
    // 验证请求
    req.validate()?;
    
    // 调用 Controller 创建 Agent
    let agent = state.controller.create_agent(req).await?;
    
    Ok(Json(agent))
}
```

**故障排除**：
- **端口占用**：`lsof -i :8080` 检查端口占用
- **启动失败**：检查日志 `RUST_LOG=debug cargo run -p kias-api-server`
- **API 返回 500**：查看日志中的错误堆栈

---

### 2.3 scheduler（调度器）

**路径**：`crates/scheduler/`

**职责**：将 Agent 调度到合适的 Node

**文件结构**：
```
crates/scheduler/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库入口
│   ├── config.rs        # 配置
│   ├── algorithms/      # 调度算法
│   │   ├── mod.rs
│   │   ├── round_robin.rs    # 轮询算法
│   │   ├── least_loaded.rs   # 最少负载算法
│   │   ├── resource_aware.rs # 资源感知算法
│   │   └── cache_aware.rs    # 缓存感知算法（创新）
│   ├── policies/        # 调度策略
│   │   ├── mod.rs
│   │   ├── affinity.rs  # 亲和性策略
│   │   └── priority.rs  # 优先级策略
│   └── optimizer/       # 优化器
│       ├── mod.rs
│       └── cache_optimizer.rs # 缓存优化器
```

**关键代码**：
```rust
// algorithms/cache_aware.rs
pub struct CacheAwareScheduler {
    cache_hub: Arc<CacheHub>,
    nodes: Vec<Node>,
}

impl Scheduler for CacheAwareScheduler {
    fn schedule(&self, agent: &Agent) -> Result<NodeId, KiasError> {
        // 1. 检查缓存命中
        let prefix = agent.system_prompt_hash();
        
        for node in &self.nodes {
            if self.cache_hub.has_prefix_on_node(&prefix, &node.id) {
                tracing::info!("Cache hit on node {}", node.id);
                return Ok(node.id.clone());
            }
        }
        
        // 2. 降级到最少负载算法
        let node = self.nodes
            .iter()
            .min_by_key(|n| n.current_load())
            .ok_or(KiasError::NoAvailableNodes)?;
        
        Ok(node.id.clone())
    }
}
```

**故障排除**：
- **调度延迟高**：检查节点数量和负载情况
- **调度不均匀**：检查调度算法配置

---

### 2.4 controller（控制器）

**路径**：`crates/controller/`

**职责**：管理 Agent 生命周期，心跳监控，故障自动恢复

**文件结构**：
```
crates/controller/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口（集成 HealthChecker）
│   ├── lib.rs           # 库入口
│   ├── state.rs         # 状态管理（AgentStatus, AgentInfo, ControllerState）
│   ├── reconciler.rs    # 调和器（Reconciler trait, DefaultReconciler）
│   ├── heartbeat.rs     # 心跳监控（HeartbeatMonitor, HeartbeatConfig）
│   ├── recovery.rs      # 故障恢复（RecoveryManager, 指数退避）
│   └── health.rs        # 健康检查循环（HealthChecker, HealthCheckSummary）
```

**关键类型**：
```rust
// state.rs — Agent 状态
pub enum AgentStatus { Pending, Running, Failed, Unresponsive, Succeeded }

pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub retry_count: u32,
    pub consecutive_failures: u32,
}

// heartbeat.rs — 心跳监控
pub struct HeartbeatMonitor {
    agents: HashMap<String, HeartbeatRecord>,
    config: HeartbeatConfig,  // timeout_secs, check_interval_ms
}

// recovery.rs — 故障恢复（指数退避）
pub struct RecoveryManager {
    agents: HashMap<String, RecoveryState>,
    config: RecoveryConfig,  // max_retries, base_backoff, max_backoff, multiplier
}

// health.rs — 集成健康检查
pub struct HealthChecker {
    heartbeat: HeartbeatMonitor,
    recovery: RecoveryManager,
    state: ControllerState,
}
```

**关键流程**：
```
HealthChecker::check()
  → HeartbeatMonitor 检查超时
    → RecoveryManager 恢复故障 Agent
      → ControllerState 同步
```

**测试**：50 个单元测试（heartbeat: 10, recovery: 14, health: 10, state: 10, reconciler: 4, 旧测试: 2）

**故障排除**：
- **Agent 创建失败**：检查节点资源是否充足
- **故障恢复不触发**：检查 HeartbeatConfig.timeout_secs 配置
- **永久失败误判**：检查 RecoveryConfig.max_retries 设置

---

### 2.5 agentsight（可观测组件）

**路径**：`crates/agentsight/`

**职责**：Token 追踪、Agent 健康监控、eBPF 探针

**文件结构**：
```
crates/agentsight/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库入口
│   ├── config.rs        # 配置
│   ├── probes/          # eBPF 探针
│   │   ├── mod.rs
│   │   ├── ebpf.rs      # eBPF 加载器
│   │   ├── procmon.rs   # 进程监控
│   │   └── network.rs   # 网络监控
│   ├── analyzer/        # 分析器
│   │   ├── mod.rs
│   │   ├── token.rs     # Token 分析
│   │   ├── health.rs    # 健康分析
│   │   └── audit.rs     # 审计分析
│   └── dashboard/       # Dashboard API
│       ├── mod.rs
│       └── handlers.rs  # API 处理器
```

**关键代码**：
```rust
// analyzer/token.rs
pub struct TokenAnalyzer {
    // Agent -> Token 使用统计
    usage: DashMap<String, TokenUsage>,
}

impl TokenAnalyzer {
    pub fn record(&self, agent: &str, input: u64, output: u64, cache_hit: bool) {
        let mut usage = self.usage.entry(agent.to_string()).or_default();
        usage.input_tokens += input;
        usage.output_tokens += output;
        
        if cache_hit {
            usage.cache_hits += 1;
            usage.cost += self.calculate_cost(input, output) * 0.1; // 缓存命中成本降低 90%
        } else {
            usage.cache_misses += 1;
            usage.cost += self.calculate_cost(input, output);
        }
    }
    
    pub fn get_report(&self, agent: &str) -> Option<TokenReport> {
        self.usage.get(agent).map(|u| TokenReport {
            agent: agent.to_string(),
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cache_hit_rate: u.cache_hits as f64 / (u.cache_hits + u.cache_misses) as f64,
            total_cost: u.cost,
        })
    }
}

// probes/ebpf.rs
pub struct EbpfProbe {
    program: EbpfProgram,
}

impl EbpfProbe {
    pub async fn load() -> Result<Self, KiasError> {
        // 加载 eBPF 程序
        let program = EbpfProgram::load(include_bytes!("probe.o"))?;
        Ok(Self { program })
    }
    
    pub async fn attach(&self, pid: u32) -> Result<(), KiasError> {
        // 附加到进程
        self.program.attach_uprobe(pid, "libc", "write")?;
        self.program.attach_uprobe(pid, "libc", "read")?;
        Ok(())
    }
}
```

**故障排除**：
- **eBPF 加载失败**：检查内核版本 >= 5.10 且启用 BTF
- **Token 统计不准**：检查 eBPF 探针是否正确附加

---

### 2.6 cache-hub（缓存优化）

**路径**：`crates/cache-hub/`

**职责**：KV Cache 优化，降低 Token 成本

**文件结构**：
```
crates/cache-hub/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库入口
│   ├── config.rs        # 配置
│   ├── prefix/          # Prefix Cache
│   │   ├── mod.rs
│   │   └── cache.rs     # 前缀缓存实现
│   ├── semantic/        # 语义缓存
│   │   ├── mod.rs
│   │   └── cache.rs     # 语义缓存实现
│   └── distributed/     # 分布式缓存
│       └── hub.rs       # 缓存协调
```

**关键代码**：
```rust
// prefix/cache.rs
pub struct PrefixCache {
    // 前缀哈希 -> KV Cache
    cache: LruCache<u64, KvCache>,
    // 统计信息
    stats: CacheStats,
}

impl PrefixCache {
    pub async fn get(&self, prefix: &str) -> Option<KvCache> {
        let hash = hash_prefix(prefix);
        self.cache.get(&hash).cloned()
    }
    
    pub async fn put(&mut self, prefix: &str, kv: KvCache) {
        let hash = hash_prefix(prefix);
        self.cache.put(hash, kv);
        self.stats.inserts += 1;
    }
    
    pub fn hit_rate(&self) -> f64 {
        self.stats.hits as f64 / (self.stats.hits + self.stats.misses) as f64
    }
}

// semantic/cache.rs
pub struct SemanticCache {
    // 向量索引
    index: HnswIndex,
    // 语义哈希 -> 响应
    cache: HashMap<u64, String>,
}

impl SemanticCache {
    pub async fn get(&self, query: &str) -> Option<String> {
        let embedding = embed(query).await?;
        let neighbors = self.index.search(embedding, 5);
        
        for neighbor in neighbors {
            if neighbor.distance < 0.05 {  // 相似度 > 95%
                return self.cache.get(&neighbor.id).cloned();
            }
        }
        None
    }
}
```

**故障排除**：
- **缓存命中率低**：检查前缀是否一致，TTL 是否过短
- **内存占用高**：调整 `max_size` 配置

---

### 2.7 knowledge（知识管理）

**路径**：`crates/knowledge/`

**职责**：知识图谱、混合检索

**文件结构**：
```
crates/knowledge/
├── Cargo.toml
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库入口
│   ├── config.rs        # 配置
│   ├── wiki/            # Wiki 层
│   │   ├── mod.rs
│   │   ├── schema.rs    # Schema 定义
│   │   ├── page.rs      # 页面管理
│   │   └── index.rs     # 索引管理
│   ├── retrieval/       # 检索层
│   │   ├── mod.rs
│   │   ├── vector.rs    # 向量检索
│   │   ├── keyword.rs   # 关键词检索
│   │   └── hybrid.rs    # 混合检索
│   └── graph/           # 图谱层
│       ├── mod.rs
│       ├── entity.rs    # 实体抽取
│       ├── relation.rs  # 关系分类
│       └── backlink.rs  # 反向链接
```

**关键代码**：
```rust
// retrieval/hybrid.rs
pub struct HybridRetrieval {
    vector_index: VectorIndex,
    keyword_index: KeywordIndex,
    graph: KnowledgeGraph,
}

impl HybridRetrieval {
    pub async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        // 1. 向量搜索
        let vector_results = self.vector_index.search(query, limit * 2).await;
        
        // 2. 关键词搜索
        let keyword_results = self.keyword_index.search(query, limit * 2).await;
        
        // 3. 合并结果
        let mut merged = self.merge_results(vector_results, keyword_results);
        
        // 4. 图谱加权
        for result in &mut merged {
            let backlinks = self.graph.get_backlinks(&result.page_id).len();
            result.score += 0.1 * backlinks as f64;
        }
        
        // 5. 排序返回
        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        merged.truncate(limit);
        merged
    }
}

// graph/entity.rs
pub struct EntityExtractor {
    wikilink_pattern: Regex,
    relation_patterns: HashMap<String, Regex>,
}

impl EntityExtractor {
    pub fn extract(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        
        // 提取 wikilink: [[entity/name]]
        for cap in self.wikilink_pattern.captures_iter(text) {
            entities.push(Entity {
                id: cap[1].to_string(),
                source: "wikilink".to_string(),
            });
        }
        
        entities
    }
}
```

**故障排除**：
- **检索结果不准确**：检查向量索引是否需要重建
- **图谱关系缺失**：检查实体抽取规则

---

## 3. 配置说明

### 3.1 配置文件结构

```toml
# config/default.toml

[api_server]
host = "0.0.0.0"
port = 8080
workers = 4

[scheduler]
algorithm = "cache_aware"  # round_robin, least_loaded, resource_aware, cache_aware
interval = 10

[controller]
heartbeat_interval = 5
failure_threshold = 3

[agentsight]
enabled = true
ebpf_enabled = true
metrics_port = 9090

[cache_hub]
prefix_cache_size = 1073741824  # 1GB
semantic_cache_enabled = true
cache_mode = "sqlite"  # sqlite 或 memory

[knowledge]
storage_path = "./knowledge"
vector_db_path = "./data/knowledge_vectors.db"
graph_db_path = "./data/knowledge_graph.db"
```

### 3.2 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `KIAS_CONFIG` | 配置文件路径 | `config/default.toml` |
| `RUST_LOG` | 日志级别 | `info` |
| `KIAS_ETCD_ENDPOINTS` | etcd 地址 | `http://localhost:2379` |

---

## 4. 故障排除指南

### 4.1 启动失败

**问题**：服务启动失败

**排查步骤**：
1. 检查日志：`RUST_LOG=debug cargo run`
2. 检查配置：`cat config/local.toml`
3. 检查端口：`lsof -i :8080`
4. 检查依赖：`etcd` 是否运行

### 4.2 API 返回 500

**问题**：API 返回内部错误

**排查步骤**：
1. 查看日志中的错误堆栈
2. 检查请求参数是否合法
3. 检查后端服务是否正常

### 4.3 调度延迟高

**问题**：Agent 调度耗时过长

**排查步骤**：
1. 检查节点数量：`kias node list`
2. 检查节点负载：`kias node metrics`
3. 检查调度算法配置

### 4.4 缓存命中率低

**问题**：缓存命中率 < 50%

**排查步骤**：
1. 检查前缀是否一致
2. 检查 TTL 配置
3. 检查缓存大小

### 4.5 Token 统计不准

**问题**：Token 使用量统计异常

**排查步骤**：
1. 检查 eBPF 探针是否加载
2. 检查进程是否正确附加
3. 检查日志中的采集记录

---

## 5. 性能调优

### 5.1 API Server

```toml
[api_server]
workers = 8  # 根据 CPU 核数调整
max_connections = 10000
request_timeout = 30
```

### 5.2 Scheduler

```toml
[scheduler]
algorithm = "cache_aware"  # 优先使用缓存感知算法
batch_size = 100  # 批量调度
```

### 5.3 Cache Hub

```toml
[cache_hub]
prefix_cache_size = 2147483648  # 2GB
semantic_cache_enabled = true
cache_mode = "sqlite"
```

---

## 6. 监控指标

### 6.1 Prometheus 指标

```yaml
# API 指标
kias_api_requests_total{method, path, status}
kias_api_request_duration_seconds{method, path, quantile}

# 调度指标
kias_scheduler_decisions_total{algorithm}
kias_scheduler_duration_seconds{algorithm}

# 缓存指标
kias_cache_hits_total{type}
kias_cache_misses_total{type}
kias_cache_hit_rate{type}

# Agent 指标
kias_agent_count{state}
kias_agent_token_usage{agent, type}
```

### 6.2 Grafana Dashboard

导入 `config/grafana-dashboard.json` 查看实时监控。

---

## 7. 日志说明

### 7.1 日志格式

```
2026-05-13T21:00:00.000Z INFO kias_api_server::handlers::agents: Creating agent: my-agent
2026-05-13T21:00:00.100Z DEBUG kias_scheduler::algorithms::cache_aware: Cache miss for prefix abc123
2026-05-13T21:00:00.200Z WARN kias_controller::recovery: Agent my-agent heartbeat timeout
```

### 7.2 日志级别

| 级别 | 用途 |
|------|------|
| `error` | 错误，需要立即处理 |
| `warn` | 警告，需要关注 |
| `info` | 信息，正常运行 |
| `debug` | 调试，开发排查 |
| `trace` | 追踪，详细排查 |

---

## 8. 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| v0.1.0 | 2026-05-13 | 初始版本，基础框架 |