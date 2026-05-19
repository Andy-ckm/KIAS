# AgentGuard Agent Shell 调度架构设计

> 版本: 0.1 (Draft)
> 日期: 2026-05-16
> 状态: 设计中

## 1. 核心洞察

传统 K8S 调度的是 **Pod = Docker Image + Env Vars**，AgentGuard 调度的是
**Agent Shell = 模板 + 参数**。

用户不需要写 workflow，只需要表达需求。系统自动：
1. 识别意图（主动咨询 / 被动理解）
2. 匹配 Shell（能力模板）
3. 注入参数（从用户需求中提取）
4. 组装执行计划（Workflow / A2A）
5. 调度执行

## 2. 概念模型

```
┌─────────────────────────────────────────────────────┐
│                   用户需求 (Natural Language)         │
│           "帮我审查 ~/kias 的 Rust 代码安全性"        │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│              意图识别层 (Intent Layer)                 │
│                                                      │
│  ┌──────────┐  ┌──────────────┐  ┌────────────────┐ │
│  │ 被动理解  │  │  主动咨询     │  │  意图图谱生成   │ │
│  │ (NLU)    │  │  (Clarify)   │  │  (IntentGraph) │ │
│  └────┬─────┘  └──────┬───────┘  └───────┬────────┘ │
│       └───────────────┼──────────────────┘          │
└───────────────────────┼──────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────┐
│              Shell 匹配层 (Shell Matcher)              │
│                                                      │
│  IntentGraph → 匹配 capabilities → 候选 Shell 列表    │
│                                                      │
│  Shell[code-reviewer]  score: 0.92                   │
│  Shell[security-scanner] score: 0.87                 │
│  Shell[perf-analyzer]  score: 0.45 (不匹配)          │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│              参数提取层 (Parameter Extractor)          │
│                                                      │
│  用户原文 + Shell input_schema → 参数映射              │
│                                                      │
│  {                                                   │
│    repo_path: "~/workspace/kias",                    │
│    language: "rust",                                 │
│    focus: "security",                                │
│    depth: "deep"                                     │
│  }                                                   │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│              组装层 (Assembler)                        │
│                                                      │
│  Shell + Params → 实例化 Agent                       │
│  多个 Shell → 组装 Workflow DAG                      │
│  Shell 间有数据依赖 → 自动连线                        │
│                                                      │
│  ┌────────────┐    ┌──────────────┐    ┌──────────┐ │
│  │ code-review │───▶│ sec-scanner  │───▶│ reporter │ │
│  │ {repo,rs}  │    │ {findings}   │    │ {report} │ │
│  └────────────┘    └──────────────┘    └──────────┘ │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────────────┐
│              调度执行层 (Scheduler + Executor)         │
│                                                      │
│  Workflow DAG → Scheduler 分配 Node → 执行           │
│  复用 AgentGuard 现有 Scheduler + TeamEngine               │
└──────────────────────────────────────────────────────┘
```

## 3. Agent Shell 定义

### 3.1 Shell 声明 (YAML)

```yaml
apiVersion: kias/v1
kind: AgentShell
metadata:
  name: code-reviewer
  version: "1.0.0"
  author: kias-team
  tags: [code, review, quality]
spec:
  description: "审查代码质量、安全性和可维护性"

  capabilities:
    - code_review
    - security_scan
    - style_check

  input_schema:
    repo_path:
      type: string
      required: true
      description: "代码仓库路径"
    language:
      type: string
      required: true
      enum: [rust, python, go, typescript, java]
    review_depth:
      type: string
      default: "normal"
      enum: [shallow, normal, deep]
    focus_areas:
      type: array
      items:
        type: string
        enum: [security, performance, style, architecture]
      default: [security, style]

  output_schema:
    issues:
      type: array
      items:
        type: object
        properties:
          severity: { type: string, enum: [critical, warning, info] }
          file: { type: string }
          line: { type: integer }
          message: { type: string }
          suggestion: { type: string }
    score:
      type: number
      minimum: 0
      maximum: 100
    summary:
      type: string

  # 运行时配置
  runtime:
    image: "kias/code-reviewer:latest"
    timeout_seconds: 300
    memory_mb: 512
    cpu_cores: 1.0
    requires_network: true
    requires_filesystem: true
    sandbox_type: docker  # docker | gvisor | process | wasm

  # 可选: LLM 配置（如果是 LLM 驱动的 Agent）
  llm:
    provider: anthropic
    model: claude-sonnet-4
    temperature: 0.3
    max_tokens: 4096
    system_prompt: |
      你是一个专业的代码审查专家。
      审查以下代码，关注 {focus_areas} 方面。
      代码语言: {language}
      审查深度: {review_depth}
```

