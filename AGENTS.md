# AGENTS.md - KIAS

> Kubernetes-like Intelligent Agent Scheduling System
> 专业 AI Agent 集群调度系统

## 1. 项目概述

KIAS 是一个 Rust 实现的 Agent 集群调度系统，借鉴 K8S 架构 + ANOLISA 可观测性 + DeepSeek 缓存优化。

**技术栈**：Rust (axum/tonic) + TypeScript (React) + etcd + SQLite + Redis

**仓库结构**（monorepo）：
```
kias/
├── crates/              # Rust 组件
│   ├── api-server/      # API 服务
│   ├── scheduler/       # 调度器
│   ├── controller/      # 控制器
│   ├── agentsight/      # 可观测（Token 追踪）
│   └── cache-hub/       # KV Cache 优化
├── dashboard/           # React 前端
├── scripts/             # 构建、启动、检查脚本
├── docs/                # 详细文档
└── reference-projects/  # 参考项目（git submodule）
```

## 2. 快速命令

```bash
# 构建
make build                    # 构建所有组件
cargo build -p kias-api-server  # 构建单个组件

# 启动
./scripts/start-control-plane.sh    # 启动控制平面
./scripts/start-node-agent.sh node1 # 启动节点代理

# 质量检查
make lint         # 代码检查
make format       # 格式化
make test         # 测试
make lint-arch    # 分层依赖检查

# 环境配置
source ~/.kias_env  # 启动脚本自动 source
```

## 3. 后端架构（Rust）

```
crates/
├── api-server/
│   ├── src/
│   │   ├── handlers/    # 请求处理
│   │   ├── routes/      # 路由定义
│   │   ├── models/      # 数据模型
│   │   └── middleware/  # 中间件
├── scheduler/
│   ├── src/
│   │   ├── algorithms/  # 调度算法 (RR/LL/RA/CA)
│   │   ├── policies/    # 调度策略 (亲和性/优先级)
│   │   └── optimizer/   # 缓存优化器
├── controller/
│   ├── src/
│   │   ├── state.rs     # 状态管理 (AgentStatus, AgentInfo)
│   │   ├── reconciler.rs # 调和器
│   │   ├── heartbeat.rs # 心跳监控
│   │   ├── recovery.rs  # 故障恢复 (指数退避)
│   │   └── health.rs    # 健康检查循环
├── workflow-engine/
│   ├── src/
│   │   ├── engine.rs    # DAG 执行引擎
│   │   ├── executor.rs  # 节点执行器 (Shell/HTTP/LLM/SubWF)
│   │   ├── graph.rs     # DAG 图结构
│   │   ├── node.rs      # 节点定义 + 执行配置
│   │   └── checkpoint.rs # 检查点持久化
├── team-engine/
│   ├── src/
│   │   ├── engine.rs    # Owner-Worker-Verifier 引擎
│   │   ├── worker.rs    # Worker 实现
│   │   └── verifier.rs  # 质量门禁
├── goal-engine/
│   ├── src/
│   │   ├── loop_runner.rs # 目标驱动循环
│   │   └── evaluator.rs # 目标评估器
├── autonomy-controller/
│   ├── src/
│   │   ├── autonomy.rs  # 三模式 (Suggest/Auto/Full)
│   │   ├── policy.rs    # 工具策略
│   │   └── ladder.rs    # 自主度梯度
├── common/              # 公共类型、错误、配置
├── cache/               # LRU + 前缀缓存
├── monitor/             # 遥测 + 指标收集
├── knowledge/           # 知识图谱
├── skills/              # 技能注册表
├── executor/            # 任务执行框架
├── agent-view/          # Agent 视图 CLI
└── kias-main/           # 主服务编排
```

