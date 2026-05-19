//! RBAC 权限控制模块

use crate::error::{AutomationError, Result};
use casbin::prelude::*;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RbacManager {
    enforcer: Arc<RwLock<Enforcer>>,
}

#[derive(Debug, Clone)]
pub struct PermissionCheck {
    pub allowed: bool,
    pub user: String,
    pub resource: String,
    pub action: String,
}

impl RbacManager {
    pub async fn new(model_path: &Path, policy_path: &Path) -> Result<Self> {
        let m = DefaultModel::from_file(model_path)
            .await
            .map_err(|e| AutomationError::Config(format!("加载RBAC模型失败: {}", e)))?;
        let a = FileAdapter::new(policy_path.to_path_buf());
        let enforcer = Enforcer::new(m, a)
            .await
            .map_err(|e| AutomationError::Config(format!("创建RBAC引擎失败: {}", e)))?;
        Ok(Self {
            enforcer: Arc::new(RwLock::new(enforcer)),
        })
    }

    pub async fn check_permission(
        &self,
        user: &str,
        resource: &str,
        action: &str,
    ) -> Result<PermissionCheck> {
        let e = self.enforcer.read().await;
        let allowed = e
            .enforce((user, resource, action))
            .map_err(|e| AutomationError::PermissionDenied(format!("权限检查失败: {}", e)))?;
        Ok(PermissionCheck {
            allowed,
            user: user.into(),
            resource: resource.into(),
            action: action.into(),
        })
    }

    pub async fn add_role(&self, user: &str, role: &str) -> Result<()> {
        let mut e = self.enforcer.write().await;
        e.add_role_for_user(user, role, None)
            .await
            .map_err(|e| AutomationError::Other(format!("添加角色失败: {}", e)))?;
        Ok(())
    }

    pub async fn get_roles(&self, user: &str) -> Result<Vec<String>> {
        let e = self.enforcer.read().await;
        let roles = e.get_roles_for_user(user, None);
        Ok(roles)
    }
}

pub fn rbac_model() -> &'static str {
    "[request_definition]\nr = sub, obj, act\n\n[policy_definition]\np = sub, obj, act\n\n[role_definition]\ng = _, _\n\n[policy_effect]\ne = some(where (p.eft == allow))\n\n[matchers]\nm = g(r.sub, p.sub) && r.obj == p.obj && r.act == p.act\n"
}