### 3.2 Shell 与 Agent 的关系

```
Shell (模板)          Agent (实例)
┌──────────┐         ┌──────────────────┐
│ 能力声明  │         │ Shell 引用        │
│ 输入 Schema│  ───▶  │ 已注入的参数       │
│ 输出 Schema│         │ 运行时状态         │
│ 运行时配置 │         │ 执行历史          │
└──────────┘         └──────────────────┘

Shell = Class, Agent = Instance
```

## 4. 意图识别层 (Intent Layer)

### 4.1 被动理解 (Passive Understanding)

从用户自然语言中提取意图，不需要用户明确指定：

```
输入: "帮我看看 ~/kias 的代码有没有安全问题"

NLU 解析:
  intent: security_audit
  entities:
    target: "~/kias"
    scope: "security"
    language: (推断) rust
  confidence: 0.85
```

### 4.2 主动咨询 (Active Clarification)

当置信度不足时，主动询问用户：

```
系统: "你提到要审查代码，我确认几个问题："
  1. "审查范围是整个仓库还是特定目录？"  [整个仓库]
  2. "重点关注安全、性能还是代码风格？"  [安全]
  3. "需要深度审查还是快速扫描？"        [深度]
```

### 4.3 意图图谱 (IntentGraph)

```rust
pub struct IntentGraph {
    /// 主意图
    pub primary_intent: Intent,
    /// 子意图（主意图分解）
    pub sub_intents: Vec<Intent>,
    /// 意图间依赖关系
    pub dependencies: Vec<(IntentId, IntentId)>,
    /// 置信度
    pub confidence: f64,
    /// 已确认的参数
    pub confirmed_params: HashMap<String, ParamValue>,
    /// 待确认的参数
    pub pending_params: Vec<PendingParam>,
}

pub struct Intent {
    pub id: IntentId,
    pub name: String,
    pub description: String,
    pub required_capabilities: Vec<String>,
    pub params: HashMap<String, ParamValue>,
}
```

## 5. Shell 匹配算法

```
Score(Shell, Intent) =
    capability_match(intent.required, shell.capabilities) * 0.4
  + schema_compatibility(intent.params, shell.input_schema) * 0.3
  + historical_success_rate(shell) * 0.2
  + user_preference_score(shell) * 0.1
```

## 6. 参数提取与注入

### 6.1 提取策略

| 参数类型 | 提取方式 | 示例 |
|---------|---------|------|
| 显式参数 | 直接从用户文本提取 | "用 Rust 写的" → language=rust |
| 路径参数 | 正则匹配文件路径 | "~/workspace/kias" → repo_path |
| 推断参数 | 从上下文推断 | 代码仓库 → 自动检测语言 |
| 默认参数 | Shell schema 中的默认值 | review_depth=normal |
| 交互参数 | 主动询问用户 | "需要深度审查吗？" |

### 6.2 参数验证

```rust
pub fn validate_params(
    shell: &AgentShell,
    params: &HashMap<String, ParamValue>,
) -> ValidationResult {
    // 1. 必填参数检查
    // 2. 类型检查
    // 3. 枚举值检查
    // 4. 范围检查
    // 5. 缺失参数 → 加入 pending_params
}
```

## 7. 自动组装 Workflow

当一个意图需要多个 Shell 协作时，自动组装 DAG：

