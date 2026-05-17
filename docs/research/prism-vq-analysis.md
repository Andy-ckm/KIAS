# PRISM-VQ 深度分析：映射到 KIAS Agent 调度与知识聚类

> **论文**: Vector-Quantized Discrete Latent Factors Meet Financial Priors
> **arXiv**: 2605.13407 (2026-05-13)
> **作者**: Namhyoung Kim, Jae Wook Song
> **代码**: https://github.com/finxlab/PRISM-VQ

---

## 1. 论文核心思想

PRISM-VQ (PRior-Informed Stock Model with Vector Quantization) 是一个动态因子模型框架，解决股票收益预测中的低信噪比和市场机制变迁问题。

### 三大核心组件

| 组件 | 作用 | 关键创新 |
|------|------|----------|
| **VQ (Vector Quantization)** | 将连续特征离散化为 codebook 中的离散码 | 信息瓶颈抑制噪声，捕获稳健的截面结构 |
| **MoE (Mixture-of-Experts)** | 时间维度的专家专业化 | 离散码同时作为潜在因子和路由信号 |
| **FiLM (先验注入)** | 融合专家先验因子与数据驱动因子 | 保持可解释性，对齐金融理论 |

### 两阶段训练

```
Stage 1: VQ-VAE 学习离散表示
  └─ 截面特征 → Encoder → VQ Codebook → 离散码 (latent factors)
  └─ 信息瓶颈：噪声抑制 + 结构保留

Stage 2: 预测模型训练
  └─ 离散码 → MoE 路由 → 专家网络 → 动态因子载荷
  └─ FiLM: 先验因子 × 学习因子 → 融合预测
```

---

## 2. 技术深度解析

### 2.1 VQ 离散化

**原理**: 将连续向量映射到最近的 codebook 向量，通过 straight-through estimator 反传梯度。

```
输入: z ∈ R^d (连续嵌入)
Codebook: e ∈ R^{K×d} (K 个码本向量)
量化: z_q = e_k, where k = argmin_j ||z - e_j||²
输出: 离散码 index k, 量化向量 z_q
```

**为什么用离散而非连续?**
- 低信噪比环境下，连续表示容易过拟合噪声
- 离散码充当信息瓶颈，强制模型学习更鲁棒的结构
- 离散码天然形成聚类，可解释性强

### 2.2 MoE 路由

**离散码的双重角色**:
1. **潜在因子**: 直接用于因子模型
2. **路由信号**: 决定激活哪个专家网络

```
离散码 k → 路由权重 w = softmax(gate(k))
输出 = Σ_i w_i · Expert_i(x)
```

**时间维度的专业化**: 不同市场机制（牛市/熊市/震荡）对应不同专家，离散码自动触发切换。

### 2.3 FiLM 先验注入

FiLM (Feature-wise Linear Modulation) 将领域先验注入神经网络:

```
FiLM(x) = γ · x + β
其中 γ, β 由先验因子生成
```

**在 PRISM-VQ 中**:
- **先验因子**: 经典金融因子（Fama-French、动量、波动率等）
- **学习因子**: VQ 离散码对应的因子
- **融合**: FiLM 动态调制两者的权重

---

## 3. 映射到 KIAS 系统

### 3.1 VQ → Agent 能力聚类

**问题**: KIAS 中 Agent 能力描述是连续的（技能向量），如何高效匹配？

**映射方案**:

```
PRISM-VQ                          KIAS
─────────────────────────────────────────────────
截面特征 (N 股票 × D 特征)   →    Agent 能力矩阵 (N_agents × D_skills)
VQ Codebook (K 码本)         →    能力原型 (K 个代表性 Agent 类型)
离散码 index                 →    Agent 类别标签
信息瓶颈 + 噪声抑制          →    能力匹配鲁棒性
```

**具体实现思路**:

