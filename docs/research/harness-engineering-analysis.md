# Harness Engineering 综合分析

> 主题: Agent = Model + Harness 工程方法论
> 日期: 2026-05-18
> 参考来源:
>   - "Harness Engineering: Agent = Model + Harness", 知乎, 2026
>   - [2605.15184] Is Grep All You Need? How Agent Harnesses Reshape Agentic Search
>   - [2605.15218] CAX-Agent: A Lightweight Agent Harness for Reliable APDL Automation
>   - [2605.13821] Harnessing Agentic Evolution
>   - harnesslabs/arbiter (Multi-Agent Framework)
>   - 1jehuang/jcode (Coding Agent Harness in Rust)
>   - OpenClaw (Execution Firewall + Agent Sandbox)
>   - moosestack (Agent Harness for Analytics)

## 一、Harness Engineering 核心理论

### 1.1 定义与公式

**核心公式**：
```
Agent(T, M) = Σᵢ [Select(m*, h*) | Step_i]
```
- T = 任务，M = 模型集
- m* = 最优模型，h* = 最优 Harness 参数
- 目标：min Loss / min TokenCost

**联合优化公式**：
```
AGI_next = Model_Params ⊕ Harness_Params → 联合迭代优化
```

### 1.2 为什么 Harness 不会被模型吃掉

| 原因 | 说明 | 实证 |
|------|------|------|
| 模型"七国八制" | 不同模型在不同任务上表现差异大，benchmark 与实际表现关联度低 | GPT-4o 编码强但推理弱，Claude 推理强但代码弱 |
| 任务会"打架" | 快慢思考、超分去模糊等任务本质上冲突，无法用单一模型统一 | 快思考(模式匹配) vs 慢思考(逻辑推理) 需不同策略 |
| 复杂任务需多模型协同 | 多模态理解+生成、具身智能感知+决策+运控 | Agent 需要 router 动态选择模型 |

### 1.3 Harness 的五个层次

```
Layer 5: Self-Evolution (自我进化)
  ├── auto-loop: 用 Agent 开发 Agent
  ├── learner: 从经验中学习
  └── feedback: 闭环反馈机制

Layer 4: Safety & Governance (安全与治理)
  ├── approval: 人工审批门禁
  ├── audit: 全链路审计
  └── policy: 工具使用策略

Layer 3: Knowledge & Memory (知识与记忆)
  ├── RAG: 检索增强生成
  ├── GraphRAG: 图结构知识
  ├── memory_layers: 分层记忆
  └── context_manager: 上下文管理

Layer 2: Orchestration (编排)
  ├── model-router: 模型选择路由
  ├── workflow-engine: DAG 工作流
  ├── team-engine: 多 Agent 协作
  └── goal-engine: 目标驱动循环

Layer 1: Runtime (运行时)
  ├── tool-executor: 工具执行
  ├── mcp-protocol: 标准化协议
  ├── sandbox: 沙箱隔离
  └── agent-runtime: Agent 生命周期
```

---

## 二、论文分析

### 2.1 [2605.15184] Is Grep All You Need? How Agent Harnesses Reshape Agentic Search

**核心论点**：Agent 的搜索能力不仅取决于模型，更取决于 Harness 如何组织和呈现信息。

**关键发现**：
1. **Grep vs RAG**：在代码搜索场景，grep-like 工具在精确匹配上优于向量搜索
2. **Harness 决定信息流**：Agent 的搜索质量取决于 Harness 如何组织工具调用序列
3. **分层检索**：先粗粒度过滤（grep），再细粒度理解（LLM），比纯向量搜索更高效

**对 KIAS 的映射**：
| 论文概念 | KIAS 模块 | 应用 |
|---------|----------|------|
| Agent Harness | kias-main | 统一 Agent 运行时编排 |
| Grep-like search | knowledge/retriever | TF-IDF + 关键词匹配，已实现 |
| 分层检索 | knowledge/agentic_rag | AgenticRAG 粗→细漏斗 |
| 工具编排 | tool-executor + mcp-protocol | 标准化工具调用 |

**KIAS 行动项**：
1. **[高]** 评估 AgenticRAG 是否需要增加 grep-like 快速过滤层
2. **[中]** 在 retriever.rs 中增加代码搜索专用策略

### 2.2 [2605.15218] CAX-Agent: A Lightweight Agent Harness for Reliable APDL Automation