**核心子系统**：
- API Server：声明式 API，RESTful + gRPC
- Scheduler：资源感知调度（4 算法 + 亲和性 + 缓存优化）
- Controller：Agent 生命周期管理，心跳监控，故障自动恢复（指数退避）
- WorkflowEngine：DAG 工作流引擎，支持 Shell/HTTP/LLM 执行器，条件分支，重试
- TeamEngine：Owner-Worker-Verifier 对抗式质量门禁
- GoalEngine：目标驱动循环，自动迭代直到达标
- AutonomyController：三模式自主度控制（Suggest/AutoEdit/FullAuto）


→ 详见 docs/architecture.md

## 4. 前端架构

**技术栈**：React + TypeScript + TailwindCSS + Recharts

**核心页面**：
- Dashboard：Agent 状态总览
- Token Analytics：Token 消耗分析
- Agent Management：Agent 管理
- Cluster View：集群拓扑

**API 层**：统一使用 hooks/useApi.ts，禁止直接 fetch

→ 详见 docs/design-docs/frontend-architecture.md

## 5. 关键约定（硬性规则）

1. **错误处理**：统一用 `KiasError`，禁止直接 `unwrap()`
   → 详见 docs/design-docs/error-handling.md

2. **分层依赖**：
   - L0 (common) ← L1 (models) ← L2 (services) ← L3 (handlers)
   - 禁止跨层依赖，`make lint-arch` 自动检查
   → 详见 docs/architecture.md#分层规则

3. **异步规范**：所有 I/O 操作必须 async，禁止阻塞调用

4. **配置管理**：禁止硬编码，使用 config crate + 环境变量

5. **日志规范**：使用 tracing，禁止 println! 调试

6. **安全**：API 必须认证，敏感数据禁止日志输出

## 6. 本地开发及验证流程

### 环境配置
```bash
# ~/.kias_env（启动脚本自动 source）
KIAS_ETCD_ENDPOINTS=http://localhost:2379
KIAS_REDIS_URL=redis://localhost:6379
KIAS_LOG_LEVEL=debug
```

### 验证闭环
```bash
# 1. 构建
make build

# 2. 启动
./scripts/start-control-plane.sh

# 3. 验证 API
curl -s http://localhost:8080/health > /tmp/health.json
python3 -c "import json; print(json.load(open('/tmp/health.json')))"

# 4. 部署 Agent
curl -s -X POST http://localhost:8080/api/v1/agents \
  -H 'Content-Type: application/json' \
  -d '{"name":"test-agent","image":"python:3.11"}' > /tmp/agent.json

# 5. 查看状态
curl -s http://localhost:8080/api/v1/agents > /tmp/agents.json
```

**验证不止于编译通过**：改完代码后必须跑通接口验证

→ 详见 docs/design-docs/api-verification.md

## 7. 质量检查

```bash
make lint         # clippy 检查
make format       # rustfmt 格式化
make test         # 单元测试 + 集成测试
make lint-arch    # 分层依赖检查
make bench        # 性能测试
```

**自动化**：CI 中自动运行以上所有检查

## 8. 参考项目约定

```
reference-projects/
├── kubernetes/          # K8S 架构参考
├── anolisa/             # ANOLISA AgentSight 实现
└── deepseek-serving/    # DeepSeek Cache 优化
```

**优先级**：
1. 本项目代码
2. 参考项目源码
3. 外部文档

→ 详见 docs/design-docs/ref-kubernetes.md
→ 详见 docs/design-docs/ref-anolisa.md

## 9. 文档导航

| 文档 | 说明 |
|------|------|
| docs/architecture.md | 分层架构、依赖规则 |
| docs/development.md | 环境搭建、构建运行 |
| docs/api.md | API 文档 |
| docs/design-docs/error-handling.md | 错误处理规范 |
| docs/design-docs/api-verification.md | API 验证规范 |
| docs/design-docs/frontend-architecture.md | 前端架构 |
| docs/design-docs/ref-kubernetes.md | K8S 架构参考 |
| docs/design-docs/ref-anolisa.md | ANOLISA 参考 |
| docs/design-docs/cache-strategy.md | 缓存策略 |

---

**控制在 ~200 行。详细内容通过链接指向 docs/。**