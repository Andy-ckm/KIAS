# A2A 与 Agent 编排调研报告
> 生成时间: 2026-05-17
> 基于 GitHub API 实时搜索

## 一、A2A 协议（Agent-to-Agent）

| 项目 | Stars | 关键特性 |
|------|-------|---------|
| [a2aproject/A2A](https://github.com/a2aproject/A2A) | 23.8k | Google A2A 协议，Agent 互操作标准 |
| [python-a2a](https://github.com/themanojdesai/python-a2a) | 990 | Python A2A 实现 |
| [a2a-x402](https://github.com/google-agentic-commerce/a2a-x402) | 510 | A2A + 加密货币支付扩展 |

**KIAS 启示**: A2A 是 Google 推出的 Agent 互操作标准，KIAS 已有 `a2a_router.rs`，应对接 A2A 协议标准。

## 二、Agent 编排框架

| 项目 | Stars | 关键特性 |
|------|-------|---------|
| [ruflo](https://github.com/ruvnet/ruflo) | 52k | Claude Agent 编排平台 |
| [agents](https://github.com/wshobson/agents) | 35.5k | Claude Code 多 Agent 编排 |
| [openai/swarm](https://github.com/openai/swarm) | 21.5k | 轻量级多 Agent 编排 |
| [edict](https://github.com/cft0808/edict) | 15.8k | 三省六部制，9 个专业 Agent |

**KIAS 启示**: 
- **edict** 的"三省六部制"模式值得借鉴——按职能划分专业 Agent
- **swarm** 的轻量级编排适合 KIAS 的调度场景

## 三、Agent 调度

| 项目 | Stars | 关键特性 |
|------|-------|---------|
| [agentcontrolplane](https://github.com/humanlayer/agentcontrolplane) | 405 | 分布式 Agent 调度器 |
| [agentflow](https://github.com/Siddhant-K-code/agentflow) | 5 | K8S for AI agents |

**KIAS 启示**: KIAS 的 `scheduler` + `AgentShell` 已经是 Agent 调度的实现，可对标 agentcontrolplane。

## 四、Agent 沙箱隔离

| 项目 | Stars | 关键特性 |
|------|-------|---------|
| [arrakis](https://github.com/abshkbh/arrakis) | 810 | 自托管 Agent 沙箱 |
| [greywall](https://github.com/GreyhavenHQ/greywall) | 183 | 无容器，默认拒绝沙箱 |
| [SecGPT](https://github.com/llm-platform-security/SecGPT) | 113 | LLM Agent 执行隔离架构 |
| [hazmat](https://github.com/dredozubov/hazmat) | 111 | macOS Agent 容器化 |

**KIAS 启示**: 
- **greywall** 的"无容器，默认拒绝"模式适合轻量级场景
- **arrakis** 的自托管沙箱适合企业部署
- KIAS 的 `SandboxType` 枚举已覆盖这些场景

## 五、KIAS 已有 vs 需要扩展

### 已有（不需要重写）
- ✅ `a2a_router.rs` — A2A 路由
- ✅ `skill_matcher.rs` — 能力匹配 + 专业 Agent 模板
- ✅ `scheduler/agent_shell.rs` — Agent 调度
- ✅ `mcp-protocol/sandbox.rs` — 沙箱隔离
- ✅ `team-engine/delegation.rs` — 任务委派

### 需要扩展（增量改进）
1. **对接 A2A 协议标准** — 让 KIAS Agent 能与外部 Agent 互操作
2. **Agent 发现机制** — Agent 广播自己的能力，类似 mDNS
3. **上下文共享协议** — Agent 间安全共享上下文
4. **Agent 生命周期管理** — 创建→运行→暂停→销毁

## 六、行动建议

1. **短期**（当前迭代）:
   - 扩展 `skill_matcher.rs` 添加专业 Agent 模板 ✅ 已完成
   - 添加 Agent 发现机制（CapabilityRegistry）

2. **中期**（下个迭代）:
   - 对接 A2A 协议标准
   - 实现 Agent 上下文共享

3. **长期**（未来版本）:
   - Agent 自主调度（KIAS 自己决定用哪个 Agent）
   - Agent 学习（从历史任务中优化匹配）