```rust
// crates/scheduler/src/algorithms/vq_cluster.rs (新增)

/// 能力原型 - 对应 VQ Codebook 中的码本向量
pub struct CapabilityPrototype {
    pub id: PrototypeId,
    pub skill_vector: Vec<f64>,    // 技能向量中心
    pub member_agents: Vec<AgentId>, // 属于此原型的 Agent
    pub usage_count: u64,
}

/// VQ 聚类调度器
pub struct VQClusterScheduler {
    codebook: Vec<CapabilityPrototype>,  // 能力原型集
    codebook_size: usize,                 // K = 原型数量
    learning_rate: f64,                   // EMA 更新率
}

impl VQClusterScheduler {
    /// 将 Agent 能力映射到最近原型 (VQ 量化)
    pub fn quantize(&self, agent_skills: &[f64]) -> PrototypeId {
        self.codebook
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                l2_distance(&a.skill_vector, agent_skills)
                    .partial_cmp(&l2_distance(&b.skill_vector, agent_skills))
                    .unwrap()
            })
            .map(|(i, _)| PrototypeId(i))
            .unwrap()
    }

    /// 调度: 先匹配原型 → 再在原型内选 Agent
    pub fn schedule(&self, task: &Task) -> AgentId {
        let task_skill_req = extract_skill_requirements(task);
        let proto = self.quantize(&task_skill_req);
        self.select_best_in_proto(proto, task)
    }

    /// 在线更新 codebook (EMA)
    pub fn update_codebook(&mut self, agent_skills: &[f64], proto_id: PrototypeId) {
        let proto = &mut self.codebook[proto_id.0];
        for (p, a) in proto.skill_vector.iter_mut().zip(agent_skills.iter()) {
            *p = *p * (1.0 - self.learning_rate) + a * self.learning_rate;
        }
    }
}
```

**优势**:
- O(K) 量化 + O(|proto|) 选择 ≪ O(N) 全量扫描
- 原型天然形成 Agent 分组，支持分层调度
- 新 Agent 加入时自动归类，无需手动标签

### 3.2 MoE → 多策略调度路由

**问题**: 不同任务类型（LLM推理/代码生成/数据处理）需要不同调度策略。

**映射方案**:

```
PRISM-VQ                          KIAS
─────────────────────────────────────────────────
MoE Experts (N 个专家)       →    调度策略池 (RR/LL/RA/CA/VQ)
离散码路由                    →    任务类型 → 策略选择
门控网络 g(k)                →    路由器 Router(task_features)
专家专业化                    →    策略专业化
```

**具体实现思路**:

```rust
// crates/scheduler/src/algorithms/moe_router.rs (新增)

/// 任务特征离散化 → 路由信号
pub struct TaskCodebook {
    entries: Vec<TaskPrototype>,  // 任务类型原型
}

/// MoE 调度路由器
pub struct MoESchedulerRouter {
    codebook: TaskCodebook,
    experts: Vec<Box<dyn SchedulingAlgorithm>>,  // 专家策略
    gate: GateNetwork,                            // 门控网络
}

impl MoESchedulerRouter {
    /// 路由: 任务 → 离散码 → 专家权重 → 融合调度
    pub fn schedule(&self, task: &Task, agents: &[Agent]) -> AgentId {
        // 1. 任务特征 → 离散码
        let task_features = extract_features(task);
        let code = self.codebook.quantize(&task_features);

        // 2. 离散码 → 专家权重
        let weights = self.gate.forward(code);

        // 3. 各专家独立打分
        let scores: Vec<f64> = agents.iter().map(|agent| {
            weights.iter().zip(self.experts.iter())
                .map(|(w, expert)| w * expert.score(task, agent))
                .sum()
        }).collect();

        // 4. 选最高分
        agents[scores.argmax()].id
    }
}
```

### 3.3 FiLM 先验注入 → Agent 置信度调制

**问题**: 如何融合领域先验（如 Agent 历史成功率）与实时信号？

**映射方案**:

```
PRISM-VQ                          KIAS
─────────────────────────────────────────────────
先验因子 (Fama-French)        →    Agent 先验 (历史成功率、延迟、可靠性)
学习因子 (VQ latent)          →    实时信号 (当前负载、队列深度)
FiLM 调制: γ·x + β           →    加权融合: α·prior + (1-α)·realtime
```

**具体实现思路**:

```rust
// crates/scheduler/src/optimizer/film_fusion.rs (新增)

/// FiLM 先验注入
pub struct PriorInjector {
    prior_weight: f64,  // α: 先验权重 (可学习)
}

impl PriorInjector {
    /// 融合先验因子与实时信号
    pub fn fuse(
        &self,
        prior_score: f64,    // 历史成功率、延迟等
        realtime_score: f64, // 当前负载、队列深度等
    ) -> f64 {
        // FiLM: γ · realtime + β
        // 简化版: 加权融合
        self.prior_weight * prior_score
            + (1.0 - self.prior_weight) * realtime_score
    }

    /// 自适应更新先验权重 (基于预测误差)
    pub fn update(&mut self, prediction: f64, actual: f64) {
        let error = (prediction - actual).abs();
        // 误差大 → 增加实时信号权重
        self.prior_weight *= 1.0 - 0.01 * error;
        self.prior_weight = self.prior_weight.clamp(0.1, 0.9);
    }
}
```

---

## 4. 架构集成方案

