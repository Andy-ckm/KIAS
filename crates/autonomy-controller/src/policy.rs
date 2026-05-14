use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 工具权限级别
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ToolPermission {
    /// 自动批准
    AutoApprove,
    /// 需要确认
    RequireConfirmation,
    /// 禁止执行
    Forbidden,
}

/// 工具策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub tool_name: String,
    pub permission: ToolPermission,
    pub requires_sandbox: bool,
    pub requires_network: bool,
    pub max_execution_time: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ToolPolicy {
    pub fn new(tool_name: &str, permission: ToolPermission) -> Self {
        Self {
            tool_name: tool_name.to_string(),
            permission,
            requires_sandbox: true,
            requires_network: false,
            max_execution_time: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// 设置是否需要沙箱
    pub fn with_sandbox(mut self, requires: bool) -> Self {
        self.requires_sandbox = requires;
        self
    }

    /// 设置是否需要网络
    pub fn with_network(mut self, requires: bool) -> Self {
        self.requires_network = requires;
        self
    }

    /// 设置最大执行时间
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.max_execution_time = Some(seconds);
        self
    }

    /// 检查是否允许执行
    pub fn is_allowed(&self) -> bool {
        !matches!(self.permission, ToolPermission::Forbidden)
    }

    /// 检查是否需要确认
    pub fn needs_confirmation(&self) -> bool {
        matches!(self.permission, ToolPermission::RequireConfirmation)
    }
}
