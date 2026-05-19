//! R031: 用户和权限管理自动化模块
//!
//! 用户CRUD / 组管理 / sudo权限 / 文件权限检查与修复
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: Ansible user module, Puppet user resource, Chef user resource
//! AgentGuard差异化: 用户管理→权限审计→合规检查→自动修复（竞品只做CRUD）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 用户管理引擎
///
/// 提供用户生命周期管理：创建、删除、修改、锁定/解锁
/// 以及用户组管理和文件权限检查/修复
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct UserManager;

impl UserManager {
    /// 执行用户管理操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &UserAction,
        audit: &AuditLog,
    ) -> Result<UserManageResult> {
        let audit_id = uuid::Uuid::new_v4().to_string();
        let mut commands_executed = Vec::new();

        let (action_desc, users, groups, permission_checks, status) = match action {
            UserAction::Create {
                username,
                uid,
                shell,
                home_dir,
                groups: user_groups,
                ssh_key,
            } => {
                let mut cmd = "useradd".to_string();
                if let Some(uid_val) = uid {
                    cmd.push_str(&format!(" -u {}", uid_val));
                }
                if let Some(shell_val) = shell {
                    cmd.push_str(&format!(" -s {}", shell_val));
                }
                if let Some(home_val) = home_dir {
                    cmd.push_str(&format!(" -d {}", home_val));
                }
                if !user_groups.is_empty() {
                    cmd.push_str(&format!(" -G {}", user_groups.join(",")));
                }
                cmd.push_str(&format!(" -m {}", username));

                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("创建用户失败: {}", e)))?;

                // 设置 SSH key
                if let Some(key) = ssh_key {
                    let ssh_cmd = format!(
                        "mkdir -p /home/{}/.ssh && echo '{}' >> /home/{}/.ssh/authorized_keys && chmod 600 /home/{}/.ssh/authorized_keys && chown -R {}:{} /home/{}/.ssh",
                        username, key, username, username, username, username, username
                    );
                    commands_executed.push(ssh_cmd.clone());
                    let _ = executor
                        .execute_command(&[host.to_string()], &ssh_cmd)
                        .await;
                }

                let user_info = Self::get_user_info(executor, host, username).await?;
                (
                    format!("创建用户: {}", username),
                    vec![user_info],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::Delete {
                username,
                remove_home,
            } => {
                let flag = if *remove_home { " -rf" } else { "" };
                let cmd = format!("userdel{} {}", flag, username);
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("删除用户失败: {}", e)))?;

                (
                    format!("删除用户: {} (remove_home={})", username, remove_home),
                    vec![],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::Modify {
                username,
                new_shell,
                new_home,
                add_groups,
                remove_groups,
                lock,
            } => {
                let mut changes = Vec::new();

                if let Some(shell) = new_shell {
                    let cmd = format!("usermod -s {} {}", shell, username);
                    commands_executed.push(cmd.clone());
                    let _ = executor.execute_command(&[host.to_string()], &cmd).await;
                    changes.push(format!("shell→{}", shell));
                }

                if let Some(home) = new_home {
                    let cmd = format!("usermod -d {} -m {}", home, username);
                    commands_executed.push(cmd.clone());
                    let _ = executor.execute_command(&[host.to_string()], &cmd).await;
                    changes.push(format!("home→{}", home));
                }

                if !add_groups.is_empty() {
                    let cmd = format!("usermod -a -G {} {}", add_groups.join(","), username);
                    commands_executed.push(cmd.clone());
                    let _ = executor.execute_command(&[host.to_string()], &cmd).await;
                    changes.push(format!("+groups:{:?}", add_groups));
                }

                if !remove_groups.is_empty() {
                    for group in remove_groups {
                        let cmd = format!("gpasswd -d {} {}", username, group);
                        commands_executed.push(cmd.clone());
                        let _ = executor.execute_command(&[host.to_string()], &cmd).await;
                    }
                    changes.push(format!("-groups:{:?}", remove_groups));
                }

                if let Some(should_lock) = lock {
                    let cmd = if *should_lock {
                        format!("passwd -l {}", username)
                    } else {
                        format!("passwd -u {}", username)
                    };
                    commands_executed.push(cmd.clone());
                    let _ = executor.execute_command(&[host.to_string()], &cmd).await;
                    changes.push(format!("lock={}", should_lock));
                }

                let user_info = Self::get_user_info(executor, host, username).await?;
                (
                    format!("修改用户: {} ({})", username, changes.join(", ")),
                    vec![user_info],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::List { system_users } => {
                let cmd =
                    "awk -F: '{print $1\":\"$3\":\"$4\":\"$6\":\"$7}' /etc/passwd".to_string();
                commands_executed.push(cmd.clone());
                let result = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("列出用户失败: {}", e)))?;

                let output = result
                    .host_results
                    .first()
                    .map(|h| h.stdout.clone())
                    .unwrap_or_default();
                let users = Self::parse_user_list(&output, *system_users);
                (
                    format!("列出用户 (system={})", system_users),
                    users,
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::Check { username } => {
                let user_info = Self::get_user_info(executor, host, username).await?;
                (
                    format!("检查用户: {}", username),
                    vec![user_info],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::Lock { username } => {
                let cmd = format!("passwd -l {}", username);
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("锁定用户失败: {}", e)))?;

                let user_info = Self::get_user_info(executor, host, username).await?;
                (
                    format!("锁定用户: {}", username),
                    vec![user_info],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::Unlock { username } => {
                let cmd = format!("passwd -u {}", username);
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("解锁用户失败: {}", e)))?;

                let user_info = Self::get_user_info(executor, host, username).await?;
                (
                    format!("解锁用户: {}", username),
                    vec![user_info],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::CreateGroup { groupname, gid } => {
                let mut cmd = "groupadd".to_string();
                if let Some(gid_val) = gid {
                    cmd.push_str(&format!(" -g {}", gid_val));
                }
                cmd.push_str(&format!(" {}", groupname));
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| {
                        AutomationError::UserManagement(format!("创建用户组失败: {}", e))
                    })?;

                (
                    format!("创建用户组: {}", groupname),
                    vec![],
                    vec![GroupInfo {
                        name: groupname.clone(),
                        gid: gid.unwrap_or(0),
                        members: vec![],
                    }],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::DeleteGroup { groupname } => {
                let cmd = format!("groupdel {}", groupname);
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| {
                        AutomationError::UserManagement(format!("删除用户组失败: {}", e))
                    })?;

                (
                    format!("删除用户组: {}", groupname),
                    vec![],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }

            UserAction::SudoManage {
                username,
                rules,
                remove,
            } => {
                let sudoers_line = if rules.is_empty() {
                    format!("{} ALL=(ALL) ALL", username)
                } else {
                    rules.join("\n")
                };

                let cmd = if *remove {
                    format!(
                        "sed -i '/^{}/d' /etc/sudoers.d/{} 2>/dev/null; rm -f /etc/sudoers.d/{}",
                        username, username, username
                    )
                } else {
                    format!(
                        "echo '{}' > /etc/sudoers.d/{} && chmod 440 /etc/sudoers.d/{}",
                        sudoers_line, username, username
                    )
                };
                commands_executed.push(cmd.clone());
                let _output = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| {
                        AutomationError::UserManagement(format!("管理sudo权限失败: {}", e))
                    })?;

                let action_desc = if *remove {
                    format!("移除sudo权限: {}", username)
                } else {
                    format!("设置sudo权限: {}", username)
                };
                (action_desc, vec![], vec![], vec![], TaskStatus::Success)
            }

            UserAction::CheckPermissions {
                path,
                expected_owner,
                expected_mode,
            } => {
                let cmd = format!("stat -c '%U:%G:%a' {}", path);
                commands_executed.push(cmd.clone());
                let result = executor
                    .execute_command(&[host.to_string()], &cmd)
                    .await
                    .map_err(|e| AutomationError::UserManagement(format!("检查权限失败: {}", e)))?;

                let output_str = result
                    .host_results
                    .first()
                    .map(|h| h.stdout.clone())
                    .unwrap_or_default();
                let parts: Vec<&str> = output_str.trim().split(':').collect();
                let (owner, group, mode) = if parts.len() >= 3 {
                    (
                        parts[0].to_string(),
                        parts[1].to_string(),
                        parts[2].to_string(),
                    )
                } else {
                    (
                        "unknown".to_string(),
                        "unknown".to_string(),
                        "000".to_string(),
                    )
                };

                let mut issues = Vec::new();
                let owner_compliant = owner == *expected_owner;
                let mode_compliant = mode == *expected_mode;
                if !owner_compliant {
                    issues.push(format!("owner: {} (expected {})", owner, expected_owner));
                }
                if !mode_compliant {
                    issues.push(format!("mode: {} (expected {})", mode, expected_mode));
                }

                let check_result = PermissionCheckResult {
                    path: path.clone(),
                    owner,
                    group,
                    mode,
                    expected_owner: Some(expected_owner.clone()),
                    expected_mode: Some(expected_mode.clone()),
                    compliant: owner_compliant && mode_compliant,
                    issues,
                };

                (
                    format!("检查权限: {}", path),
                    vec![],
                    vec![],
                    vec![check_result],
                    TaskStatus::Success,
                )
            }

            UserAction::FixPermissions {
                path,
                owner,
                mode,
                recursive,
            } => {
                let recursive_flag = if *recursive { " -R" } else { "" };
                let chown_cmd = format!("chown{} {} {}", recursive_flag, owner, path);
                let chmod_cmd = format!("chmod{} {} {}", recursive_flag, mode, path);
                commands_executed.push(chown_cmd.clone());
                commands_executed.push(chmod_cmd.clone());

                let _ = executor
                    .execute_command(&[host.to_string()], &chown_cmd)
                    .await;
                let _ = executor
                    .execute_command(&[host.to_string()], &chmod_cmd)
                    .await;

                (
                    format!("修复权限: {} ({}:{})", path, owner, mode),
                    vec![],
                    vec![],
                    vec![],
                    TaskStatus::Success,
                )
            }
        };

        // 审计日志
        info!(
            target: "user_management",
            host = host,
            action = action_desc,
            audit_id = audit_id,
            commands = ?commands_executed,
            "用户管理操作完成"
        );

        let _ = audit.log_action(
            "system",
            "UserManagement",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&commands_executed).unwrap_or_default(),
        );

        Ok(UserManageResult {
            action: action_desc,
            host: host.to_string(),
            status,
            message: format!("操作成功，执行了 {} 条命令", commands_executed.len()),
            users,
            groups,
            permission_checks,
            commands_executed,
            audit_id,
        })
    }

    /// 获取用户信息
    async fn get_user_info(
        executor: &TaskExecutor,
        host: &str,
        username: &str,
    ) -> Result<UserInfo> {
        let cmd = format!(
            "getent passwd {} | awk -F: '{{print $1\":\"$3\":\"$4\":\"$6\":\"$7}}'",
            username
        );
        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::UserManagement(format!("获取用户信息失败: {}", e)))?;

        let output_str = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();
        let parts: Vec<&str> = output_str.trim().split(':').collect();
        if parts.len() < 5 {
            return Err(AutomationError::UserManagement(format!(
                "用户不存在或格式错误: {}",
                username
            )));
        }

        let uid: u32 = parts[1].parse().unwrap_or(0);
        let gid: u32 = parts[2].parse().unwrap_or(0);

        // 获取用户组
        let groups_cmd = format!("id -nG {} 2>/dev/null", username);
        let groups_result = executor
            .execute_command(&[host.to_string()], &groups_cmd)
            .await;
        let groups = groups_result
            .ok()
            .and_then(|r| r.host_results.first().map(|h| h.stdout.clone()))
            .map(|s| s.split_whitespace().map(|w| w.to_string()).collect())
            .unwrap_or_default();

        // 检查是否锁定
        let lock_cmd = format!("passwd -S {} 2>/dev/null", username);
        let lock_result = executor
            .execute_command(&[host.to_string()], &lock_cmd)
            .await;
        let locked = lock_result
            .ok()
            .and_then(|r| r.host_results.first().map(|h| h.stdout.clone()))
            .map(|s| s.contains(" L "))
            .unwrap_or(false);

        // 获取最后登录
        let last_cmd = format!("lastlog -u {} 2>/dev/null | tail -1", username);
        let last_result = executor
            .execute_command(&[host.to_string()], &last_cmd)
            .await;
        let last_login = last_result
            .ok()
            .and_then(|r| r.host_results.first().map(|h| h.stdout.clone()))
            .and_then(|s| {
                if s.contains("Never") || s.trim().is_empty() {
                    None
                } else {
                    Some(s.trim().to_string())
                }
            });

        Ok(UserInfo {
            username: parts[0].to_string(),
            uid,
            gid,
            home_dir: parts[3].to_string(),
            shell: parts[4].to_string(),
            groups,
            locked,
            last_login,
            comment: String::new(),
        })
    }

    /// 解析用户列表输出
    fn parse_user_list(output: &str, include_system: bool) -> Vec<UserInfo> {
        output
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.trim().split(':').collect();
                if parts.len() < 5 {
                    return None;
                }
                let uid: u32 = parts[1].parse().unwrap_or(0);
                // 系统用户 UID < 1000
                if !include_system && uid < 1000 {
                    return None;
                }
                Some(UserInfo {
                    username: parts[0].to_string(),
                    uid,
                    gid: parts[2].parse().unwrap_or(0),
                    home_dir: parts[3].to_string(),
                    shell: parts[4].to_string(),
                    groups: vec![],
                    locked: false,
                    last_login: None,
                    comment: String::new(),
                })
            })
            .collect()
    }

    /// 生成用户管理 SSH 命令（用于预览/审计）
    pub fn preview_commands(action: &UserAction) -> Vec<String> {
        match action {
            UserAction::Create {
                username,
                uid,
                shell,
                home_dir,
                groups,
                ssh_key,
            } => {
                let mut cmds = vec![format!("useradd {}", username)];
                if let Some(uid_val) = uid {
                    cmds[0] = format!("useradd -u {} {}", uid_val, username);
                }
                if let Some(shell_val) = shell {
                    cmds[0].push_str(&format!(" -s {}", shell_val));
                }
                if let Some(home_val) = home_dir {
                    cmds[0].push_str(&format!(" -d {}", home_val));
                }
                if !groups.is_empty() {
                    cmds[0].push_str(&format!(" -G {}", groups.join(",")));
                }
                cmds[0].push_str(" -m");
                if ssh_key.is_some() {
                    cmds.push(format!("setup SSH key for {}", username));
                }
                cmds
            }
            UserAction::Delete {
                username,
                remove_home,
            } => {
                let flag = if *remove_home { " -rf" } else { "" };
                vec![format!("userdel{} {}", flag, username)]
            }
            UserAction::Lock { username } => vec![format!("passwd -l {}", username)],
            UserAction::Unlock { username } => vec![format!("passwd -u {}", username)],
            UserAction::CreateGroup { groupname, gid } => {
                let mut cmd = "groupadd".to_string();
                if let Some(gid_val) = gid {
                    cmd.push_str(&format!(" -g {}", gid_val));
                }
                cmd.push_str(&format!(" {}", groupname));
                vec![cmd]
            }
            _ => vec!["(complex operation, see execute())".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === UserAction 构造测试 ===

    #[test]
    fn test_user_action_create() {
        let action = UserAction::Create {
            username: "testuser".to_string(),
            uid: Some(1001),
            shell: Some("/bin/bash".to_string()),
            home_dir: Some("/home/testuser".to_string()),
            groups: vec!["docker".to_string(), "sudo".to_string()],
            ssh_key: Some("ssh-rsa AAAA...".to_string()),
        };
        assert!(matches!(action, UserAction::Create { .. }));
        if let UserAction::Create { username, uid, .. } = action {
            assert_eq!(username, "testuser");
            assert_eq!(uid, Some(1001));
        }
    }

    #[test]
    fn test_user_action_delete() {
        let action = UserAction::Delete {
            username: "olduser".to_string(),
            remove_home: true,
        };
        assert!(matches!(action, UserAction::Delete { .. }));
        if let UserAction::Delete {
            username,
            remove_home,
        } = action
        {
            assert_eq!(username, "olduser");
            assert!(remove_home);
        }
    }

    #[test]
    fn test_user_action_modify() {
        let action = UserAction::Modify {
            username: "moduser".to_string(),
            new_shell: Some("/bin/zsh".to_string()),
            new_home: None,
            add_groups: vec!["wheel".to_string()],
            remove_groups: vec![],
            lock: None,
        };
        assert!(matches!(action, UserAction::Modify { .. }));
    }

    #[test]
    fn test_user_action_list() {
        let action = UserAction::List {
            system_users: false,
        };
        assert!(matches!(action, UserAction::List { .. }));
    }

    #[test]
    fn test_user_action_check() {
        let action = UserAction::Check {
            username: "checkuser".to_string(),
        };
        assert!(matches!(action, UserAction::Check { .. }));
    }

    #[test]
    fn test_user_action_lock() {
        let action = UserAction::Lock {
            username: "lockuser".to_string(),
        };
        assert!(matches!(action, UserAction::Lock { .. }));
    }

    #[test]
    fn test_user_action_unlock() {
        let action = UserAction::Unlock {
            username: "unlockuser".to_string(),
        };
        assert!(matches!(action, UserAction::Unlock { .. }));
    }

    #[test]
    fn test_user_action_create_group() {
        let action = UserAction::CreateGroup {
            groupname: "devteam".to_string(),
            gid: Some(2000),
        };
        assert!(matches!(action, UserAction::CreateGroup { .. }));
    }

    #[test]
    fn test_user_action_delete_group() {
        let action = UserAction::DeleteGroup {
            groupname: "oldteam".to_string(),
        };
        assert!(matches!(action, UserAction::DeleteGroup { .. }));
    }

    #[test]
    fn test_user_action_sudo_manage() {
        let action = UserAction::SudoManage {
            username: "admin".to_string(),
            rules: vec!["ALL=(ALL) ALL".to_string()],
            remove: false,
        };
        assert!(matches!(action, UserAction::SudoManage { .. }));
    }

    #[test]
    fn test_user_action_check_permissions() {
        let action = UserAction::CheckPermissions {
            path: "/etc/shadow".to_string(),
            expected_owner: "root".to_string(),
            expected_mode: "640".to_string(),
        };
        assert!(matches!(action, UserAction::CheckPermissions { .. }));
    }

    #[test]
    fn test_user_action_fix_permissions() {
        let action = UserAction::FixPermissions {
            path: "/var/log/app".to_string(),
            owner: "appuser:appgroup".to_string(),
            mode: "750".to_string(),
            recursive: true,
        };
        assert!(matches!(action, UserAction::FixPermissions { .. }));
    }

    // === UserInfo 构造测试 ===

    #[test]
    fn test_user_info_construction() {
        let user = UserInfo {
            username: "testuser".to_string(),
            uid: 1001,
            gid: 1001,
            home_dir: "/home/testuser".to_string(),
            shell: "/bin/bash".to_string(),
            groups: vec!["docker".to_string(), "sudo".to_string()],
            locked: false,
            last_login: Some("2026-05-20".to_string()),
            comment: "Test User".to_string(),
        };
        assert_eq!(user.username, "testuser");
        assert_eq!(user.uid, 1001);
        assert_eq!(user.groups.len(), 2);
        assert!(!user.locked);
    }

    #[test]
    fn test_user_info_locked() {
        let user = UserInfo {
            username: "lockeduser".to_string(),
            uid: 1002,
            gid: 1002,
            home_dir: "/home/lockeduser".to_string(),
            shell: "/bin/bash".to_string(),
            groups: vec![],
            locked: true,
            last_login: None,
            comment: String::new(),
        };
        assert!(user.locked);
        assert!(user.last_login.is_none());
    }

    // === GroupInfo 构造测试 ===

    #[test]
    fn test_group_info_construction() {
        let group = GroupInfo {
            name: "devteam".to_string(),
            gid: 2000,
            members: vec!["alice".to_string(), "bob".to_string()],
        };
        assert_eq!(group.name, "devteam");
        assert_eq!(group.gid, 2000);
        assert_eq!(group.members.len(), 2);
    }

    // === UserManageResult 构造测试 ===

    #[test]
    fn test_user_manage_result_construction() {
        let result = UserManageResult {
            action: "创建用户: testuser".to_string(),
            host: "server1".to_string(),
            status: TaskStatus::Success,
            message: "操作成功".to_string(),
            users: vec![],
            groups: vec![],
            permission_checks: vec![],
            commands_executed: vec!["useradd testuser".to_string()],
            audit_id: "uuid-123".to_string(),
        };
        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.commands_executed.len(), 1);
    }

    // === PermissionCheckResult 测试 ===

    #[test]
    fn test_permission_check_compliant() {
        let check = PermissionCheckResult {
            path: "/etc/shadow".to_string(),
            owner: "root".to_string(),
            group: "shadow".to_string(),
            mode: "640".to_string(),
            expected_owner: Some("root".to_string()),
            expected_mode: Some("640".to_string()),
            compliant: true,
            issues: vec![],
        };
        assert!(check.compliant);
        assert!(check.issues.is_empty());
    }

    #[test]
    fn test_permission_check_non_compliant() {
        let check = PermissionCheckResult {
            path: "/etc/passwd".to_string(),
            owner: "nobody".to_string(),
            group: "nogroup".to_string(),
            mode: "777".to_string(),
            expected_owner: Some("root".to_string()),
            expected_mode: Some("644".to_string()),
            compliant: false,
            issues: vec![
                "owner: nobody (expected root)".to_string(),
                "mode: 777 (expected 644)".to_string(),
            ],
        };
        assert!(!check.compliant);
        assert_eq!(check.issues.len(), 2);
    }

    // === parse_user_list 测试 ===

    #[test]
    fn test_parse_user_list_normal_users() {
        let output = "root:0:0:/root:/bin/bash\nalice:1000:1000:/home/alice:/bin/bash\nbob:1001:1001:/home/bob:/bin/zsh";
        let users = UserManager::parse_user_list(output, false);
        // root (uid=0) is system user, filtered out
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].username, "alice");
        assert_eq!(users[1].username, "bob");
    }

    #[test]
    fn test_parse_user_list_with_system_users() {
        let output = "root:0:0:/root:/bin/bash\nalice:1000:1000:/home/alice:/bin/bash";
        let users = UserManager::parse_user_list(output, true);
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].username, "root");
    }

    #[test]
    fn test_parse_user_list_empty() {
        let users = UserManager::parse_user_list("", false);
        assert!(users.is_empty());
    }

    #[test]
    fn test_parse_user_list_malformed() {
        let output = "invalid_line\nalso:bad\nok:1000:1000:/home/ok:/bin/bash";
        let users = UserManager::parse_user_list(output, false);
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].username, "ok");
    }

    // === preview_commands 测试 ===

    #[test]
    fn test_preview_create_user() {
        let action = UserAction::Create {
            username: "newuser".to_string(),
            uid: Some(1001),
            shell: Some("/bin/bash".to_string()),
            home_dir: None,
            groups: vec!["docker".to_string()],
            ssh_key: Some("ssh-rsa AAAA...".to_string()),
        };
        let cmds = UserManager::preview_commands(&action);
        assert!(!cmds.is_empty());
        assert!(cmds[0].contains("useradd"));
        assert!(cmds[0].contains("newuser"));
        assert!(cmds[0].contains("1001"));
        assert!(cmds[0].contains("/bin/bash"));
        assert!(cmds[0].contains("docker"));
    }

    #[test]
    fn test_preview_delete_user() {
        let action = UserAction::Delete {
            username: "olduser".to_string(),
            remove_home: true,
        };
        let cmds = UserManager::preview_commands(&action);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("userdel"));
        assert!(cmds[0].contains("-rf"));
        assert!(cmds[0].contains("olduser"));
    }

    #[test]
    fn test_preview_lock_user() {
        let action = UserAction::Lock {
            username: "baduser".to_string(),
        };
        let cmds = UserManager::preview_commands(&action);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("passwd -l"));
    }

    #[test]
    fn test_preview_unlock_user() {
        let action = UserAction::Unlock {
            username: "gooduser".to_string(),
        };
        let cmds = UserManager::preview_commands(&action);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("passwd -u"));
    }

    #[test]
    fn test_preview_create_group() {
        let action = UserAction::CreateGroup {
            groupname: "newgroup".to_string(),
            gid: Some(3000),
        };
        let cmds = UserManager::preview_commands(&action);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("groupadd"));
        assert!(cmds[0].contains("3000"));
    }

    // === UserAction 序列化测试 ===

    #[test]
    fn test_user_action_serialize_roundtrip() {
        let action = UserAction::Create {
            username: "test".to_string(),
            uid: Some(1001),
            shell: None,
            home_dir: None,
            groups: vec![],
            ssh_key: None,
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: UserAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_user_info_serialize_roundtrip() {
        let user = UserInfo {
            username: "test".to_string(),
            uid: 1001,
            gid: 1001,
            home_dir: "/home/test".to_string(),
            shell: "/bin/bash".to_string(),
            groups: vec!["docker".to_string()],
            locked: false,
            last_login: None,
            comment: String::new(),
        };
        let json = serde_json::to_string(&user).unwrap();
        let deserialized: UserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(user, deserialized);
    }

    #[test]
    fn test_group_info_serialize_roundtrip() {
        let group = GroupInfo {
            name: "testgroup".to_string(),
            gid: 2000,
            members: vec!["user1".to_string()],
        };
        let json = serde_json::to_string(&group).unwrap();
        let deserialized: GroupInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(group, deserialized);
    }

    // === TaskType::UserManage 测试 ===

    #[test]
    fn test_task_type_user_manage() {
        let task = TaskType::UserManage {
            hosts: vec!["server1".to_string()],
            action: UserAction::List {
                system_users: false,
            },
        };
        assert!(matches!(task, TaskType::UserManage { .. }));
    }

    #[test]
    fn test_task_type_user_manage_create() {
        let task = TaskType::UserManage {
            hosts: vec!["h1".to_string(), "h2".to_string()],
            action: UserAction::Create {
                username: "deploy".to_string(),
                uid: None,
                shell: Some("/bin/bash".to_string()),
                home_dir: None,
                groups: vec!["docker".to_string()],
                ssh_key: None,
            },
        };
        if let TaskType::UserManage { hosts, action } = task {
            assert_eq!(hosts.len(), 2);
            assert!(matches!(action, UserAction::Create { .. }));
        } else {
            panic!("Expected UserManage");
        }
    }

    // === 边界情况测试 ===

    #[test]
    fn test_user_action_create_no_optional() {
        let action = UserAction::Create {
            username: "minimal".to_string(),
            uid: None,
            shell: None,
            home_dir: None,
            groups: vec![],
            ssh_key: None,
        };
        if let UserAction::Create { uid, shell, .. } = action {
            assert!(uid.is_none());
            assert!(shell.is_none());
        }
    }

    #[test]
    fn test_user_action_modify_all_none() {
        let action = UserAction::Modify {
            username: "noop".to_string(),
            new_shell: None,
            new_home: None,
            add_groups: vec![],
            remove_groups: vec![],
            lock: None,
        };
        assert!(matches!(action, UserAction::Modify { .. }));
    }

    #[test]
    fn test_permission_check_empty_issues() {
        let check = PermissionCheckResult {
            path: "/tmp".to_string(),
            owner: "root".to_string(),
            group: "root".to_string(),
            mode: "1777".to_string(),
            expected_owner: None,
            expected_mode: None,
            compliant: true,
            issues: vec![],
        };
        assert!(check.compliant);
        assert!(check.expected_owner.is_none());
    }

    #[test]
    fn test_user_manage_result_debug() {
        let result = UserManageResult {
            action: "test".to_string(),
            host: "h1".to_string(),
            status: TaskStatus::Success,
            message: "ok".to_string(),
            users: vec![],
            groups: vec![],
            permission_checks: vec![],
            commands_executed: vec![],
            audit_id: "id".to_string(),
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("UserManageResult"));
    }

    #[test]
    fn test_user_info_debug() {
        let user = UserInfo {
            username: "debug_user".to_string(),
            uid: 1001,
            gid: 1001,
            home_dir: "/home/debug".to_string(),
            shell: "/bin/bash".to_string(),
            groups: vec![],
            locked: false,
            last_login: None,
            comment: String::new(),
        };
        let debug = format!("{:?}", user);
        assert!(debug.contains("debug_user"));
    }

    #[test]
    fn test_group_info_debug() {
        let group = GroupInfo {
            name: "debug_group".to_string(),
            gid: 3000,
            members: vec![],
        };
        let debug = format!("{:?}", group);
        assert!(debug.contains("debug_group"));
    }
}
