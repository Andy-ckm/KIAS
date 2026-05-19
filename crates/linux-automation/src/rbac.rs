//! RBAC 权限控制模块
//!
//! 基于 casbin-rs 实现细粒度权限控制
//! 参考：/mnt/reference-projects/casbin-rs/

use crate::error::{AutomationError, Result};
use casbin::{CoreApi, DefaultModel, Enforcer, FileAdapter};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// RBAC 权限管理器
pub struct RbacManager {
    enforcer: Arc<RwLock<Enforcer>>,
}

/// 权限检查结果
#[derive(Debug, Clone)]
pub struct PermissionCheck {
    pub allowed: bool,
    pub user: String,
    pub resource: String,
    pub action: String,
}

impl RbacManager {
    /// 创建新的 RBAC 管理器
    pub async fn new(model_path: &Path, policy_path: &Path) -> Result<Self> {
        let model = DefaultModel::from_file(model_path)
            .await
            .map_err(|e| AutomationError::Config(format!("加载 RBAC 模型失败: {}", e)))?;

        let adapter = FileAdapter::new(policy_path);

        let enforcer = Enforcer::new(model, adapter)
            .await
            .map_err(|e| AutomationError::Config(format!("创建 RBAC 引擎失败: {}", e)))?;

        Ok(Self {
            enforcer: Arc::new(RwLock::new(enforcer)),
        })
    }

    /// 检查权限
    pub async fn check_permission(
        &self,
        user: &str,
        resource: &str,
        action: &str,
    ) -> Result<PermissionCheck> {
        let enforcer = self.enforcer.read().await;

        let allowed = enforcer
            .enforce((user, resource, action))
            .map_err(|e| AutomationError::PermissionDenied(format!("权限检查失败: {}", e)))?;

        Ok(PermissionCheck {
            allowed,
            user: user.to_string(),
            resource: resource.to_string(),
            action: action.to_string(),
        })
    }

    /// 添加角色
    pub async fn add_role_for_user(&self, user: &str, role: &str) -> Result<()> {
        let mut enforcer = self.enforcer.write().await;

        enforcer
            .add_role_for_user(user, role, None)
            .await
            .map_err(|e| AutomationError::Other(format!("添加角色失败: {}", e)))?;

        Ok(())
    }

    /// 删除角色
    pub async fn delete_role_for_user(&self, user: &str, role: &str) -> Result<()> {
        let mut enforcer = self.enforcer.write().await;

        enforcer
            .delete_role_for_user(user, role, None)
            .await
            .map_err(|e| AutomationError::Other(format!("删除角色失败: {}", e)))?;

        Ok(())
    }

    /// 获取用户角色
    pub async fn get_roles_for_user(&self, user: &str) -> Result<Vec<String>> {
        let enforcer = self.enforcer.read().await;

        let roles = enforcer
            .get_roles_for_user(user, None)
            .await
            .map_err(|e| AutomationError::Other(format!("获取角色失败: {}", e)))?;

        Ok(roles)
    }

    /// 添加权限
    pub async fn add_permission_for_user(
        &self,
        user: &str,
        permission: Vec<String>,
    ) -> Result<()> {
        let mut enforcer = self.enforcer.write().await;

        enforcer
            .add_permission_for_user(user, permission)
            .await
            .map_err(|e| AutomationError::Other(format!("添加权限失败: {}", e)))?;

        Ok(())
    }
}

/// RBAC 模型配置（基于 casbin-rs）
pub fn default_rbac_model() -> String {
    r#"
[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act

[role_definition]
g = _, _

[policy_effect]
e = some(where (p.eft == allow))

[matchers]
m = g(r.sub, p.sub) && r.obj == p.obj && r.act == p.act
"#
    .to_string()
}

/// 默认策略
pub fn default_policy() -> String {
    r#"
p, admin, server, read
p, admin, server, write
p, admin, server, execute
p, admin, compliance, read
p, admin, compliance, write
p, admin, audit, read

p, operator, server, read
p, operator, server, execute
p, operator, compliance, read

p, viewer, server, read
p, viewer, compliance, read
p, viewer, audit, read

g, alice, admin
g, bob, operator
g, charlie, viewer
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rbac_model_creation() {
        let model = default_rbac_model();
        assert!(model.contains("[request_definition]"));
        assert!(model.contains("[role_definition]"));
    }

    #[tokio::test]
    async fn test_policy_creation() {
        let policy = default_policy();
        assert!(policy.contains("p, admin, server, read"));
        assert!(policy.contains("g, alice, admin"));
    }
}