**核心论点**：轻量级 Agent Harness 可以在专业领域（如工程仿真 APDL）实现可靠自动化。

**关键设计模式**：
1. **分层 Harness**：解析层 → 理解层 → 执行层 → 验证层
2. **可靠性保证**：每个执行步骤有验证门禁
3. **轻量化**：Harness 不应成为性能瓶颈

**对 KIAS 的映射**：
| CAX-Agent 设计 | KIAS 模块 | 对应关系 |
|---------------|----------|---------|
| 解析层 | knowledge/entity_extractor | 实体抽取 |
| 执行层 | tool-executor + sandbox | 工具执行 + 沙箱 |
| 验证层 | team-engine/verifier | Worker-Verifier 对抗 |
| 轻量化 | scheduler 优化 | 资源感知调度 |

**KIAS 行动项**：
1. **[中]** 研究 tool-executor 的验证门禁机制
2. **[低]** 评估 Harness 轻量化策略对延迟的影响

### 2.3 [2605.13821] Harnessing Agentic Evolution

**核心论点**：Agent 演化需要稳定接口来组织证据（候选方案、反馈、轨迹），而非固定的流程或通用 Agent。

**对 KIAS 的映射**（详见 [agentic-evolution-analysis.md](agentic-evolution-analysis.md)）：
1. workflow-engine: 演化反馈循环
2. goal-engine: 目标驱动迭代
3. auto-loop: 代码演化
4. skills: 演化轨迹→技能提取

---

## 三、开源项目分析

### 3.1 harnesslabs/arbiter ⭐740

**定位**：Multi-agent framework for design, simulation, and auditing

**架构**：
```
arbiter/
├── design/      # Agent 设计 DSL
├── simulation/  # 仿真测试环境
└── auditing/    # 审计日志
```

**KIAS 借鉴**：
- 设计/仿真/审计三位一体的 Agent 开发范式
- 多 Agent 协调的审计日志机制
- **差距**：KIAS 有 gxp_audit 做审计，但缺少仿真测试环境

### 3.2 1jehuang/jcode ⭐6,116 (Rust)

**定位**：Coding Agent Harness — 可嵌入、可扩展的 AI 编码 Agent

**核心设计**：
- Rust 实现，高性能
- Harness 模式：Agent 作为可嵌入组件
- 插件化工具系统

**KIAS 借鉴**：
- Agent harness 设计模式：Agent = 可嵌入的 Harness + 可替换的 Model
- Rust 实现参考：cargo workspace 结构、async runtime 设计
- **差距**：KIAS 更偏集群调度，jcode 更偏单 Agent 编码

### 3.3 OpenClaw

**定位**：Execution Firewall — Seccomp-locked AI agent sandbox

**核心设计**：
- Seccomp 系统调用过滤
- 策略驱动的命令治理
- Agent 执行沙箱

**KIAS 借鉴**：
- 沙箱安全机制 → KIAS sandbox 模块
- 策略驱动执行 → KIAS autonomy-controller
- **差距**：KIAS sandbox 是容器级隔离，OpenClaw 是系统调用级

### 3.4 moosestack ⭐578

**定位**：Agent Harness for building analytics into apps on top of ClickHouse, Redpanda

**核心设计**：
- Agent Harness 封装数据分析能力
- ClickHouse + Redpanda 数据管道
- 嵌入式 Agent 模式

**KIAS 借鉴**：
- Harness 作为能力封装层的模式
- 嵌入式 Agent 集成方式
- **差距**：KIAS 是独立集群系统，moosestack 是嵌入式库

---

## 四、KIAS Harness 架构全景映射

### 4.1 完整映射表

| Harness 要素 | 理论来源 | KIAS 模块 | 实现状态 | 备注 |
|-------------|---------|----------|---------|------|
| **模型选择路由** | Harness 公式1 | model-router + tier_routing | ✅ 已实现 | 任务复杂度→最优模型 |
| **Prompt/技能优化** | Harness 4.3 | skills + quality_pipeline | ✅ 已实现 | 技能文件 + 竞技场评估 |
| **RAG/知识检索** | 2605.15184 | knowledge + graphrag + entity_extractor | ✅ 已实现 | 混合检索 + 知识图谱 |
| **分层记忆** | 2605.13438 (Cognifold) | memory_layers + DreamConsolidator | ✅ 已实现 | 工作/长期/程序三层 |
| **安全/审计** | Harness 4.3 | gxp_audit + gxp_auth + approval | ✅ 已实现 | GxP 合规全链路 |
| **工具调用** | 2605.15184 | tool-executor + mcp-protocol | ✅ 已实现 | 标准化工具协议 |
| **自我进化** | 2605.13821 | auto-loop + learner | ⚠️ 部分实现 | 闭环反馈待完善 |
| **Harness 轻量化** | 2605.15218 (CAX) | scheduler 优化 | ⚠️ 部分实现 | 延迟优化待做 |
| **仿真测试** | harnesslabs/arbiter | N/A | ❌ 未实现 | Agent 仿真测试环境 |
| **执行防火墙** | OpenClaw | sandbox | ⚠️ 部分实现 | 容器级，非系统调用级 |
| **嵌入式 Harness** | moosestack/jcode | kias-main | ✅ 已实现 | 可嵌入式 Agent 运行时 |

