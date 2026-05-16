# KIAS 用户使用手册

> Production-grade AI Agent cluster orchestration built in Rust

---

## 目录

1. [快速开始](#快速开始)
2. [安装部署](#安装部署)
3. [配置说明](#配置说明)
4. [核心功能](#核心功能)
5. [API参考](#api参考)
6. [CLI使用](#cli使用)
7. [AgenticRAG检索](#agenticrag检索)
8. [记忆系统](#记忆系统)
9. [自动循环](#自动循环)
10. [故障排查](#故障排查)

---

## 快速开始

### 前置条件

- Rust 1.95+
- SQLite 3.x
- 2GB+ 可用磁盘空间

### 5分钟启动

```bash
# 1. 克隆仓库
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

# 2. 编译
cargo build --release

# 3. 配置
cp config/kias.toml.example config/kias.toml
# 编辑 config/kias.toml，填入你的API key

# 4. 启动
./target/release/kias-api-server --config config/kias.toml
```

### 验证安装

```bash
# 健康检查
curl http://localhost:8080/healthz

# 查看agents
curl http://localhost:8080/api/v1/agents
```

---

## 安装部署

### 从源码编译

```bash
# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# 编译
cargo build --release

# 运行测试
cargo test --workspace
```

### Docker部署

```bash
docker build -t kias .
docker run -p 8080:8080 -v ./config:/app/config kias
```

---

## 配置说明

### 配置文件

主配置文件: `config/kias.toml`

```toml
[server]
host = "0.0.0.0"
port = 8080

[model]
provider = "openai"
model = "gpt-4o"
api_key = "sk-YOUR_API_KEY_HERE"  # 替换为真实key

[database]
url = "sqlite:kias.db"

[innovation_agent]
enabled = true
sources = ["github", "arxiv"]
schedule_minutes = 20
```

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `KIAS_HOST` | 监听地址 | `0.0.0.0` |
| `KIAS_PORT` | 监听端口 | `8080` |
| `KIAS_DB_URL` | 数据库连接 | `sqlite:kias.db` |
| `KIAS_API_KEY` | API密钥 | 无 |

### 敏感信息处理

**重要:** 不要将真实API key提交到Git。

```bash
# 本地使用真实key
vim config/kias.toml

# 推送到GitHub时用占位符
# config/kias.toml.example 中使用 sk-YOUR_API_KEY_HERE
```

---

## 核心功能

### Agent管理

```bash
# 注册Agent
curl -X POST http://localhost:8080/api/v1/agents \
  -H "Content-Type: application/json" \
  -d '{
    "name": "code-reviewer",
    "description": "代码审查Agent",
    "capabilities": ["code_review", "security_scan"]
  }'

# 列出所有Agent
curl http://localhost:8080/api/v1/agents

# 查看Agent详情
curl http://localhost:8080/api/v1/agents/{agent_id}
```

### Workflow管理

```bash
# 创建工作流
curl -X POST http://localhost:8080/api/v1/workflows \
  -H "Content-Type: application/json" \
  -d '{
    "name": "code-review-flow",
    "steps": [
      {"agent": "code-reviewer", "action": "review"},
      {"agent": "doc-writer", "action": "document"}
    ]
  }'

# 执行工作流
curl -X POST http://localhost:8080/api/v1/workflows/{workflow_id}/run
```

### 自然语言命令

```bash
# 注册Agent
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "注册一个新Agent叫code-reviewer，负责代码审查"}'

# 创建工作流
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "创建工作流：先review代码，再写文档"}'

# 查看状态
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "查看系统状态"}'
```

---

## API参考

### Agent API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/agents` | GET | 列出所有Agent |
| `/api/v1/agents` | POST | 注册新Agent |
| `/api/v1/agents/{id}` | GET | 查看Agent详情 |
| `/api/v1/agents/{id}` | PUT | 更新Agent |
| `/api/v1/agents/{id}` | DELETE | 删除Agent |

### Workflow API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/workflows` | GET | 列出所有工作流 |
| `/api/v1/workflows` | POST | 创建工作流 |
| `/api/v1/workflows/{id}/run` | POST | 执行工作流 |

### NL API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/nl/command` | POST | 自然语言命令 |
| `/api/v1/nl/stream` | POST | 流式NL命令(SSE) |

### 健康检查

| 端点 | 方法 | 说明 |
|------|------|------|
| `/healthz` | GET | 基础健康检查 |
| `/healthz/deep` | GET | 深度健康检查 |

---

## CLI使用

### 安装CLI

```bash
cargo install --path crates/kias-cli
```

### 常用命令

```bash
# 查看状态
kias status

# 注册Agent
kias agent register --name code-reviewer --capabilities code_review

# 创建工作流
kias workflow create --name review-flow --steps review,document

# 执行工作流
kias workflow run review-flow

# 查看日志
kias logs --tail 100
```

---

## AgenticRAG检索

### 概述

AgenticRAG是基于微软论文(2605.05538)的多轮迭代检索系统。不是一次检索就结束，而是给模型配备工具，让模型自主决定搜什么、看哪部分。

### 四层工具

| 工具 | 功能 | 参数 |
|------|------|------|
| Search | 全局搜索 | queries: Vec<String>, max_results: usize |
| Find | 文档内搜索 | ref_id: String, patterns: Vec<String> |
| Open | 窗口化阅读 | ref_id: String, start_line: usize |
| Summarize | 上下文压缩 | preserve_refs: Vec<String> |

### 使用示例

```rust
use kias_knowledge::agentic_rag::*;

// 创建引擎
let store = Arc::new(InMemoryDocumentStore::new());
let engine = AgenticRAGEngine::with_rules(store)?;

// 执行检索
let result = engine.retrieve("revenue analysis").await;

println!("Iterations: {}", result.iterations);
println!("References: {:?}", result.references);
println!("Answer: {}", result.answer);
```

### 配置

```rust
let config = AgenticRAGConfig {
    max_iterations: 15,        // 最大迭代轮数
    max_search_results: 10,    // 每次搜索最大结果
    open_window_lines: 1800,   // Open窗口大小
    token_threshold: 128_000,  // Token阈值
    token_warning_ratio: 0.9,  // 90%预警
    ..Default::default()
};
```

### 企业级特性

- **可观测性:** RetrievalMetrics追踪每次调用
- **审计日志:** AuditEntry记录耗时/参数/结果
- **熔断器:** 单次30s、总2min超时
- **策略引擎:** 可插拔DecisionStrategy（规则/LLM/混合）
- **飞轮学习:** FlywheelLearner积累检索经验

---

## 记忆系统

### 七层记忆架构

KIAS实现了基于Claude Code和AgentScope的分层记忆系统：

| 层级 | 名称 | 功能 | 成本 |
|------|------|------|------|
| L1 | 工具结果存储 | 大结果写磁盘，上下文放2KB预览 | 极低 |
| L3 | 会话记忆 | 实时结构化笔记，零成本压缩 | 零 |
| L6 | 做梦机制 | 跨会话记忆巩固 | 低 |

### L1: 工具结果存储

```rust
use kias_knowledge::memory_layers::*;

let config = ToolResultStoreConfig {
    preview_size: 2048,  // 2KB预览
    storage_path: PathBuf::from(".kias/tool-results"),
    ..Default::default()
};
let store = ToolResultStore::new(config);

// 存储大结果，返回预览
let preview = store.store("result1", "search", &long_content).await;
// 上下文只放preview，完整内容在磁盘
```

### L3: 会话记忆

```rust
let config = SessionMemoryConfig {
    compaction_token_threshold: 100_000,  // 100K tokens触发压缩
    ..Default::default()
};
let mgr = SessionMemoryManager::new(config);

// 创建会话
mgr.create_session("s1", "revenue analysis").await;

// 实时更新笔记
mgr.update("s1", "Found key metric in doc1", Some("doc1")).await;

// 生成摘要（零成本）
let summary = mgr.generate_summary("s1").await;
```

### L6: 做梦机制

```rust
let config = DreamConfig {
    min_sessions_to_dream: 5,  // 5个会话后触发做梦
    dream_interval_hours: 24,  // 每24小时
    ..Default::default()
};
let consolidator = DreamConsolidator::new(config);

// 记录会话
consolidator.record_session(session).await;

// 检查是否需要做梦
if consolidator.should_dream().await {
    let result = consolidator.dream().await;
    // result.memories_consolidated, result.contradictions_resolved
}
```

---

## 自循环闭环

### 概述

KIAS用KIAS开发KIAS，形成飞轮：

```
Detect → Analyze → Plan → Generate → Verify → Deploy → Learn
```

### 模块

| 模块 | 文件 | 功能 |
|------|------|------|
| detector | `detector.rs` | 问题自动检测 |
| analyzer | `analyzer.rs` | 根因自动分析 |
| planner | `planner.rs` | 方案自动生成 |
| codegen | `codegen.rs` | 代码自动生成 |
| verifier | `verifier.rs` | 自动测试验证 |
| deployer | `deployer.rs` | 自动部署 |
| learner | `learner.rs` | 经验积累 |

### 使用

```bash
# 注入问题
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "发现问题：Agent数据持久化缺失"}'

# 启动自动循环
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "启动自动循环：修复Agent数据持久化问题"}'

# 查看状态
curl -X POST http://localhost:8080/api/v1/nl/command \
  -d '{"command": "查看自动循环状态"}'
```

---

## 故障排查

### 常见问题

#### 1. 启动失败: 端口被占用

```bash
# 查看占用端口的进程
lsof -i :8080

# 杀掉进程
kill -9 <PID>
```

#### 2. 数据库连接失败

```bash
# 检查SQLite文件权限
ls -la kias.db

# 重新初始化
rm kias.db
cargo run --bin kias-api-server
```

#### 3. API Key错误

```bash
# 检查配置文件
cat config/kias.toml | grep api_key

# 确保使用真实key，不是占位符
# sk-YOUR_API_KEY_HERE → sk-xxxxxxxxxxxx
```

#### 4. 内存不足

```bash
# 查看内存使用
free -h

# 清理编译缓存
cargo clean
```

#### 5. 磁盘空间不足

```bash
# 查看磁盘
df -h

# 清理target目录
cargo clean

# 清理旧日志
find . -name "*.log" -mtime +7 -delete
```

### 日志查看

```bash
# 实时日志
tail -f logs/kias.log

# 错误日志
grep ERROR logs/kias.log

# 调试模式
RUST_LOG=debug cargo run
```

---

## 安全注意事项

1. **API Key管理:** 不要将真实key提交到Git，使用占位符
2. **网络访问:** 生产环境使用HTTPS
3. **权限控制:** 启用RBAC
4. **沙箱执行:** 不可信代码在沙箱中运行
5. **审计日志:** 启用操作审计

---

## 更新日志

### v0.2.0 (2026-05-16)

- 新增 AgenticRAG 多轮迭代检索
- 新增 七层记忆架构 (L1/L3/L6)
- 新增 自循环闭环 (detect→analyze→plan→generate→verify→deploy→learn)
- 新增 飞轮学习器
- 测试: 1,637 passing

### v0.1.0 (2026-05-15)

- 初始版本
- 21个crate
- 1,464 tests passing

---

## 许可证

MIT License
