# KIAS — AI Agent 合规免疫系统

> 让受监管行业的企业，敢于在核心生产环境里使用 AI Agent。

## 一句话

**你的 AI Agent 能通过 FDA 审计吗？KIAS 让答案变成"能"。**

## 问题

制药、医疗器械、金融等受监管行业的企业，面临一个共同困境：

- **想用 AI**：AI Agent 能极大提升文档审查、变更管理、质量检查的效率
- **不敢用**：现有 AI Agent 框架（LangChain/CrewAI/AutoGen）没有合规审计链、没有电子签名、没有审批流——直接用在 GxP 环境里，FDA 审计过不了

结果：**AI 在受监管行业的渗透率远低于其他行业。**

## 解决方案

KIAS 是一个 **AI Agent 合规免疫系统**，它不是替代 LangChain/CrewAI，而是在任何 Agent 框架之上，加一层合规壳：

| 合规要求 | KIAS 模块 | 对应标准 |
|---------|----------|---------|
| 不可篡改审计链 | gxp_audit（SHA-256 哈希链） | 21 CFR Part 11 / EU Annex 11 |
| 电子签名 + 2FA | gxp_auth（TOTP） | 21 CFR Part 11 §11.50 |
| 变更审批流 | approval（9 状态机） | ICH Q10 / ALCOA+ |
| 副作用预演 | side_effect_gate（Dry-Run） | GAMP 5 |
| 权限隔离 | RBAC + 会话管理 | 21 CFR Part 11 §11.10 |

## 核心场景：CCR（系统变更控制）

在制药企业的 CCR 流程中，一个系统变更需要：

1. **变更请求** → KIAS approval 模块自动创建审批流
2. **影响评估** → KIAS side_effect_gate 自动 dry-run 预演变更影响
3. **审批决策** → KIAS gxp_auth 要求审批人电子签名（TOTP 2FA）
4. **执行变更** → KIAS auto-loop 执行，每步记录审计日志
5. **验证确认** → KIAS verifier 自动验证变更结果
6. **审计报告** → KIAS gxp_audit 生成 SHA-256 哈希链审计报告

**全程：0 人工审计日志整理，100% 自动生成合规报告。**

## 差异化

| 维度 | LangChain/CrewAI | KIAS |
|------|-----------------|------|
| 审计链 | ❌ 无 | ✅ SHA-256 不可篡改 |
| 电子签名 | ❌ 无 | ✅ TOTP 2FA |
| 审批流 | ❌ 无 | ✅ 9 状态机 |
| 副作用预演 | ❌ 无 | ✅ Dry-Run |
| FDA 合规 | ❌ 不能 | ✅ 21 CFR Part 11 |
| 开源 | 部分 | ✅ MIT 全部 |

## 技术指标

- 26 个 Rust crate，110K+ LOC
- 2656 个自动化测试，0 失败
- MCP 协议支持（可接入任何 Agent 框架）
- 本地部署 / 私有化（数据不出企业网络）

## 下一步

找 1 家制药企业，免费跑通 CCR 场景的第一个真实闭环，换取案例背书。

## 联系方式

GitHub: https://github.com/Andy-ckm/KIAS