### 4.2 KIAS 独特优势

KIAS 相比其他 Harness 实现的独特优势：

1. **集群级 Harness**：不仅单 Agent Harness，而是集群调度 + Agent Harness 的融合
2. **声明式 API**：借鉴 K8S 的声明式管理，Harness 配置即代码
3. **GxP 合规**：内置审计、审批、合规全链路，适合制药/金融等高合规场景
4. **Rust 性能**：全 Rust 实现，零 GC 延迟，适合生产环境
5. **可观测性**：Prometheus 指标 + 审计日志 + 健康检查，Harness 行为完全可观测

### 4.3 与其他 Harness 框架的对比

| 维度 | KIAS | jcode | OpenClaw | moosestack | arbiter |
|------|------|-------|----------|------------|---------|
| 语言 | Rust | Rust | - | - | - |
| 定位 | 集群调度 | 编码 Agent | 沙箱安全 | 数据分析 | 仿真审计 |
| 模型路由 | ✅ | ❌ | ❌ | ❌ | ❌ |
| 多 Agent | ✅ | ❌ | ❌ | ❌ | ✅ |
| 工作流 | ✅ DAG | ❌ | ❌ | ❌ | ❌ |
| RAG | ✅ | ❌ | ❌ | ❌ | ❌ |
| 合规审计 | ✅ GxP | ❌ | ❌ | ❌ | ✅ |
| 沙箱 | ✅ 容器 | ❌ | ✅ seccomp | ❌ | ❌ |

---

## 五、灵魂之争的深度分析

### 5.1 问题重述

> 如果 Harness 控制模型选择，甚至基于 Harness 数据增训模型，灵魂到底属于谁？

### 5.2 多维分析

**维度 1：可替换性**
- 模型可替换：换 GPT-4o → Claude → DeepSeek，Agent 行为基本不变
- Harness 不可替换：换掉 skills/memory/approval，Agent 行为完全不同
- **结论**：灵魂在 Harness

**维度 2：身份连续性**
- 模型更新（GPT-4 → GPT-5）：Agent 能力增强但身份不变
- Harness 更新（新技能/新记忆）：Agent 身份演化
- **结论**：Harness 是 Agent 身份的载体

**维度 3：控制权**
- 模型：被动执行，接受指令
- Harness：主动选择，决定策略
- **结论**：Harness 拥有控制权

### 5.3 KIAS 的实践答案

KIAS 通过以下机制实践"灵魂在 Harness"：

1. **model-router**：Harness 决定用哪个模型，模型不知道自己被选中
2. **skills**：Harness 定义 Agent 的能力边界，模型只是执行器
3. **memory_layers**：Harness 管理记忆，模型无状态
4. **approval**：Harness 控制安全边界，模型无权限概念
5. **auto-loop**：Harness 驱动自我进化，模型不会自我改进

---

## 六、KIAS 行动项汇总

### 高优先级
1. 在 AgenticRAG 中增加 grep-like 快速过滤层（来源：2605.15184）
2. 完善 self-evolution 闭环：learner 经验 → RAG 知识库（来源：2605.13821）

### 中优先级
3. 研究 tool-executor 的验证门禁机制（来源：2605.15218 CAX-Agent）
4. 评估 Agent 仿真测试环境的必要性（来源：harnesslabs/arbiter）
5. 研究 sandbox 从容器级→系统调用级的可能性（来源：OpenClaw）

### 低优先级
6. 评估 Harness 轻量化策略对调度延迟的影响
7. 研究嵌入式 Harness 模式用于 KIAS-as-library 场景

---

*最后更新: 2026-05-18*
