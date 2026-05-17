# KIAS 核心目标

> 更新日期：2026-05-18

## 一句话定位

**KIAS = Long-running Agents 的基石**

## 为什么是"基石"

算力网时代，维持庞大算力网运转，必须依赖智能体。基于 ReAct 或 MCP 机制的长周期智能体（Long-running Agents），是企业级验证、授权、自动化运维的核心执行单元。

KIAS 不是聊天机器人，不是一次性脚本执行器。KIAS 是让 Agent 能**持续运行、自主决策、自我修复**的基础设施。

## 核心能力矩阵

| 能力 | 模块 | Long-running 支撑 |
|------|------|-------------------|
| 持续运行 | autonomy-controller | Agent 不停机，循环执行 |
| 自主决策 | runtime-loop (OODA) | 观察→调整→再执行 |
| 自我修复 | controller/recovery | 故障检测→自动恢复 |
| 目标驱动 | goal-engine | 持续逼近目标直到达成 |
| 多 Agent 协作 | team-engine | 分工、委托、看板 |
| 状态持久化 | data-store + memory | 跨重启保持状态 |
| 安全执行 | auth + sandbox + audit | 权限隔离 + 全链路审计 |
| 外部集成 | mcp-protocol | 标准化工具调用 |

## 三个战略阶段

### 阶段一：Dogfooding（当前）
用 KIAS 开发 KIAS。验证 Long-running 能力。
- ✅ 自主开发循环（cron jobs）
- ✅ 任务队列系统
- ✅ Runtime Loop (OODA)
- ✅ SOUL.md 身份层
- ✅ MidTermMemory 三层记忆

### 阶段二：企业级验证
在真实业务场景中跑通 Long-running Agents。
- [ ] Finance/Logistics/HR 工作流模板
- [ ] 多租户隔离
- [ ] 数据合规框架
- [ ] AIOps 自动化运维

### 阶段三：算力网中间件
成为算力网的 Agent 编排层。
- [ ] 异构算力调度
- [ ] 分布式 Agent 长周期运行
- [ ] 算力网到私有云无缝链接
- [ ] 出海标准输出

## 竞争壁垒

1. **开源** — 私有化部署，不锁定
2. **Rust** — 性能 + 安全，适合 Long-running
3. **多模型** — 不依赖单一 LLM
4. **自循环** — 用 KIAS 开发 KIAS，dogfooding 验证
5. **企业级** — RBAC + 审计 + 沙箱 + 合规

## 关键指标

- Agent 连续运行时长（目标：>24h 无人值守）
- 故障自愈率（目标：>95%）
- 目标达成率（目标：>80%）
- 跨重启状态保持（目标：100%）
