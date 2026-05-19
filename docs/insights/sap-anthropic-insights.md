# SAP × Anthropic 合作启示录
> 日期: 2026-05-17
> 来源: 行业洞察

## 核心事件

SAP 与 Anthropic 合作，Claude 作为主力推理引擎嵌入 SAP 全系列产品。

## SAP 的"自主企业"架构

```
┌─────────────────────────────────────┐
│  Joule Work — 自然语言界面          │ ← NL-first（AgentGuard 已有）
├─────────────────────────────────────┤
│  Agent 嵌入核心业务应用              │ ← 行业定制 Agent
├─────────────────────────────────────┤
│  AI 平台 — 构建和治理 Agent         │ ← AgentGuard 定位
└─────────────────────────────────────┘
```

## 对 AgentGuard 的启示

### 1. 架构验证 ✅
SAP 的架构和 AgentGuard 一致：
- **Claude 只是推理引擎** — 哪些业务流程需要推理，哪些审批必须合规，哪些数据绝不能出边界，这些规则还是 SAP 来定
- **AgentGuard 也是** — Agent 是底座，业务规则由上层定义

### 2. 行业定制 Agent 是方向
SAP 和 Anthropic 会一起开发面向具体行业的定制代理：
- 公用事业
- 医疗
- 教育
- 能源

**AgentGuard 行动**：扩展 BuiltinAgents，添加行业专属 Agent

### 3. 高价值场景
- **财务月结** — Autonomous Close Assistant，自动处理日记账、对账、查错
- **HR** — 人力资源管理
- **供应链** — 供应链优化

**AgentGuard 行动**：优先实现 FinanceAgent、HRAgent、SupplyChainAgent

### 4. NL 接口验证
Joule Work — 用自然语言代替屏幕导航
**AgentGuard 验证**：我们的 NL-first 设计是对的

### 5. 迁移窗口
17000 家企业从 ECC 迁移，是 AI 升级的入口
**AgentGuard 机会**：企业级 Agent 调度平台

## AgentGuard 下一步行动

1. **扩展行业 Agent** — FinanceAgent、HRAgent、SupplyChainAgent
2. **强化 NL 接口** — 确保自然语言交互流畅
3. **企业级功能** — RBAC、审计、合规
4. **A2A 协议** — 支持跨企业 Agent 互操作
