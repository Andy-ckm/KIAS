use axum::http::StatusCode;

use super::{Claims, Role};

/// Permissions that can be granted to roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read agent information.
    ReadAgents,
    /// Create or update agents.
    WriteAgents,
    /// Delete agents.
    DeleteAgents,
    /// Read node information.
    ReadNodes,
    /// Create or update nodes.
    WriteNodes,
    /// Read knowledge base.
    ReadKnowledge,
    /// Full system administration.
    ManageSystem,
}

/// Return the set of permissions granted to the given role.
///
/// Roles are hierarchical: Admin > Operator > Viewer. Each role inherits
/// all permissions of the roles below it.
pub fn role_permissions(role: Role) -> Vec<Permission> {
    match role {
        Role::Admin => vec![
            Permission::ReadAgents,
            Permission::WriteAgents,
            Permission::DeleteAgents,
            Permission::ReadNodes,
            Permission::WriteNodes,
            Permission::ReadKnowledge,
            Permission::ManageSystem,
        ],
        Role::Operator => vec![
            Permission::ReadAgents,
            Permission::WriteAgents,
            Permission::ReadNodes,
            Permission::WriteNodes,
            Permission::ReadKnowledge,
        ],
        Role::Viewer => vec![
            Permission::ReadAgents,
            Permission::ReadNodes,
            Permission::ReadKnowledge,
        ],
    }
}

/// Check whether `claims` grant the required `permission`.
///
/// Returns `Ok(())` if the permission is granted, or `Err(StatusCode::FORBIDDEN)` otherwise.
pub fn require_permission(claims: &Claims, permission: Permission) -> Result<(), StatusCode> {
    let perms = role_permissions(claims.role);
    if perms.contains(&permission) {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Check whether `claims` grant **all** of the required permissions.
pub fn require_permissions(claims: &Claims, permissions: &[Permission]) -> Result<(), StatusCode> {
    let perms = role_permissions(claims.role);
    for perm in permissions {
        if !perms.contains(perm) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{create_claims, JwtConfig};

    fn test_config() -> JwtConfig {
        JwtConfig::new("test-secret", "kias-test", 24)
    }

    #[test]
    fn test_admin_has_all_permissions() {
        let perms = role_permissions(Role::Admin);
        assert_eq!(perms.len(), 7);
        assert!(perms.contains(&Permission::ManageSystem));
        assert!(perms.contains(&Permission::DeleteAgents));
        assert!(perms.contains(&Permission::WriteNodes));
    }

    #[test]
    fn test_operator_has_limited_permissions() {
        let perms = role_permissions(Role::Operator);
        assert_eq!(perms.len(), 5);
        assert!(perms.contains(&Permission::ReadAgents));
        assert!(perms.contains(&Permission::WriteAgents));
        assert!(!perms.contains(&Permission::DeleteAgents));
        assert!(!perms.contains(&Permission::ManageSystem));
    }

    #[test]
    fn test_viewer_has_read_only_permissions() {
        let perms = role_permissions(Role::Viewer);
        assert_eq!(perms.len(), 3);
        assert!(perms.contains(&Permission::ReadAgents));
        assert!(perms.contains(&Permission::ReadNodes));
        assert!(perms.contains(&Permission::ReadKnowledge));
        assert!(!perms.contains(&Permission::WriteAgents));
        assert!(!perms.contains(&Permission::ManageSystem));
    }

    #[test]
    fn test_require_permission_admin_manage_system() {
        let config = test_config();
        let claims = create_claims("admin", Role::Admin, &config);
        assert!(require_permission(&claims, Permission::ManageSystem).is_ok());
    }

    #[test]
    fn test_require_permission_viewer_manage_system_denied() {
        let config = test_config();
        let claims = create_claims("viewer", Role::Viewer, &config);
        let result = require_permission(&claims, Permission::ManageSystem);
        assert_eq!(result, Err(StatusCode::FORBIDDEN));
    }

    #[test]
    fn test_require_permission_operator_delete_agents_denied() {
        let config = test_config();
        let claims = create_claims("op", Role::Operator, &config);
        let result = require_permission(&claims, Permission::DeleteAgents);
        assert_eq!(result, Err(StatusCode::FORBIDDEN));
    }

    #[test]
    fn test_require_permissions_multiple() {
        let config = test_config();
        let claims = create_claims("admin", Role::Admin, &config);
        assert!(require_permissions(
            &claims,
            &[
                Permission::ReadAgents,
                Permission::WriteAgents,
                Permission::DeleteAgents
            ],
        )
        .is_ok());
    }

    #[test]
    fn test_require_permissions_partial_denied() {
        let config = test_config();
        let claims = create_claims("op", Role::Operator, &config);
        let result =
            require_permissions(&claims, &[Permission::ReadAgents, Permission::ManageSystem]);
        assert_eq!(result, Err(StatusCode::FORBIDDEN));
    }
}
