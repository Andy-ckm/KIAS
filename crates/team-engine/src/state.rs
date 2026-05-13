use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Agent 角色定义（借鉴 MiniMax Agent Team）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentRole {
    /// 控制面：理解目标、拆子任务、分配、合并结果、控制停止
    Owner,
    /// 执行面：具体执行任务
    Worker,
    /// 质量门禁：检查事实、格式、来源
    Verifier,
}

/// 任务状态（确定性状态机驱动）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// 待分配
    Pending,
    /// 已分配给 Worker
    Assigned,
    /// Worker 执行中
    InProgress,
    /// Worker 完成，等待验证
    Completed,
    /// Verifier 验证中
    Verifying,
    /// 验证通过
    Verified,
    /// 验证失败，需要重试
    Rejected,
    /// 任务失败
    Failed,
    /// 任务取消
    Cancelled,
}

/// 任务定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub description: String,
    pub assigned_to: Option<String>,  // Agent ID
    pub verified_by: Option<String>,  // Verifier ID
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub context: serde_json::Value,  // 上下文隔离
}

/// Agent 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub role: AgentRole,
    pub status: AgentStatus,
    pub current_task: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Agent 运行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Idle,
    Busy,
    Waiting,
    Failed,
}

/// Team 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamState {
    pub team_id: String,
    pub owner: Agent,
    pub workers: Vec<Agent>,
    pub verifiers: Vec<Agent>,
    pub tasks: Vec<Task>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
