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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_policy_defaults() {
        let policy = ToolPolicy::new("shell", ToolPermission::AutoApprove);
        assert_eq!(policy.tool_name, "shell");
        assert_eq!(policy.permission, ToolPermission::AutoApprove);
        assert!(policy.requires_sandbox);
        assert!(!policy.requires_network);
        assert!(policy.max_execution_time.is_none());
    }

    #[test]
    fn test_forbidden_policy() {
        let policy = ToolPolicy::new("rm", ToolPermission::Forbidden);
        assert!(!policy.is_allowed());
        assert!(!policy.needs_confirmation());
    }

    #[test]
    fn test_auto_approve_policy() {
        let policy = ToolPolicy::new("read", ToolPermission::AutoApprove);
        assert!(policy.is_allowed());
        assert!(!policy.needs_confirmation());
    }

    #[test]
    fn test_require_confirmation_policy() {
        let policy = ToolPolicy::new("write", ToolPermission::RequireConfirmation);
        assert!(policy.is_allowed());
        assert!(policy.needs_confirmation());
    }

    #[test]
    fn test_with_sandbox_builder() {
        let policy = ToolPolicy::new("exec", ToolPermission::AutoApprove).with_sandbox(false);
        assert!(!policy.requires_sandbox);
    }

    #[test]
    fn test_with_network_builder() {
        let policy = ToolPolicy::new("curl", ToolPermission::AutoApprove).with_network(true);
        assert!(policy.requires_network);
    }

    #[test]
    fn test_with_timeout_builder() {
        let policy = ToolPolicy::new("build", ToolPermission::AutoApprove).with_timeout(300);
        assert_eq!(policy.max_execution_time, Some(300));
    }

    #[test]
    fn test_builder_chaining() {
        let policy = ToolPolicy::new("deploy", ToolPermission::RequireConfirmation)
            .with_sandbox(true)
            .with_network(true)
            .with_timeout(600);
        assert!(policy.requires_sandbox);
        assert!(policy.requires_network);
        assert_eq!(policy.max_execution_time, Some(600));
        assert!(policy.is_allowed());
        assert!(policy.needs_confirmation());
    }

    #[test]
    fn test_tool_permission_partial_eq() {
        assert_eq!(ToolPermission::AutoApprove, ToolPermission::AutoApprove);
        assert_ne!(ToolPermission::AutoApprove, ToolPermission::Forbidden);
        assert_ne!(
            ToolPermission::RequireConfirmation,
            ToolPermission::Forbidden
        );
    }

    #[test]
    fn test_tool_policy_clone() {
        let policy = ToolPolicy::new("shell", ToolPermission::AutoApprove).with_timeout(60);
        let cloned = policy.clone();
        assert_eq!(cloned.tool_name, "shell");
        assert_eq!(cloned.max_execution_time, Some(60));
    }

    #[test]
    fn test_is_allowed_for_all_permissions() {
        assert!(ToolPolicy::new("a", ToolPermission::AutoApprove).is_allowed());
        assert!(ToolPolicy::new("b", ToolPermission::RequireConfirmation).is_allowed());
        assert!(!ToolPolicy::new("c", ToolPermission::Forbidden).is_allowed());
    }

    #[test]
    fn test_needs_confirmation_only_require_confirmation() {
        assert!(!ToolPolicy::new("a", ToolPermission::AutoApprove).needs_confirmation());
        assert!(ToolPolicy::new("b", ToolPermission::RequireConfirmation).needs_confirmation());
        assert!(!ToolPolicy::new("c", ToolPermission::Forbidden).needs_confirmation());
    }
}
