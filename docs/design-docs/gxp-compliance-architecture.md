# GxP 合规对齐方案 — AgentGuard 可追溯性架构

> 日期：2026-05-18
> 方法论：四步法 + 钱学森七原则 + 论文/源码支撑
> 状态：Step 3 方案阶段

---

## Step 1: 评估

| 维度 | 评估 |
|------|------|
| 解决什么问题 | Agent 自动化操作缺乏可追溯性，不满足受监管行业要求 |
| 不做会怎样 | AgentGuard 只能用于非受监管场景，市场受限 |
| 核心价值 | **极高** — 决定 AgentGuard 能否进入制药/医疗/金融市场 |
| 用户场景 | GxP 环境下每个变更必须可追溯（谁、何时、改了什么、为什么） |
| 论文支撑 | 7 篇（见下方） |
| 源码支撑 | 10+ 项目（见下方） |
| 结论 | **做** — 这是 AgentGuard 的差异化竞争力 |

---

## Step 2: 审视 — AgentGuard 现有能力

| 现有模块 | GxP 覆盖度 | 缺什么 |
|---------|-----------|--------|
| approval.rs | 有审批状态机 | 无电子签名、无哈希链 |
| audit::MemoryAuditLog | 有审计日志 | 非不可变、无哈希链 |
| quality_pipeline.rs | 有质量管线 | 无 ALCOA+ 合规检查 |
| auth (RBAC) | 有权限控制 | 无职责分离 |
| graph.rs | 有知识图谱 | 无时间旅行查询 |

**差距分析**：AgentGuard 有基础审计能力，但离 GxP 合规差距很大。

---

## 论文支撑（7 篇）

| # | 论文 | 年份 | 引用 | 核心价值 |
|---|------|------|------|---------|
| 1 | Harnessing AI/ML in Drug Discovery: Regulatory Perspective | 2025 | 83 | AI 在 GxP 中的监管框架 |
| 2 | Standard Requirements for GCP-compliant Data Management | 2011 | 80 | GCP 数据管理标准 |
| 3 | Regulatory Perspectives for AI/ML in Pharmaceutical GMP | 2025 | 46 | AI/ML 在 GMP 环境的监管要求 |
| 4 | Validating Intelligent Automation in Pharmacovigilance | 2021 | 42 | 智能自动化验证方法 |
| 5 | Explainable AI in GxP Validation | 2025 | 9 | AI 可解释性 + 可追溯性 |
| 6 | SCDM for CMC Regulatory Submissions | 2021 | 26 | 结构化内容数据管理 |
| 7 | METRIC-framework: ML Training Data Quality | 2024 | 113 | ML 数据质量框架 |

---

## 源码支撑（关键项目）

| 项目 | Stars | 语言 | 核心模式 |
|------|-------|------|---------|
| **paper_trail** | 7001 | Ruby | 模型变更追踪金标准 |
| **tradememory-protocol** | 927 | Python | AI Agent 决策审计 + SHA-256 篡改检测 |
| **cordum** | 480 | Go | Agent 控制面：预执行策略 + 审批门 + 审计 |
| **pgMemento** | 407 | PLpgSQL | PostgreSQL 事务级审计 + schema 版本 |
| **Aegis** | 356 | TS | AI Agent 运行时策略 + 密码学审计 + 人机协同 |
| **DriftDB** | 135 | Rust | 追加写数据库 + 时间旅行 + 数据完整性 |
| **asqav-sdk** | 127 | Python | AI Agent 治理 SDK + 量子安全签名 |

---

## Step 3: 方案 — 五大架构模式

### 模式 1: 不可变审计日志 + 哈希链

```
┌─────────────────────────────────────────┐
│           Audit Entry N                  │
│  timestamp | actor_id | action | target  │
│  reason | before_state | after_state     │
│  hash(prev_entry) | hash(this_entry)     │
│           ↕ hash chain                   │
│           Audit Entry N+1                │
└─────────────────────────────────────────┘
```

