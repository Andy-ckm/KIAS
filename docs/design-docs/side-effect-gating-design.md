# Side-effect Gating (Dry-Run Harness) 设计文档

> 来源: all-agentic-architectures #17 Dry-Run Harness
> 日期: 2026-05-18
> 状态: AgentGuard 架构补充设计

## 1. 它要解决什么问题？

AgentGuard 的 auto-loop 可以自动生成代码、修改文件、执行命令——但没有一个机制在执行前预演副作用。

GxP 合规场景的刚需：
- 修改生产配置前必须预演影响范围
- 批量数据操作前必须 dry-run 确认影响行数
- 发送外部通知前必须人工审批内容
- 删除文件/记录前必须确认无依赖

approval.rs 有审批流（Draft→Reviewing→Approved→Published），但缺少 **dry-run 预演机制**——审批人看到的是文字描述，不是实际执行效果的模拟。

## 2. State 设计

```rust
/// 副作用操作的执行模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionMode {
    /// 仅预演，不产生真实副作用
    DryRun,
    /// 正式执行，产生真实副作用
    Execute,
}

/// 副作用操作记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffectAction {
    pub id: Uuid,
    pub action_type: ActionType,
    pub target: String,
    pub parameters: serde_json::Value,
    pub mode: ExecutionMode,
    pub preview_result: Option<ExecutionPreview>,
    pub actual_result: Option<ExecutionResult>,
    pub approval: Option<ApprovalDecision>,
    pub created_at: DateTime<Utc>,
}

/// 动作类型分类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionType {
    FileWrite,      // 写文件
    FileDelete,     // 删文件
    CommandExec,    // 执行命令
    NetworkRequest, // 外部 HTTP 请求
    DataMutation,   // 数据库写操作
    Notification,   // 发送通知
}

/// 预演结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPreview {
    pub would_affect: Vec<String>,     // 受影响的资源列表
    pub risk_level: RiskLevel,         // 风险等级
    pub diff: Option<String>,          // 变更差异
    pub estimated_impact: String,      // 影响描述
    pub reversible: bool,              // 是否可逆
}

/// 审批决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub approver: String,
    pub decision: ApprovalOutcome,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalOutcome {
    Approved,
    Rejected,
    Modified { notes: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,      // 只读操作、本地文件
    Medium,   // 写本地文件、修改配置
    High,     // 外部请求、数据删除
    Critical, // 生产环境、不可逆操作
}
```

## 3. 拓扑：三阶段控制流

```
请求 → [Dry-Run 预演] → [风险评估] → [人工/自动审批] → [正式执行]
                                    ↓ (拒绝)
                                  取消 + 记录
```

关键设计：工具本身带 `dry_run` 参数，Workflow 显式插入审批 step。

## 4. 与 approval.rs 的关系

approval.rs 处理**文档审批**（知识生命周期）。
Side-effect Gating 处理**动作审批**（执行生命周期）。

两者互补：
- approval.rs: Draft → Reviewing → Approved → Published
- Side-effect Gating: Preview → Risk-Assess → Approve/Reject → Execute

## 5. 实现方案

### 5.1 工具层：所有副作用工具内置 dry_run 参数

```rust
pub trait SideEffectTool {
    fn execute(&self, params: &Value, mode: ExecutionMode) -> ToolResult;
    
    fn dry_run(&self, params: &Value) -> ExecutionPreview {
        self.execute(params, ExecutionMode::DryRun).into_preview()
    }
}
```

### 5.2 控制流层：auto-loop 集成

在 auto-loop 的 generate 阶段，所有副作用操作必须经过 gating：

```rust
// auto-loop generate 阶段
fn gated_execute(action: SideEffectAction) -> Result<ExecutionResult> {
    // Step 1: Dry-run 预演
    let preview = tool.dry_run(&action.parameters)?;
    
    // Step 2: 风险评估（自动或人工）
    let risk = assess_risk(&action, &preview);
    
    // Step 3: 审批
    match risk {
        RiskLevel::Low => {
            // 自动通过，直接执行
            tool.execute(&action.parameters, ExecutionMode::Execute)
        }
        RiskLevel::Medium => {
            // 需要自动检查通过
            if auto_approve_check(&action, &preview)? {
                tool.execute(&action.parameters, ExecutionMode::Execute)
            } else {
                Err(Error::RequiresHumanApproval)
            }
        }
        RiskLevel::High | RiskLevel::Critical => {
            // 必须人工审批
            let decision = request_human_approval(&action, &preview)?;
            match decision.outcome {
                ApprovalOutcome::Approved => {
                    tool.execute(&action.parameters, ExecutionMode::Execute)
                }
                ApprovalOutcome::Rejected => {
                    Err(Error::RejectedByReviewer)
                }
                ApprovalOutcome::Modified { notes } => {
                    // 用修改后的参数重新执行
                    tool.execute(&action.parameters, ExecutionMode::Execute)
                }
            }
        }
    }
}
```

### 5.3 GxP 合规集成

dry-run 记录必须写入 gxp_audit 的不可变审计链：
- 每次 dry-run 预演 → 审计日志
- 每次审批决策 → 审计日志（含审批人签名）
- 每次正式执行 → 审计日志（含 dry-run 对比）

## 6. 失败模式

| 失败场景 | 影响 | 缓解措施 |
|---------|------|---------|
| dry-run 环境与真实环境不一致 | 预演通过但执行失败 | 记录差异，定期校准 |
| 人工审批瓶颈 | 执行延迟 | 低风险自动通过 + 审批超时机制 |
| 预演信息泄漏 | preview 内容暴露敏感数据 | preview 脱敏 + 权限控制 |
| 过度保守 | 大量操作被拒 | 可调阈值 + 历史统计 |

## 7. AgentGuard 对接

| AgentGuard 模块 | 对接方式 |
|-----------|---------|
| approval.rs | 文档审批流程复用 |
| gxp_audit.rs | 审计链记录 dry-run 和执行 |
| gxp_auth.rs | 审批人身份验证（电子签名） |
| tool-executor | 工具层内置 dry_run 参数 |
| auto-loop | generate 阶段集成 gating |
| team-engine | 多 agent 场景的审批路由 |

## 8. 优先级

**P0 — GxP 合规必需**。没有 dry-run 机制，AgentGuard 在受监管行业的执行力是裸奔的。
