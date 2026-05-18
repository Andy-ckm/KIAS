# 论文架构→KIAS模块映射分析

> 生成日期: 2026-05-18
> 论文来源: KIAS autonomous loop 自动下载的 arXiv 论文
> 分析目标: 提取架构模式，映射到 KIAS 现有模块，识别差距与建议

---

## 目录

1. [SDOF: State-Driven Orchestration Framework](#1-sdof)
2. [SkillSmith: Boundary-First Skill Compilation](#2-skillsmith)
3. [NIMO Controller: MCP-based Orchestrator](#3-nimo-controller)
4. [DTF: Distributed Trust Framework](#4-dtf)
5. [综合映射矩阵](#5-综合映射矩阵)
6. [优先级建议](#6-优先级建议)

---

## 1. SDOF

**论文**: SDOF: Taming the Alignment Tax in Multi-Agent Orchestration with State-Constrained Dispatch
**作者**: Zhantao Wang (Digital China)
**日期**: 2026-05-15 | arXiv: 2605.15204
**页数**: 7 pages

### 核心贡献

将多Agent编排建模为**约束状态机**，在现有编排框架(LangChain/LangGraph/CrewAI)之上增加两层防御：
- **Intent-Stage Binding (Λ)**: 意图只能在合法的业务阶段执行
- **Precondition Validation (Πpre)**: 技能执行前验证前置条件

### 架构图

```
用户消息
  │
  ▼
┌──────────────────────┐
│   IntentRouterAgent  │ ← GRPO-trained 7B model (意图识别)
└──────────┬───────────┘
           │ intent
           ▼
┌──────────────────────┐
│  GoalStage FSM       │ ← 有限状态机 (init→src→int→off→onb→close)
│  (阶段合法性检查)      │    Λ(i) ⊆ S: 意图-阶段绑定
└──────────┬───────────┘
           │ stage-legal intent
           ▼
┌──────────────────────┐
│  SkillRegistry       │ ← 三级分类: L0原子/L1组合/L2策略
│  (前置条件验证)       │    Πpre: precondition checks
│  (渐进式披露)         │    L0仅暴露元数据, L1/L2按需加载
└──────────┬───────────┘
           │ precondition-satisfied
           ▼
┌──────────────────────┐
│ StateAwareDispatcher │ ← Algorithm 1: 阶段过滤→技能选择→约束执行
│ (调度器)              │    生成可重放的 ProcessEvent 审计轨迹
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│ GoalManager          │ ← PostgreSQL-backed 共享程序记忆
│ (企业治理记忆基底)     │    按 goal_id 范围化, 仅通过合法状态转换变更
└──────────────────────┘
```

### 关键模式

| 模式 | 描述 | 形式化 |
|------|------|--------|
| Intent-Stage Binding | 意图绑定到合法业务阶段 | Λ: I → 2^S |
| FSM约束执行 | 业务流程建模为有限状态机 | (S, s₀, T, I, Λ) |
| 三级技能分类 | L0原子/L1组合/L2策略, 渐进披露 | sk = (id, ℓ, Σ, Πpre, Πpost, ρ) |
| 前置条件验证 | 执行前验证上下文满足所有前置条件 | ∀π ∈ Πpre(sk), π(C) = ⊤ |
| 可重放审计 | ProcessEvent 事件链支持回放审计 | 完整执行轨迹持久化 |

### KIAS映射

| 论文模式 | KIAS模块 | 差距 | 建议 |
|----------|----------|------|------|
| GoalStage FSM | `workflow-engine` (node/edge/state) | KIAS有DAG图但缺少**业务阶段约束**；无Λ(intent-stage binding)概念 | 在workflow-engine中增加StageConstraint层，为每个node绑定合法阶段集 |
| IntentRouter | `model-router` (router.rs) | KIAS model-router做模型选择而非意图路由；缺少GRPO训练的专用路由器 | 扩展model-router支持意图分类模式，或新建intent-router子模块 |
| SkillRegistry 三级分类 | `skills` (registry/skill) | KIAS SkillRegistry有registry但无风险分级(L0/L1/L2)和渐进披露 | 在Skill结构体中增加risk_level字段和disclosure_policy |
| Πpre 前置条件 | `workflow-engine` (approval.rs) | KIAS有ApprovalPolicy但它是审批策略，非前置条件验证 | 新增PreconditionValidator trait，每个skill定义前置条件集 |
| StateAwareDispatcher | `workflow-engine` (dispatcher.rs) | KIAS Dispatcher做Agent选择，不做阶段过滤 | 在dispatcher中增加stage_filter: 在选择技能前先过滤掉当前阶段不允许的技能 |
| GoalManager 共享记忆 | `agent-runtime` (session_memory) | KIAS记忆是per-session的，缺少按goal_id范围化的共享程序记忆 | 扩展SessionMemory支持goal-scoped共享状态 |
| ProcessEvent审计 | `data-governance` (audit_middleware) | KIAS有审计中间件但缺少可重放的流程事件轨迹 | 增加ProcessEvent类型，记录完整的状态机转换链 |

---

## 2. SkillSmith

**论文**: SkillSmith: Compiling Agent Skills into Boundary-Guided Runtime Interfaces
**作者**: Duling Xu, Zheng Chen et al. (AetherHeart Tech / Renmin University / UCSD)
**日期**: 2026-05-15 | arXiv: 2605.15215

### 核心贡献

**编译器-运行时**框架：将技能包离线编译为最小可执行接口（boundary contracts），消除两种冗余：
- 无关上下文注入（51% token浪费）
- 重复技能推理（45.5%推理轨迹相似度）

### 架构图

```
═══ 编译时 (一次性) ═══

技能包 (SKILL.md + 脚本/模板/引用)
  │
  ▼
┌──────────────────────────┐
│  Source-Shape 分类器      │ ← 识别技能类型：描述型/工作流型/检查清单型
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│  编译器-本地降低          │
│  ├─ 工作流编译 → 步骤图   │
│  ├─ 调度器提取 → 操作符   │
│  ├─ 策略提示 → 约束       │
│  └─ 引用索引 → 回退路径   │
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│  Boundary Contract       │ ← 编译产物：
│  (边界合约)               │    - 操作符列表 (可执行)
│                          │    - 输入需求 (typed)
│                          │    - 策略约束 (policy)
│                          │    - 验证证据
│                          │    - 回退路径
└──────────────────────────┘

═══ 运行时 ═══

Agent调用
  │
  ▼
┌──────────────────────────┐
│  渐进式披露               │ ← 1. 先显示compact handle + boundary summary
│  Progressive Disclosure  │    2. 选择后才披露详细操作符/策略/回退
└──────────┬───────────────┘
           │
           ▼
┌──────────────────────────┐
│  调度器 (Dispatcher)     │ ← 选择相关操作符
│  → 策略检查 (Guards)     │ ← 验证策略约束
│  → 执行 / 指导 / 回退    │ ← typed操作直接执行，否则回退到LLM
└──────────────────────────┘
```

### 关键模式

| 模式 | 描述 | 效果 |
|------|------|------|
| Boundary-First Compilation | 编译边界合约而非统一IR | -57% token, -43% 推理调用 |
| 渐进式披露 | 先compact后detail | 减少上下文窗口污染 |
| 操作符选择性执行 | typed操作直接执行，非typed回退LLM | 2.02× 加速 |
| 编译产物跨模型复用 | 强模型编译→弱模型运行时使用 | 小模型准确率提升 |
| 回退胶囊 | 无法编译的部分保留原始源材料 | 不丢失能力 |

### KIAS映射

| 论文模式 | KIAS模块 | 差距 | 建议 |
|----------|----------|------|------|
| 技能编译 | `skills` (pipeline/distillation) | KIAS有pipeline和distillation但都是运行时概念，无离线编译 | 新增`skill-compiler`子模块：输入SKILL.md → 输出boundary contract JSON |
| Boundary Contract | `skills` (skill.rs SkillConfig) | SkillConfig是运行时配置，不是编译产物 | 定义CompiledSkill结构体：{operators, inputs, policies, fallback} |
| 渐进式披露 | `skills` (registry) | KIAS registry暴露完整skill元数据，无分层披露 | 在SkillRegistry查询中增加disclosure_level参数(L0摘要/L1完整) |
| Source-Shape分类 | `skills` (builtin.rs) | KIAS内置技能是硬编码的，无自动分类 | 增加skill classifier：自动识别skill类型(描述/工作流/检查清单) |
| 操作符调度器 | `workflow-engine` (executor.rs) | KIAS executor执行node，但不区分typed操作vs LLM推理 | 在executor中增加operator_type判断：Deterministic直接执行，Generative调LLM |
| 缓存编译产物 | `cache` (strategy.rs) | KIAS有缓存策略但未用于技能编译产物 | 增加skill artifact缓存层，按skill hash缓存编译结果 |

---

## 3. NIMO Controller

**论文**: NIMO Controller: A Self-Driving Laboratory Orchestrator Based on Model Context Protocol
**作者**: Naruki Yoshikawa, Ryo Tamura (NIMS / University of Tokyo)
**日期**: 2026-05-15 | arXiv: 2605.15227

### 核心贡献

基于MCP的自驱动实验室(SDL)编排器：所有实验室功能通过MCP server暴露，自动生成可视化编程界面，统一人机交互与AI Agent接口。

### 架构图

```
┌─────────────────────────────────────────┐
│          NIMO Controller (MCP Host)     │
│                                         │
│  ┌─────────────┐  ┌──────────────────┐ │
│  │  Blockly    │  │  自然语言界面     │ │
│  │  可视化编程  │  │  (LLM Agent)     │ │
│  └──────┬──────┘  └───────┬──────────┘ │
│         │                 │             │
│         ▼                 ▼             │
│  ┌──────────────────────────────────┐  │
│  │      MCP Client Layer            │  │
│  │  ├─ MCP Client (NIMO)            │  │
│  │  └─ MCP Client (Component) ×N    │  │
│  └──────────────┬───────────────────┘  │
└─────────────────┼──────────────────────┘
                  │ MCP协议 (JSON-RPC)
    ┌─────────────┼─────────────┐
    ▼             ▼             ▼
┌────────┐  ┌────────┐  ┌────────────┐
│NIMO MCP│  │Device  │  │Database    │
│Server  │  │MCP Srv │  │MCP Server  │
│(决策)   │  │(硬件)   │  │(数据)      │
└────────┘  └────────┘  └────────────┘
```

### 关键模式

| 模式 | 描述 | 特点 |
|------|------|------|
| MCP-as-Abstraction | 所有功能通过MCP server暴露 | 松耦合, 插拔式扩展 |
| Tool Discovery自动UI | MCP tool定义自动生成Blockly块 | 零代码工作流设计 |
| 统一人机接口 | 同一MCP后端服务人类和AI | 代码只写一次 |
| 安全审批门控 | 工具调用需用户批准(auto-approve可选) | human-on-the-loop |
| 远程MCP透明化 | 远程实验 = 远程MCP server | 无需修改客户端 |

### KIAS映射

| 论文模式 | KIAS模块 | 差距 | 建议 |
|----------|----------|------|------|
| MCP-as-Abstraction | `mcp-protocol` (client/server) | KIAS已有完整MCP实现！包括server、client、transport、auth等 | ✅ 已覆盖。可参考NIMO的tool discovery模式增强现有实现 |
| Tool Discovery→自动UI | `skills` (web_recorder) / `agent-view` | KIAS有web recorder和agent-view dashboard，但不从MCP tool定义自动生成UI | 新增MCP tool → UI block自动生成器 |
| 安全审批门控 | `autonomy-controller` + `workflow-engine` (approval) | KIAS有三模式自主度和审批策略 | ✅ 已覆盖。可参考NIMO的auto-approve toggle设计 |
| 统一人机接口 | `api-server` + `im-integration` | KIAS有API server和IM集成，但人机接口未统一 | 统一api-server和IM bot的tool调用路径 |
| 远程MCP透明化 | `mcp-protocol` (transport) | KIAS MCP支持stdio/HTTP+SSE/内存传输 | ✅ 已覆盖 |

---

## 4. DTF

**论文**: Verifiable Agentic Infrastructure: Proof-Derived Authorization for Sovereign AI Systems
**作者**: Jun He, Deying Yu (OpenKedge.io)
**日期**: 2026-05-15 | arXiv: 2605.15228

### 核心贡献

**分布式信任框架(DTF)**：从身份中心授权转向**证明派生授权**。Agent不因持有凭证而有权执行，而是因有经共识批准的证明而获得临时执行身份。

### 架构图

```
Agent提出意图 It
  │
  ▼
┌──────────────────────────────────┐
│  证明构建 f: I×C×P → J          │ ← Justification Proof
│  (意图 + 上下文 + 策略 → 证明)    │    绑定: 意图/上下文/策略基/风险/执行边界
└──────────────┬───────────────────┘
               │ JPt
               ▼
┌──────────────────────────────────┐
│  共识验证 vi: J → A              │ ← n个独立评估者各自签署证明
│  共识规则 q: A^n×G → D           │    D = {approve, reject, escalate}
│  (Evaluation Swarm)              │
└──────────────┬───────────────────┘
               │ Dt = approve
               ▼
┌──────────────────────────────────┐
│  执行身份派生 h: J×A×G → E       │ ← 临时Execution Identity
│  边界检查 Scope(EI) ⪯ B          │    从证明派生，非从身份
└──────────────┬───────────────────┘
               │ bounded authority
               ▼
┌──────────────────────────────────┐
│  执行 Execute(EIt)               │
└──────────────┬───────────────────┘
               │
               ▼
┌──────────────────────────────────┐
│  证据链 EC = (I,C,P,JP,A,G,D,   │ ← Append-only, 不可变
│              EI,X,O)             │    完整授权生命周期
└──────────────────────────────────┘
```

### 四条系统不变量

1. **Proof-bound Execution**: 无证明 → 不执行
2. **Consensus-gated Authority**: 无共识批准 → 无执行身份
3. **Non-escalation**: 执行范围 ⊆ 证明边界
4. **Evidence Completeness**: 每个意图 → 恰好一条完整证据链

### KIAS映射

| 论文模式 | KIAS模块 | 差距 | 建议 |
|----------|----------|------|------|
| Justification Proof | `autonomy-controller` (policy) | KIAS有ToolPolicy但无结构化证明对象 | 新增JustificationProof结构体：{intent, context, policy_basis, risk, boundary} |
| 共识验证(Evaluation Swarm) | `controller` (reconciler) | KIAS reconciler做状态调和，不做多评估者共识 | 新增ConsensusValidator trait，支持N-of-M评估者模式 |
| Execution Identity(临时) | `mcp-protocol` (credentials) | KIAS有credential管理但基于长期凭证 | 增加EphemeralIdentity：从approved proof派生，有TTL和scope约束 |
| Evidence Chain(不可变) | `data-governance` (audit_middleware) | KIAS有审计中间件但非append-only证据链 | 升级审计系统为不可变证据链，记录完整证明→批准→执行→结果生命周期 |
| 非升级约束 | `autonomy-controller` (ladder) | KIAS有自主度分级但无scope边界检查 | 增加ScopeBoundary：执行身份的scope必须⊆证明中声明的边界 |
| 意图治理(非直接执行) | `workflow-engine` (approval) | KIAS有审批但Agent仍可直接执行 | 引入intent-governed mutation模式：Agent提交意图而非直接调用工具 |

---

## 5. 综合映射矩阵

### 论文模式→KIAS模块交叉表

| 论文模式 | skills | workflow-engine | autonomy-controller | mcp-protocol | data-governance | controller | agent-runtime |
|----------|--------|-----------------|---------------------|--------------|-----------------|------------|---------------|
| FSM阶段约束 (SDOF) | | ★★★ | | | | ★ | |
| 前置条件验证 (SDOF) | ★★ | ★★★ | ★ | | | | |
| 技能编译 (SkillSmith) | ★★★ | | | | | | |
| 渐进式披露 (SkillSmith) | ★★★ | | | | | | |
| MCP抽象层 (NIMO) | | | | ✅已有 | | | |
| 自动UI生成 (NIMO) | ★ | | | ★★ | | | |
| 证明派生授权 (DTF) | | | ★★★ | | ★★ | ★ | |
| 共识验证 (DTF) | | | ★★ | | | ★★★ | |
| 证据链 (DTF) | | | | | ★★★ | | |
| 临时执行身份 (DTF) | | | ★★ | ★★ | | | |

> ★★★ = 强相关/主要映射目标 | ★★ = 中等相关 | ★ = 弱相关/辅助 | ✅ = 已实现

### KIAS现有能力评估

| KIAS模块 | 行数 | 核心能力 | 论文覆盖度 |
|----------|------|----------|------------|
| workflow-engine | ~7K | DAG编排、审批、检查点、看板 | SDOF 40%, SkillSmith 20% |
| skills | ~6K | 注册、组合、流水线、蒸馏、Web录制 | SkillSmith 30%, SDOF 20% |
| mcp-protocol | ~10K | MCP客户端/服务端、认证、弹性、沙箱 | NIMO 85% ✅ |
| autonomy-controller | ~1K | 三级自主度、工具策略、审计 | DTF 25% |
| data-governance | ~1K | 数据源、策略引擎、审计中间件 | DTF 30% |
| controller | ~4K | 控制循环、事件总线、生命周期、恢复 | DTF 20%, SDOF 15% |
| agent-runtime | ~1K | 上下文、会话记忆、工具结果缓存 | SDOF 15% |
| knowledge | ~6K | AgenticRAG、GraphRAG、上下文管理 | 间接相关 |
| scheduler | ~3K | 调度、Agent分级 | 间接相关 |

---

## 6. 优先级建议

### P0 — 高价值、低实现难度

1. **SkillRegistry风险分级 + 渐进式披露**
   - 来源: SDOF + SkillSmith
   - 工作量: ~2天
   - 修改: `crates/skills/src/skill.rs` 增加 `risk_level` 和 `disclosure_level`
   - 修改: `crates/skills/src/registry.rs` 查询支持分层返回

2. **前置条件验证 trait**
   - 来源: SDOF
   - 工作量: ~3天
   - 新增: `crates/skills/src/precondition.rs`
   - 集成: workflow-engine executor执行前调用

3. **审计升级为证据链**
   - 来源: DTF
   - 工作量: ~3天
   - 修改: `crates/data-governance/src/audit_middleware.rs`
   - 新增: 证明→批准→执行→结果的完整生命周期记录

### P1 — 高价值、中等实现难度

4. **技能离线编译器**
   - 来源: SkillSmith
   - 工作量: ~5天
   - 新增: `crates/skill-compiler/` crate
   - 功能: SKILL.md → CompiledArtifact(operators, inputs, policies, fallback)

5. **FSM阶段约束层**
   - 来源: SDOF
   - 工作量: ~5天
   - 修改: `crates/workflow-engine/` 增加 StageConstraint
   - 集成: dispatcher执行前做阶段合法性过滤

6. **共识验证框架**
   - 来源: DTF
   - 工作量: ~5天
   - 新增: `crates/controller/src/consensus.rs`
   - 支持N-of-M评估者模式

### P2 — 中等价值、较高实现难度

7. **意图路由器(专用模型)**
   - 来源: SDOF
   - 工作量: ~10天
   - 需要: 训练数据 + 微调pipeline

8. **临时执行身份**
   - 来源: DTF
   - 工作量: ~7天
   - 修改: `crates/mcp-protocol/src/credentials.rs`
   - 新增: 从proof派生的ephemeral identity with TTL

9. **MCP Tool → 可视化UI自动生成**
   - 来源: NIMO
   - 工作量: ~7天
   - 修改: `crates/agent-view/`
   - 动态从MCP tool schema生成可交互UI组件

---

## 附录: 论文间关系

```
SDOF (状态约束编排)
  │
  │ 前置条件验证 ──────────┐
  │                        │
  ▼                        ▼
SkillSmith (技能编译)    DTF (证明派生授权)
  │                        │
  │ 边界合约 ──────────→  证明 = 执行边界
  │ 渐进披露 ──────────→  共识 = 审批门控
  │                        │
  └──── NIMO (MCP统一层) ──┘
         │
         ▼
    KIAS mcp-protocol (已有基础)
```

三篇论文(SDOF/SkillSmith/DTF)从不同角度解决同一核心问题：**如何约束Agent的执行自由度**。
- SDOF: 通过业务阶段约束
- SkillSmith: 通过编译时边界提取
- DTF: 通过证明-共识-派生授权链
- NIMO: 通过MCP统一抽象层(已有)

KIAS当前最缺的是前三者的约束机制，最不缺的是NIMO的MCP抽象层。
