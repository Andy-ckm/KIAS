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
    "[request_definition]\\nr = sub, obj, act\\n\\n[policy_definition]\\np = sub, obj, act\\n\\n[role_definition]\\ng = _, _\\n\\n[policy_effect]\\ne = some(where (p.eft == allow))\\n\\n[matchers]\\nm = g(r.sub, p.sub) && r.obj == p.obj && r.act == p.act\\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_model_file(dir: &std::path::Path) -> std::path::PathBuf {
        let model_path = dir.join("model.conf");
        let content = r#"[request_definition]
r = sub, obj, act

[policy_definition]
p = sub, obj, act

[role_definition]
g = _, _

[policy_effect]
e = some(where (p.eft == allow))

[matchers]
m = g(r.sub, p.sub) && r.obj == p.obj && r.act == p.act
"#;
        std::fs::write(&model_path, content).unwrap();
        model_path
    }

    fn create_test_policy_file(dir: &std::path::Path) -> std::path::PathBuf {
        let policy_path = dir.join("policy.csv");
        let content = "p, admin, server1, reboot\np, admin, server1, shutdown\np, operator, server1, reboot\ng, alice, admin\ng, bob, operator\n";
        std::fs::write(&policy_path, content).unwrap();
        policy_path
    }

    // ============================================================
    // rbac_model() tests
    // ============================================================

    #[test]
    fn test_rbac_model_not_empty() {
        let model = rbac_model();
        assert!(!model.is_empty());
    }

    #[test]
    fn test_rbac_model_contains_sections() {
        let model = rbac_model();
        assert!(model.contains("[request_definition]"));
        assert!(model.contains("[policy_definition]"));
        assert!(model.contains("[role_definition]"));
        assert!(model.contains("[policy_effect]"));
        assert!(model.contains("[matchers]"));
    }

    #[test]
    fn test_rbac_model_contains_required_fields() {
        let model = rbac_model();
        assert!(model.contains("r = sub, obj, act"));
        assert!(model.contains("p = sub, obj, act"));
        assert!(model.contains("g = _, _"));
    }

    #[test]
    fn test_rbac_model_contains_matcher() {
        let model = rbac_model();
        assert!(model.contains("g(r.sub, p.sub)"));
        assert!(model.contains("r.obj == p.obj"));
        assert!(model.contains("r.act == p.act"));
    }

    #[test]
    fn test_rbac_model_contains_effect() {
        let model = rbac_model();
        assert!(model.contains("some(where (p.eft == allow))"));
    }

    #[test]
    fn test_rbac_model_has_five_sections() {
        let model = rbac_model();
        let section_count = model.matches('[').count();
        assert_eq!(section_count, 5);
    }

    // ============================================================
    // PermissionCheck tests
    // ============================================================

    #[test]
    fn test_permission_check_fields() {
        let check = PermissionCheck {
            allowed: true,
            user: "admin".to_string(),
            resource: "server1".to_string(),
            action: "reboot".to_string(),
        };
        assert!(check.allowed);
        assert_eq!(check.user, "admin");
        assert_eq!(check.resource, "server1");
        assert_eq!(check.action, "reboot");
    }

    #[test]
    fn test_permission_check_clone() {
        let check = PermissionCheck {
            allowed: true,
            user: "admin".to_string(),
            resource: "server1".to_string(),
            action: "reboot".to_string(),
        };
        let cloned = check.clone();
        assert_eq!(cloned.allowed, check.allowed);
        assert_eq!(cloned.user, check.user);
        assert_eq!(cloned.resource, check.resource);
        assert_eq!(cloned.action, check.action);
    }

    #[test]
    fn test_permission_check_denied() {
        let check = PermissionCheck {
            allowed: false,
            user: "guest".to_string(),
            resource: "server1".to_string(),
            action: "reboot".to_string(),
        };
        assert!(!check.allowed);
        assert_eq!(check.user, "guest");
    }

    #[test]
    fn test_permission_check_debug() {
        let check = PermissionCheck {
            allowed: true,
            user: "admin".to_string(),
            resource: "server1".to_string(),
            action: "reboot".to_string(),
        };
        let debug = format!("{:?}", check);
        assert!(debug.contains("PermissionCheck"));
        assert!(debug.contains("admin"));
        assert!(debug.contains("server1"));
        assert!(debug.contains("reboot"));
    }

    #[test]
    fn test_permission_check_clone_independence() {
        let check = PermissionCheck {
            allowed: true,
            user: "admin".to_string(),
            resource: "server1".to_string(),
            action: "reboot".to_string(),
        };
        let mut cloned = check.clone();
        cloned.user = "modified".to_string();
        assert_eq!(check.user, "admin");
        assert_eq!(cloned.user, "modified");
    }

    #[test]
    fn test_permission_check_allowed_false() {
        let check = PermissionCheck {
            allowed: false,
            user: "guest".to_string(),
            resource: "db".to_string(),
            action: "write".to_string(),
        };
        assert!(!check.allowed);
        assert_eq!(check.resource, "db");
        assert_eq!(check.action, "write");
    }

    // ============================================================
    // RbacManager integration tests (with temp files)
    // ============================================================

    #[tokio::test]
    async fn test_rbac_manager_new_with_valid_files() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await;
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_rbac_manager_check_permission_admin_allowed() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        // alice has role admin, admin can reboot server1
        let result = manager
            .check_permission("alice", "server1", "reboot")
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.user, "alice");
        assert_eq!(result.resource, "server1");
        assert_eq!(result.action, "reboot");
    }

    #[tokio::test]
    async fn test_rbac_manager_check_permission_denied() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        // bob has role operator, operator cannot shutdown server1
        let result = manager
            .check_permission("bob", "server1", "shutdown")
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_rbac_manager_check_permission_unknown_user() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        let result = manager
            .check_permission("unknown", "server1", "reboot")
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_rbac_manager_check_permission_wrong_resource() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        // admin can reboot server1, but not server2
        let result = manager
            .check_permission("alice", "server2", "reboot")
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_rbac_manager_get_roles() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        let roles = manager.get_roles("alice").await.unwrap();
        assert!(roles.contains(&"admin".to_string()));
    }

    #[tokio::test]
    async fn test_rbac_manager_get_roles_empty() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        let roles = manager.get_roles("unknown_user").await.unwrap();
        assert!(roles.is_empty());
    }

    #[tokio::test]
    async fn test_rbac_manager_add_role() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        let result = manager.add_role("charlie", "admin").await;
        assert!(result.is_ok());

        let roles = manager.get_roles("charlie").await.unwrap();
        assert!(roles.contains(&"admin".to_string()));
    }

    #[tokio::test]
    async fn test_rbac_manager_new_missing_model_file() {
        let result = RbacManager::new(
            std::path::Path::new("/nonexistent/model.conf"),
            std::path::Path::new("/nonexistent/policy.csv"),
        )
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rbac_manager_new_invalid_model_content() {
        let tmp = TempDir::new().unwrap();
        let model_path = tmp.path().join("bad_model.conf");
        std::fs::write(&model_path, "this is not a valid casbin model").unwrap();
        let policy_path = tmp.path().join("policy.csv");
        std::fs::write(&policy_path, "").unwrap();

        let result = RbacManager::new(&model_path, &policy_path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rbac_manager_add_role_then_check() {
        let tmp = TempDir::new().unwrap();
        let model_path = create_test_model_file(tmp.path());
        let policy_path = create_test_policy_file(tmp.path());

        let manager = RbacManager::new(&model_path, &policy_path).await.unwrap();
        // dave has no role initially
        let result = manager
            .check_permission("dave", "server1", "reboot")
            .await
            .unwrap();
        assert!(!result.allowed);

        // add operator role to dave
        manager.add_role("dave", "operator").await.unwrap();
        let result = manager
            .check_permission("dave", "server1", "reboot")
            .await
            .unwrap();
        assert!(result.allowed);
    }
}