**ALCOA+ 对齐**：
- **A**ttributable → actor_id 字段
- **L**egible → 结构化 JSON 格式
- **C**ontemporaneous → timestamp 在操作发生时记录
- **O**riginal → 追加写，不可修改
- **A**ccurate → before/after 状态对比
- **C**omplete → 不可删除条目
- **C**onsistent → 哈希链保证顺序
- **E**nduring → 持久化存储
- **A**vailable → 支持查询和导出

**参考实现**：tradememory-protocol (SHA-256)、DriftDB (append-only)、pgMemento (triggers)

### 模式 2: 电子签名工作流

```
┌──────┐    ┌──────┐    ┌──────┐
│Draft │───▶│Review│───▶│Signed│
└──┬───┘    └──┬───┘    └──┬───┘
   ▼           ▼           ▼
 audit+esig  audit+esig  audit+esig
```

**21 CFR Part 11 要求**：
- 签名唯一绑定到个人
- 签名含义明确（authored/reviewed/approved）
- 签名与电子记录不可分割
- 不可否认性

**参考实现**：cordum (approval gates)、Aegis (HITL)

### 模式 3: 变更控制状态机（ICH Q10）

```
Proposed → Impact Assessment → Approved → Implemented → Verified → Closed
    ↑            │                                           │
    └────────────┴─── Rejected (with documented reason) ────┘
```

每个状态转移记录：who, when, why, what changed, approval chain

**与现有 approval.rs 的关系**：扩展 approval.rs，增加 Impact Assessment 和 Verified 阶段

### 模式 4: AI Agent 专项治理

| 治理点 | 要求 | 参考 |
|--------|------|------|
| 预执行策略检查 | Agent 执行前必须通过策略引擎 | cordum OPA |
| 审批门 | 高风险操作必须人工批准 | Aegis HITL |
| 紧急停止 | 随时可终止 Agent 执行 | Aegis kill switch |
| 确定性重放 | 完整审计轨迹支持决策重放 | tradememory |
| 模型版本追踪 | 决策关联到具体模型版本/配置 | METRIC |

### 模式 5: 时间旅行查询

**需求**：查询任意历史时间点的系统状态

**参考**：DriftDB (append-only + time-travel)、pgMemento (transaction-based)

**实现思路**：审计日志支持 `as_of(timestamp)` 查询，重建任意时刻状态

---

## Step 4: 开发路线

### Phase 1: 增强审计日志（不可变 + 哈希链）
- 扩展 `common/audit.rs` — 追加写 + SHA-256 哈希链
- ALCOA+ 9 字段全覆盖
- 链完整性验证方法

### Phase 2: 电子签名
- 新增 `gxp/signature.rs`
- 签名绑定到 actor + record + timestamp
- 签名含义枚举（Authored/Reviewed/Approved）

### Phase 3: 变更控制增强
- 扩展 `approval.rs` — 增加 Impact Assessment + Verified 阶段
- 多级审批链
- 变更有效性监控

### Phase 4: AI Agent 治理
- 预执行策略引擎
- 审批门（高风险操作人工介入）
- 紧急停止机制
- 模型版本追踪

### Phase 5: 时间旅行 + 可追溯性矩阵
- `as_of(timestamp)` 查询
- 需求→设计→测试追溯矩阵
- 定期审查/再确认

---

## 钱学森七原则检查

- ✅ **整体性**: 审计模块不重复，增强现有 common/audit.rs
- ✅ **综合集成**: 融合 7 篇论文 + 7 个开源项目模式
- ✅ **反馈控制**: 哈希链 = 技术层面的反馈控制
- ✅ **层次分解**: 5 层架构（审计→签名→变更控制→治理→时间旅行）
- ✅ **鲁棒性**: 哈希链检测篡改，追加写防数据丢失
- ✅ **可观测性**: 审计日志本身就是可观测性的基础设施
- ✅ **工程化**: 每个 Phase 独立可测，渐进式交付
