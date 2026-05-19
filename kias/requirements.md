# AgentGuard 核心开发需求 — 用 AgentGuard 开发 AgentGuard

## 需求来源
用户明确要求：用 AgentGuard 系统来管理自身的开发，形成正向循环。
参考 Codex (OpenAI) 和 CloudDM 的设计思维。

## P0 — 核心执行引擎

### 1. LLM 集成引擎
- 支持多 LLM 提供商: OpenAI, Anthropic, 本地模型
- 流式输出支持 (SSE)
- Token 计数和成本追踪
- 失败重试和降级策略
- 参考: LiteLLM 的 provider 抽象层

### 2. Agent 执行循环
- Codex 风格: User → LLM → Tool → Observation → Loop
- 多轮对话管理
- 工具调用和结果注入
- 超时和取消支持
- 参考: Codex CLI 的 agentic loop

### 3. 工具执行框架
- 内置工具: file_read, file_write, shell, search
- 工具注册和发现机制
- 沙箱执行 (process/docker/namespace)
- 权限控制 (read/write/deny)
- 参考: Codex 的 3 工具模型 + 沙箱

### 4. 工作流执行引擎
- DAG 执行: 节点依赖解析
- 并行执行无依赖节点
- 条件分支和循环
- 失败重试和补偿 (Saga 模式)
- 状态持久化和恢复

## P1 — 增强功能

### 5. 上下文管理
- 项目级上下文文件 (kias.md)
- Agent 记忆持久化
- 对话历史管理
- 知识库检索增强 (RAG)

### 6. 移动端/IM 深度集成
- 命令别名系统
- 卡片消息格式
- 文件上传/下载
- 实时通知推送
- 多会话管理

### 7. 自然语言驱动开发
- 代码生成 Agent
- 测试生成 Agent
- 文档生成 Agent
- 代码审查 Agent
- 自动 PR 创建

## P2 — 企业级特性

### 8. 多租户隔离
- 命名空间隔离
- 资源配额管理
- 访问控制 (RBAC)

### 9. 可观测性
- 全链路 Trace
- 指标收集 (Prometheus)
- 日志聚合
- 告警规则

### 10. 自循环开发模式
- 用 AgentGuard 管理 AgentGuard 开发
- 自动化测试和部署
- 代码审查自动化
- 发布管理

## 开发工作流
```
需求 → AgentGuard Agent 分析 → 制定计划 → 实现代码 → 测试 → 审查 → 部署
                    ↑                                              |
                    └──────────── 反馈循环 ←───────────────────────┘
```
