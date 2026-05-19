# Hermes Agent 架构深度分析

> 来源：知乎文章 2034051084671567728
> 日期：2026-05-18
> 用途：**AgentGuard 的设计参考蓝图**

## 一、核心架构

Hermes Agent 的本质不是聊天机器人，而是一个**有学习环路的个人 AI 系统**。

```
SOUL.md（身份层）
    ↓
Runtime Loop（运行时循环）
    ↓
经验捕获 → 技能自动生成 → Curator 健康检查
    ↓
GEPA 离线优化（进化搜索）
```

## 二、四层架构拆解

### 2.1 SOUL.md — 身份层

- **作用**：定义 Agent 的性格、知识范围、行为约束
- **格式**：Markdown + YAML frontmatter
- **运行时加载**：每个消息注入到 system prompt
- **热更新**：修改文件即可，无需重启

**AgentGuard 差距**：AgentGuard 的 agent 定义是 YAML（team-engine），没有独立的"灵魂"文件。
**借鉴点**：给 AgentGuard Agent 加 `SOUL.md`，作为身份层的声明式定义。

### 2.2 Runtime Loop — 运行时循环

```
用户消息 → LLM 决策 → 工具执行 → 结果返回 → 经验捕获
                ↑                              ↓
                └──── 记忆注入 ←── 持久化存储 ←──┘
```

**关键设计**：
- 每个 tool call 都有 timeout + retry
- 上下文窗口管理（压缩旧对话）
- 工具结果自动注入回 LLM

**AgentGuard 差距**：AgentGuard 的 controller 是单次执行，没有循环反馈。
**借鉴点**：controller 需要支持"执行→观察→调整→再执行"的循环。

### 2.3 三层记忆体系

| 层级 | Hermes | AgentGuard 现状 | 差距 |
|------|--------|-----------|------|
| 短期 | 对话上下文 | 无（每次新会话） | **缺对话状态** |
| 中期 | `MEMORY.md`（用户画像+环境） | 无 | **缺用户画像** |
| 长期 | Skills（可复用过程） | skills crate 有基础 | **缺自动生成** |

**关键洞察**：
- 短期记忆 = 对话历史（token 管理）
- 中期记忆 = 用户偏好、环境事实、工具特性（持久化注入）
- 长期记忆 = 技能库（可复用的多步骤流程）

**AgentGuard 借鉴**：
1. 给每个 Agent 加 `MEMORY.md`，存储用户偏好
2. 技能系统支持自动生成（不只是手动定义）
3. 记忆分层注入到 prompt

### 2.4 技能自进化

**Hermes 流程**：
```
任务完成（5+ tool calls）→ 自动提议创建 Skill → 用户确认 → 写入 SKILL.md
        ↓
Skill 执行中发现问题 → 自动 patch
        ↓
Curator 定期扫描 → 清理过期/错误 Skill
        ↓
GEPA 离线优化 → 进化搜索最优版本 → PR 提交
```

**AgentGuard 差距**：
- skills 是静态定义的，没有自动生成
- 没有 Curator 健康检查
- 没有 GEPA 式优化

**借鉴点**：
1. 任务完成后自动生成 Skill 定义
2. 定期扫描 Skill 健康度
3. 执行痕迹驱动的优化（不是问 AI 做得好不好）

## 三、GEPA 离线优化

**核心思路**：别问 AI "你做得好不好"，直接读执行痕迹，理解失败原因。

**流水线**：
1. 从仓库读取当前 Skill
2. 生成评测数据集（合成 + 真实 + 黄金集）
3. 运行优化器：读痕迹 → 找失败 → 生成候选改进
4. LLM 评分（带细则，非二元判断）
5. 约束门：测试 100%、文件 <15KB、缓存兼容、语义不漂移
6. 胜出版本以 PR 提交

**AgentGuard 借鉴**：
- 用 AgentGuard 自己的 workflow-engine 跑 GEPA 流程
- 执行痕迹存 SQLite audit log
- 约束门可以用 AgentGuard 的质量门禁（cargo test + clippy）

## 四、多 Agent 架构

**Hermes profiles**：
- 每个 Agent 完全隔离（配置、记忆、技能）
- 可以创建：程序员、研究员、设计师
- 定时任务 + 自然语言描述

**AgentGuard 差距**：
- team-engine 有多 Agent，但没有 profile 隔离
- 没有定时任务的自然语言描述
- 没有 Agent 间的消息传递

## 五、可直接映射到 AgentGuard 的设计

| Hermes 设计 | AgentGuard 映射 | 实现难度 |
|-------------|-----------|----------|
| SOUL.md | agent 定义 YAML + 热加载 | 低 |
| MEMORY.md | agent 内存（SQLite + prompt 注入） | 中 |
| Skill 自动生成 | 任务完成后调 LLM 生成 SKILL.md | 中 |
| Curator 健康检查 | 定期 cron job 扫描 Skill 健康度 | 低 |
| GEPA 优化 | workflow-engine 编排 + SQLite 痕迹 | 高 |
| Runtime Loop | controller 支持循环执行 | 高 |
| Profile 隔离 | team-engine agent namespace | 中 |

## 六、开发任务提取

1. [ ] Agent SOUL.md 身份层（优先级：高）
2. [ ] Agent MEMORY.md 中期记忆（优先级：高）
3. [ ] Skill 自动生成流水线（优先级：中）
4. [ ] Curator 健康检查 cron（优先级：中）
5. [ ] GEPA 优化 pipeline（优先级：低，后期）
6. [ ] Runtime Loop 循环执行（优先级：高）
7. [ ] Profile 隔离（优先级：中）
