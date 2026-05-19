# AgentGuard 开发需求清单

> 注入时间：2026-05-16
> 状态：进行中

---

## 一、核心卖点（已实现，需打磨）

| # | 卖点 | 状态 | 代码位置 |
|---|------|------|---------|
| 1 | Cache-Aware 调度（KV Cache 命中率优化） | ✅ 已实现 | crates/scheduler/src/algorithms/cache_aware.rs |
| 2 | LangGraph 状态图引擎（条件分支/并行扇出/检查点） | ✅ 已实现 | crates/langgraph-engine/src/graph.rs |
| 3 | TypedState 类型安全通道（编译期 Reducer） | ✅ 已实现 | crates/workflow-engine/src/typed_state.rs |
| 4 | 三层记忆系统（ShortTerm/LongTerm/Entity） | ✅ 已实现 | crates/team-engine/src/memory.rs |
| 5 | Worker-Verifier 对抗式质量门禁 | ✅ 已实现 | crates/team-engine/src/verifier.rs |
| 6 | 自主度梯度控制器（Suggest/AutoEdit/FullAuto） | ✅ 已实现 | crates/autonomy-controller/src/ |
| 7 | 节点级错误处理器（Retry/Skip/Fallback/Abort） | ✅ 已实现 | crates/workflow-engine/src/error_handler.rs |
| 8 | A2A 协议 + MCP 沙箱（5种后端） | ✅ 已实现 | crates/mcp-protocol/src/ |
| 9 | 数据脱敏框架（零信任日志安全） | ✅ 已实现 | crates/common/src/data_mask.rs |
| 10 | 反调度器（PDB 约束下的集群再平衡） | ✅ 已实现 | crates/scheduler/src/descheduler/ |

## 二、待实现功能（参考 Codex / CloudDM / LangGraph）

### P0 — 高优先级

| # | 功能 | 参考来源 | 预计工时 |
|---|------|---------|---------|
| 1 | 自然语言驱动开发（通过微信/CLI/对话控制 Agent） | Hermes Agent | 16h |
| 2 | 手机端远程控制（API + WebSocket） | Codex CLI | 12h |
| 3 | 流式输出（token-level streaming） | LangGraph v1.2 | 10h |
| 4 | SQL 审核规则引擎（54条内置规则 + 自定义） | CloudDM | 12h |
| 5 | 列级数据脱敏（5条内置规则 + 自定义） | CloudDM | 8h |

### P1 — 中优先级

| # | 功能 | 参考来源 | 预计工时 |
|---|------|---------|---------|
| 6 | Agent-as-Tool 模式（Agent 调用 Agent） | AutoGen | 10h |
| 7 | 会话历史压缩（token 预算管理） | OpenAI Agents SDK | 8h |
| 8 | HITL 审批门禁（人工确认节点） | CrewAI | 8h |
| 9 | 工单系统（提交→审核→执行→验证） | CloudDM | 12h |
| 10 | RBAC 权限模型（功能权限 + 资源权限分离） | CloudDM | 10h |

### P2 — 低优先级

| # | 功能 | 参考来源 | 预计工时 |
|---|------|---------|---------|
| 11 | OpenTelemetry GenAI 标准追踪 | AutoGen/ADK | 10h |
| 12 | 可调用条件边（GraphFlow） | AutoGen | 8h |
| 13 | Flow 声明式持久化（@persist） | CrewAI | 10h |
| 14 | A2A 协议标准支持 | Google ADK | 12h |
| 15 | Dashboard Web 控制台 | 自研 | 20h |

## 三、质量要求

- 所有测试通过（当前 1495 测试全绿）
- Clippy 0 warning
- 无 unwrap() 在非测试代码中
- 所有 API 端点响应格式统一（ListResponse）
- CLI 所有命令正常工作
- 文档与代码同步

## 四、文档要求

- README 突出 10 个真实技术卖点
- 每个卖点引用具体代码文件和行号
- 最新模型数据（GPT-5.5, Claude Opus 4.7, Gemini 3.1, DeepSeek-V4, Qwen3, Llama 4）
- 参考 K8s / Hermes / CloudDM 文档风格
- 中英文双版本同步