### 4.1 整体架构

```
Task Request
    │
    ▼
┌──────────────────────────────────────┐
│        MoE Scheduler Router          │
│  ┌─────────┐  ┌─────────┐  ┌──────┐ │
│  │ RR Expert│  │ LL Expert│  │ VQ E │ │
│  └─────────┘  └─────────┘  └──────┘ │
│        ▲           ▲           ▲     │
│        └───────────┼───────────┘     │
│                    │                 │
│              Gate Network            │
│         (Task Codebook Route)        │
└──────────────────────────────────────┘
                    │
                    ▼
┌──────────────────────────────────────┐
│        VQ Agent Cluster              │
│  ┌────────┐ ┌────────┐ ┌────────┐   │
│  │Proto A │ │Proto B │ │Proto C │   │
│  │(Code 0)│ │(Code 1)│ │(Code 2)│   │
│  └────────┘ └────────┘ └────────┘   │
│   Agent₁   Agent₃   Agent₅         │
│   Agent₂   Agent₄   Agent₆         │
└──────────────────────────────────────┘
                    │
                    ▼
┌──────────────────────────────────────┐
│        FiLM Prior Fusion             │
│  prior_weight × history_success_rate │
│  + (1-prior_weight) × current_load   │
│  → Final Agent Score                 │
└──────────────────────────────────────┘
                    │
                    ▼
           Selected Agent
```

### 4.2 实现路径

| 阶段 | 内容 | 涉及 crate | 工作量 |
|------|------|-----------|--------|
| Phase 1 | VQ 能力聚类 | `scheduler` | 2-3 天 |
| Phase 2 | MoE 路由器 | `scheduler` | 3-4 天 |
| Phase 3 | FiLM 先验注入 | `scheduler/optimizer` | 1-2 天 |
| Phase 4 | 集成测试 + 基准 | `benchmarks` | 2 天 |

### 4.3 与现有系统的兼容性

| 现有组件 | 兼容方式 |
|----------|----------|
| `SchedulingAlgorithm` trait | MoE Router 实现此 trait，内部调用多个子策略 |
| `SkillRegistry` | VQ Codebook 可从 SkillRegistry 初始化 |
| `HandoffController` | FiLM 融合可增强 `skill_match_score` 计算 |
| `ModelRouter` | MoE 路由逻辑可复用到 LLM 模型选择 |

---

## 5. 关键洞察与启示

### 5.1 离散化的力量

PRISM-VQ 的核心洞察：**在低信噪比环境中，离散表示比连续表示更鲁棒**。

对 KIAS 的启示：
- Agent 能力描述不应是无限精度的浮点向量
- 离散化形成自然聚类，简化匹配复杂度
- 新 Agent 自动归类，无需人工标签

### 5.2 双重角色的优雅

离散码同时作为**潜在因子**和**路由信号**，避免了额外的路由网络。

对 KIAS 的启示：
- 任务分类码可同时用于：(1) 描述任务类型 (2) 选择调度策略
- 一个 embedding 完成两件事，减少计算开销

### 5.3 先验与数据的平衡

FiLM 提供了一个优雅的框架来融合领域知识和数据驱动信号。

对 KIAS 的启示：
- 调度决策不应纯靠历史统计，也不应纯靠实时信号
- 自适应权重让系统在不同环境下自动调整
- 新节点冷启动时依赖先验，成熟后逐渐信任数据

---

## 6. 相关工作参考

| 论文/项目 | 关系 |
|-----------|------|
| [CVQ-VAE](https://github.com/lyndonzheng/CVQ-VAE) | PRISM-VQ 的 VQ 基础设施来源 |
| [VQ-VAE (van den Oord 2017)](https://arxiv.org/abs/1711.00937) | 向量量化开创性工作 |
| [Switch Transformer (Fedus 2022)](https://arxiv.org/abs/2101.03961) | MoE 路由的稀疏激活 |
| [FiLM (Perez 2018)](https://arxiv.org/abs/1709.07871) | 特征级线性调制 |
| Qlib (Microsoft) | 金融数据基础设施 |

---

## 7. 结论

PRISM-VQ 为 KIAS 提供了三个可落地的技术方向：

1. **VQ 聚类调度** → Agent 能力离散化，O(1) 匹配
2. **MoE 策略路由** → 任务驱动的多策略融合
3. **FiLM 先验注入** → 历史信号与实时信号的自适应融合

**建议优先级**: VQ 聚类 > FiLM 先验 > MoE 路由

VQ 聚类最直接解决当前 KIAS 的 Agent 匹配效率问题，且实现最简单。
