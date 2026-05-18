# Self-boundary Reasoning (Metacognitive Agent) 设计文档

> 来源: all-agentic-architectures #18 Reflexive Metacognitive Agent
> 日期: 2026-05-18
> 状态: KIAS 架构补充设计

## 1. 它要解决什么问题？

KIAS 的 tier_routing 按任务复杂度选模型（简单→小模型，复杂→大模型），但缺少一个更根本的判断：**这个任务我到底该不该做？**

当前 tier_routing 的盲区：
- 不知道自己不擅长什么（置信度估计缺失）
- 不知道哪些任务应该交给人类（escalation 策略缺失）
- 不知道什么时候应该调工具而不是自己推理（策略选择缺失）

在医疗、法律、金融场景，agent 最强的能力不是"回答"，而是"拒绝"。

## 2. State 设计

```rust
/// 元认知分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetacognitiveAnalysis {
    /// 置信度 0.0~1.0
    pub confidence: f64,
    /// 选择的策略
    pub strategy: ResponseStrategy,
    /// 推理过程
    pub reasoning: String,
    /// 如果策略是 UseTool，指定用哪个工具
    pub tool_to_use: Option<String>,
    /// 如果策略是 Escalate，指定原因
    pub escalation_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStrategy {
    /// 置信度高 + 低风险 → 直接回答
    ReasonDirectly,
    /// 有匹配工具 → 调工具
    UseTool { tool_name: String },
    /// 高风险或低置信度 → 交给人类
    Escalate { reason: String },
    /// 置信度中等 + 有部分知识 → 回答但标注不确定性
    ReasonWithCaveat { caveat: String },
}

/// Agent 自我模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    /// 擅长的知识领域
    pub knowledge_domains: Vec<String>,
    /// 可用工具列表
    pub tools_available: Vec<ToolCapability>,
    /// 置信度阈值（低于此值自动 escalate）
    pub confidence_threshold: f64,
    /// 高风险主题（必须 escalate）
    pub high_risk_topics: Vec<String>,
    /// 历史表现统计
    pub performance_stats: PerformanceStats,
}

/// 工具能力描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    pub description: String,
    pub applicable_domains: Vec<String>,
    pub reliability: f64, // 历史成功率
}

/// 历史表现统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStats {
    pub total_tasks: u64,
    pub direct_success_rate: f64,
    pub tool_success_rate: f64,
    pub escalation_rate: f64,
    pub false_confidence_count: u64, // 自信回答但被纠正的次数
}
```

## 3. 拓扑：两级路由

```
用户请求 → [Self-Model 评估] → Router:
                                  ├─ ReasonDirectly → 直接回答
                                  ├─ UseTool → 调用指定工具 → 回答
                                  ├─ ReasonWithCaveat → 回答 + 不确定性标注
                                  └─ Escalate → 交给人类 + 原因说明
```

## 4. 与 tier_routing 的关系

| 维度 | tier_routing | Self-boundary Reasoning |
|------|-------------|----------------------|
| 判断什么 | 用哪个模型 | 该不该做、怎么做 |
| 输入 | 任务复杂度 | 任务 + 自我模型 + 历史表现 |
| 输出 | 模型选择 | 策略选择（直接/工具/升级） |
| 层级 | 模型选择层 | 任务决策层 |

Self-boundary Reasoning 在 tier_routing **之前**执行：
```
请求 → Self-boundary → (如果 ReasonDirectly) → tier_routing → 模型选择
                          (如果 UseTool) → 直接调工具
                          (如果 Escalate) → 交给人类
```

## 5. 实现方案

### 5.1 自我模型初始化

```rust
impl SelfModel {
    /// 从 KIAS 的知识库自动构建自我模型
    pub fn from_knowledge_base(kb: &KnowledgeBase) -> Self {
        Self {
            knowledge_domains: kb.get_domain_list(),
            tools_available: ToolRegistry::list_capabilities(),
            confidence_threshold: 0.7,
            high_risk_topics: vec![
                "medical diagnosis".into(),
                "legal advice".into(),
                "financial trading".into(),
                "production data deletion".into(),
            ],
            performance_stats: PerformanceStats::load_from_audit_log(),
        }
    }
}
```

### 5.2 元认知评估（auto-loop 集成）

在 auto-loop 的 detect 阶段之后、analyze 阶段之前插入：

```rust
// auto-loop 扩展
fn metacognitive_gate(task: &Task, self_model: &SelfModel) -> MetacognitiveAnalysis {
    let prompt = format!(
        "Self-model: {:?}
Task: {}

         Estimate confidence and pick strategy.",
        self_model, task.description
    );
    
    // 用小模型做快速评估（不消耗大模型 token）
    let analysis: MetacognitiveAnalysis = small_model.structured_predict(&prompt);
    
    // 程序化覆盖：高风险主题强制 escalate
    if self_model.high_risk_topics.iter()
        .any(|t| task.description.to_lowercase().contains(t)) 
    {
        analysis.strategy = ResponseStrategy::Escalate {
            reason: "High-risk topic detected".into(),
        };
    }
    
    analysis
}
```

### 5.3 自我模型进化

每次任务完成后，更新 PerformanceStats：
- 直接回答成功 → direct_success_rate 更新
- 调工具成功 → tool_success_rate 更新
- 被用户纠正 → false_confidence_count + 1 → 调低相关领域置信度
- 成功 escalate → escalation_rate 更新

## 6. 失败模式

| 失败场景 | 影响 | 缓解措施 |
|---------|------|---------|
| 置信度估计不准 | 低估→过度保守，高估→危险自信 | 历史数据校准 + 人工反馈 |
| 自我模型过时 | 工具新增但模型不知道 | 定期重建 + 工具注册自动更新 |
| 高风险主题匹配粗糙 | 正常任务被误判为高风险 | 语义匹配替代关键词匹配 |
| escalation 过多 | 人工负担过重 | 分级 escalation + 自动学习 |

## 7. KIAS 对接

| KIAS 模块 | 对接方式 |
|-----------|---------|
| tier_routing | Self-boundary 在其之前执行，决定是否需要模型选择 |
| scheduler | 任务调度时考虑 agent 能力边界 |
| skills | 技能注册表提供 tools_available |
| knowledge | 知识域列表提供 knowledge_domains |
| gxp_audit | 记录每次 escalate 决策 |
| auto-loop | detect→**metacognitive**→analyze→plan→generate→verify |

## 8. 优先级

**P1 — 生产安全必需**。没有自我边界建模，agent 在高风险场景下可能做出危险的自信回答。
对于 GxP 合规，"拒绝回答"的能力和"回答正确"的能力同样重要。
