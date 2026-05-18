# OpenAaaS: An Open Agent-as-a-Service Framework

> 论文: 2605.13618 | 作者: Peng Kang, Bixuan Li
> 日期: 2026-05-13 | 分类: cs.AI
> 来源: https://arxiv.org/abs/2605.13618
> PDF: docs/papers/2605.13618.pdf (462KB)

## 摘要

材料基因组计划推动了 SaaS/PaaS/IaaS 集中平台的发展。同时 LLM 和自主 Agent 为科学研究提供了强大推理能力。然而存在关键的"最后一公里"问题：虽然拥有世界级模型和海量材料数据，但缺乏跨机构安全编排这些能力的组织基础设施。OpenAaaS 提出开放的 Agent-as-a-Service 框架解决此问题。

## 对 KIAS 的映射价值

### 1. Agent-as-a-Service 架构
- KIAS 的 Agent 调度本质上是 A2aaS 的一种实现
- OpenAaaS 的跨机构编排可为 KIAS 多集群部署提供参考
- **映射**：api-server 的 RESTful API → AaaS 接口标准

### 2. 分布式 Agent 框架
- OpenAaaS 强调跨机构边界的安全编排
- 与 KIAS 的 etcd-based 状态管理 + gRPC 通信一致
- 可借鉴其权限模型增强 Agent 认证

### 3. 材料科学领域特化
- 特定领域的 Agent 编排模式
- KIAS 可参考其领域适配器模式扩展 skills 系统

## 技术细节

- **核心问题**：跨组织 Agent 协作的安全和编排
- **解决方案**：开放标准的 AaaS 接口 + 权限控制
- **应用场景**：科学研究中的多 Agent 协作

## KIAS 行动项

1. **[高]** 研究 OpenAaaS 的跨机构认证模型，映射到 Agent 身份系统
2. **[中]** 评估 AaaS 接口标准与 KIAS API 的兼容性
3. **[低]** 考虑领域适配器模式扩展 skills 注册表