```
意图: "代码审查 + 安全扫描 + 生成报告"

自动组装:
  ┌────────────┐     ┌────────────┐     ┌────────────┐
  │ code-review │────▶│ sec-scanner │────▶│ reporter   │
  │             │     │             │     │            │
  │ 输入: repo  │     │ 输入:       │     │ 输入:      │
  │ 输出: issues│     │   issues    │     │   findings │
  │             │     │ 输出:       │     │ 输出:      │
  └────────────┘     │   vulns     │     │   report   │
                     └────────────┘     └────────────┘

数据流自动连线:
  code-review.issues → sec-scanner.input
  sec-scanner.vulns + code-review.issues → reporter.input
```

### 7.1 组装规则

```rust
pub struct AssemblyRule {
    /// 当意图包含这些能力时触发
    pub trigger_capabilities: Vec<String>,
    /// 需要的 Shell 组合
    pub shell_sequence: Vec<ShellRef>,
    /// 数据流连线
    pub data_flows: Vec<DataFlow>,
    /// 可选: 并行执行的分支
    pub parallel_branches: Option<Vec<Vec<ShellRef>>>,
}
```

## 8. 与现有 AgentGuard 组件的集成

```
┌─────────────────────────────────────────────────┐
│                  AgentGuard 架构                        │
│                                                  │
│  ┌─────────────┐     ┌─────────────────────┐    │
│  │ NL API      │────▶│ Intent Layer (新)    │    │
│  │ (现有)       │     │ - 被动理解           │    │
│  └─────────────┘     │ - 主动咨询           │    │
│                      │ - 意图图谱           │    │
│                      └─────────┬───────────┘    │
│                                │                 │
│                      ┌─────────▼───────────┐    │
│                      │ Shell Matcher (新)   │    │
│                      │ - 能力匹配           │    │
│                      │ - 参数提取           │    │
│                      │ - 自动组装           │    │
│                      └─────────┬───────────┘    │
│                                │                 │
│          ┌─────────────────────┼──────────┐     │
│          ▼                     ▼          ▼     │
│  ┌──────────────┐  ┌──────────────┐ ┌────────┐ │
│  │ Workflow      │  │ TeamEngine   │ │Scheduler│ │
│  │ Engine (现有) │  │ (现有)       │ │ (现有)  │ │
│  └──────────────┘  └──────────────┘ └────────┘ │
└─────────────────────────────────────────────────┘
```

## 9. 实现路线

### Phase 1: Shell 定义 + 注册 (1-2天)
- [ ] Shell YAML schema 定义
- [ ] Shell 注册表（skills crate 扩展）
- [ ] Shell 解析器 + 验证器

### Phase 2: 意图识别层 (2-3天)
- [ ] IntentGraph 数据结构
- [ ] 被动理解（NLU 解析）
- [ ] 主动咨询（澄清对话）
- [ ] 参数提取器

### Phase 3: Shell 匹配 + 组装 (2-3天)
- [ ] 匹配算法（能力+Schema+历史）
- [ ] 自动组装 Workflow DAG
- [ ] 数据流自动连线

### Phase 4: 集成 + 端到端 (1-2天)
- [ ] NL API → Intent → Shell → Workflow → Scheduler 全链路
- [ ] 端到端测试
- [ ] 示例 Shell 库

## 10. 参考方向

需要在 GitHub 上搜索类似项目：
- Agent template / Agent shell 框架
- Intent recognition for AI agents
- Agent composition / orchestration
- Parameterized agent systems
- Agent marketplace / registry

## 11. 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| Shell 格式 | YAML | 与 AgentGuard 现有 workflow 一致，人类可读 |
| 参数类型系统 | JSON Schema 子集 | 标准化，可验证，可序列化 |
| 意图识别 | LLM + 规则混合 | 简单场景用规则，复杂场景用 LLM |
| 匹配算法 | 加权评分 | 可解释，可调参，可学习 |
| 组装策略 | 基于数据流的 DAG | 复用现有 WorkflowEngine |
