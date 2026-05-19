//! R033: 服务管理模块 — systemd/nginx/mysql/redis
//!
//! 服务生命周期管理 / 开机自启控制 / 日志查看 / 健康检查
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: Ansible systemd module, Puppet service resource, Chef service resource
//! AgentGuard差异化: 服务管理→依赖分析→健康检查→自动恢复→合规审计（竞品只做启停）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 服务管理引擎
///
/// 提供 systemd/nginx/mysql/redis 的完整服务管理：
/// 启动/停止/重启/状态/自启/日志/列表/失败单元/daemon-reload
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct ServiceManager;

impl ServiceManager {
    /// 执行服务操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &ServiceAction,
        audit: &AuditLog,
    ) -> Result<ServiceOpsResult> {
        let audit_id = uuid::Uuid::new_v4().to_string();
        let mut commands_executed = Vec::new();

        let (action_desc, result) = match action {
            // === systemd 操作 ===
            ServiceAction::Start { service } => {
                let cmd = format!("systemctl start {}", service);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let state = Self::get_unit_state(executor, host, service).await?;
                (
                    format!("启动服务: {}", service),
                    ServiceOpsResult {
                        action: "Start".to_string(),
                        service: service.clone(),
                        success: state.0 == "active",
                        active_state: Some(state.0),
                        sub_state: Some(state.1),
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Stop { service } => {
                let cmd = format!("systemctl stop {}", service);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let state = Self::get_unit_state(executor, host, service).await?;
                (
                    format!("停止服务: {}", service),
                    ServiceOpsResult {
                        action: "Stop".to_string(),
                        service: service.clone(),
                        success: state.0 == "inactive",
                        active_state: Some(state.0),
                        sub_state: Some(state.1),
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Restart { service } => {
                let cmd = format!("systemctl restart {}", service);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let state = Self::get_unit_state(executor, host, service).await?;
                (
                    format!("重启服务: {}", service),
                    ServiceOpsResult {
                        action: "Restart".to_string(),
                        service: service.clone(),
                        success: state.0 == "active",
                        active_state: Some(state.0),
                        sub_state: Some(state.1),
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Status { service } => {
                let cmd = format!(
                    "systemctl show {} --property=ActiveState,SubState,UnitFileState,Description",
                    service
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let (active, sub, enabled, desc) = Self::parse_unit_show(&output);
                (
                    format!("查看服务状态: {}", service),
                    ServiceOpsResult {
                        action: "Status".to_string(),
                        service: service.clone(),
                        success: !active.is_empty(),
                        active_state: Some(active.clone()),
                        sub_state: Some(sub.clone()),
                        enabled: Some(enabled),
                        services: vec![ServiceInfo {
                            name: service.clone(),
                            unit_type: "service".to_string(),
                            active_state: active,
                            sub_state: sub,
                            description: desc,
                            enabled,
                        }],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Enable { service } => {
                let cmd = format!("systemctl enable {}", service);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let is_enabled = Self::is_enabled(executor, host, service).await;
                (
                    format!("启用自启: {}", service),
                    ServiceOpsResult {
                        action: "Enable".to_string(),
                        service: service.clone(),
                        success: is_enabled,
                        active_state: None,
                        sub_state: None,
                        enabled: Some(is_enabled),
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Disable { service } => {
                let cmd = format!("systemctl disable {}", service);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let is_enabled = Self::is_enabled(executor, host, service).await;
                (
                    format!("禁用自启: {}", service),
                    ServiceOpsResult {
                        action: "Disable".to_string(),
                        service: service.clone(),
                        success: !is_enabled,
                        active_state: None,
                        sub_state: None,
                        enabled: Some(is_enabled),
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::Logs { service, lines } => {
                let cmd = format!(
                    "journalctl -u {} --no-pager -n {}",
                    service,
                    (*lines).min(1000)
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("查看日志: {} ({}行)", service, lines),
                    ServiceOpsResult {
                        action: "Logs".to_string(),
                        service: service.clone(),
                        success: true,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::ListUnits { unit_type } => {
                let utype = if unit_type.is_empty() {
                    "service"
                } else {
                    unit_type
                };
                let cmd = format!(
                    "systemctl list-units --type={} --no-pager --no-legend --plain",
                    utype
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let services = Self::parse_list_units(&output);
                let count = services.len();
                (
                    format!("列出{}单元 ({}个)", utype, count),
                    ServiceOpsResult {
                        action: "ListUnits".to_string(),
                        service: unit_type.clone(),
                        success: true,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services,
                        output: format!("共 {} 个单元", count),
                        errors: vec![],
                    },
                )
            }
            ServiceAction::ListFailed {} => {
                let cmd = "systemctl list-units --failed --no-pager --no-legend --plain";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let services = Self::parse_list_units(&output);
                let count = services.len();
                (
                    format!("列出失败单元 ({}个)", count),
                    ServiceOpsResult {
                        action: "ListFailed".to_string(),
                        service: "failed".to_string(),
                        success: count == 0,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services,
                        output: if count == 0 {
                            "没有失败的单元".to_string()
                        } else {
                            format!("{} 个失败的单元", count)
                        },
                        errors: if count > 0 {
                            vec![format!("{} 个单元处于失败状态", count)]
                        } else {
                            vec![]
                        },
                    },
                )
            }
            ServiceAction::DaemonReload {} => {
                let cmd = "systemctl daemon-reload";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                (
                    "重载systemd配置".to_string(),
                    ServiceOpsResult {
                        action: "DaemonReload".to_string(),
                        service: "systemd".to_string(),
                        success: true,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }

            // === nginx 操作 ===
            ServiceAction::NginxReload {} => {
                let cmd = "nginx -t && systemctl reload nginx";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                (
                    "重载nginx配置".to_string(),
                    ServiceOpsResult {
                        action: "NginxReload".to_string(),
                        service: "nginx".to_string(),
                        success: !output.contains("failed"),
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::NginxTest {} => {
                let cmd = "nginx -t 2>&1";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let ok = output.contains("syntax is ok") || output.contains("successful");
                (
                    "测试nginx配置".to_string(),
                    ServiceOpsResult {
                        action: "NginxTest".to_string(),
                        service: "nginx".to_string(),
                        success: ok,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: if !ok {
                            vec!["nginx配置测试失败".to_string()]
                        } else {
                            vec![]
                        },
                    },
                )
            }
            ServiceAction::NginxStatus {} => {
                let cmd = "systemctl is-active nginx && nginx -v 2>&1";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let active = output.trim().lines().next().unwrap_or("");
                (
                    "查看nginx状态".to_string(),
                    ServiceOpsResult {
                        action: "NginxStatus".to_string(),
                        service: "nginx".to_string(),
                        success: active == "active",
                        active_state: Some(active.to_string()),
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }

            // === mysql 操作 ===
            ServiceAction::MysqlStatus {} => {
                let cmd = "systemctl is-active mysql 2>/dev/null || systemctl is-active mysqld 2>/dev/null || systemctl is-active mariadb 2>/dev/null";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let active = output.trim().to_string();
                (
                    "查看mysql状态".to_string(),
                    ServiceOpsResult {
                        action: "MysqlStatus".to_string(),
                        service: "mysql".to_string(),
                        success: active == "active",
                        active_state: Some(active),
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::MysqlQuery { query } => {
                // Safety: only allow SELECT queries for safety
                let trimmed = query.trim().to_uppercase();
                if !trimmed.starts_with("SELECT")
                    && !trimmed.starts_with("SHOW")
                    && !trimmed.starts_with("DESCRIBE")
                    && !trimmed.starts_with("EXPLAIN")
                {
                    return Err(AutomationError::ServiceOperation(
                        "只允许 SELECT/SHOW/DESCRIBE/EXPLAIN 查询".to_string(),
                    ));
                }
                let cmd = format!("mysql -e '{}' 2>&1", query.replace('\'', "'\\''"));
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("执行MySQL查询: {}", &query[..query.len().min(50)]),
                    ServiceOpsResult {
                        action: "MysqlQuery".to_string(),
                        service: "mysql".to_string(),
                        success: !output.contains("ERROR"),
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::MysqlCheckTables { database } => {
                let cmd = format!("mysqlcheck --check {} 2>&1", database);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let has_error = output.contains("error") || output.contains("corrupt");
                (
                    format!("检查MySQL表: {}", database),
                    ServiceOpsResult {
                        action: "MysqlCheckTables".to_string(),
                        service: "mysql".to_string(),
                        success: !has_error,
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: if has_error {
                            vec!["数据库表检查发现问题".to_string()]
                        } else {
                            vec![]
                        },
                    },
                )
            }

            // === redis 操作 ===
            ServiceAction::RedisStatus {} => {
                let cmd = "redis-cli ping 2>&1";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let ok = output.trim() == "PONG";
                (
                    "查看redis状态".to_string(),
                    ServiceOpsResult {
                        action: "RedisStatus".to_string(),
                        service: "redis".to_string(),
                        success: ok,
                        active_state: Some(if ok { "active" } else { "inactive" }.to_string()),
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::RedisInfo {} => {
                let cmd = "redis-cli info 2>&1";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                (
                    "查看redis信息".to_string(),
                    ServiceOpsResult {
                        action: "RedisInfo".to_string(),
                        service: "redis".to_string(),
                        success: !output.contains("Error")
                            && !output.contains("Connection refused"),
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            ServiceAction::RedisCommand { command } => {
                // Safety: block dangerous commands
                let upper = command.trim().to_uppercase();
                if upper.contains("FLUSHALL")
                    || upper.contains("FLUSHDB")
                    || upper.contains("CONFIG SET")
                    || upper.contains("DEBUG ")
                    || upper.contains("SHUTDOWN")
                {
                    return Err(AutomationError::ServiceOperation(format!(
                        "危险的redis命令被拒绝: {}",
                        command
                    )));
                }
                let cmd = format!("redis-cli {} 2>&1", command);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("执行redis命令: {}", &command[..command.len().min(50)]),
                    ServiceOpsResult {
                        action: "RedisCommand".to_string(),
                        service: "redis".to_string(),
                        success: !output.contains("Error")
                            && !output.contains("Connection refused"),
                        active_state: None,
                        sub_state: None,
                        enabled: None,
                        services: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
        };

        // 审计记录
        info!(
            audit_id = %audit_id,
            host = host,
            action = %action_desc,
            success = result.success,
            "服务操作完成"
        );
        let _ = audit.log_action(
            "system",
            "ServiceManager",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&result).unwrap_or_default(),
        );
        commands_executed.iter().for_each(|cmd| {
            info!(host = host, cmd = cmd, "执行命令");
        });

        Ok(result)
    }

    // === 内部辅助方法 ===

    /// 远程执行命令
    async fn run_cmd(executor: &TaskExecutor, host: &str, cmd: &str) -> Result<String> {
        let result = executor
            .execute_command(&[host.to_string()], cmd)
            .await
            .map_err(|e| AutomationError::ServiceOperation(e.to_string()))?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();
        Ok(output)
    }

    /// 获取 systemd 单元状态
    async fn get_unit_state(
        executor: &TaskExecutor,
        host: &str,
        service: &str,
    ) -> Result<(String, String)> {
        let cmd = format!("systemctl show {} --property=ActiveState,SubState", service);
        let output = Self::run_cmd(executor, host, &cmd).await?;
        let mut active = String::new();
        let mut sub = String::new();
        for line in output.lines() {
            if let Some(val) = line.strip_prefix("ActiveState=") {
                active = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("SubState=") {
                sub = val.trim().to_string();
            }
        }
        Ok((active, sub))
    }

    /// 检查服务是否 enabled
    async fn is_enabled(executor: &TaskExecutor, host: &str, service: &str) -> bool {
        let cmd = format!("systemctl is-enabled {} 2>/dev/null", service);
        if let Ok(output) = Self::run_cmd(executor, host, &cmd).await {
            output.trim() == "enabled"
        } else {
            false
        }
    }

    /// 解析 systemctl show 输出
    fn parse_unit_show(output: &str) -> (String, String, bool, String) {
        let mut active = String::new();
        let mut sub = String::new();
        let mut enabled = false;
        let mut desc = String::new();
        for line in output.lines() {
            if let Some(val) = line.strip_prefix("ActiveState=") {
                active = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("SubState=") {
                sub = val.trim().to_string();
            } else if let Some(val) = line.strip_prefix("UnitFileState=") {
                enabled = val.trim() == "enabled";
            } else if let Some(val) = line.strip_prefix("Description=") {
                desc = val.trim().to_string();
            }
        }
        (active, sub, enabled, desc)
    }

    /// 解析 systemctl list-units 输出
    fn parse_list_units(output: &str) -> Vec<ServiceInfo> {
        let mut services = Vec::new();
        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                services.push(ServiceInfo {
                    name: parts[0].to_string(),
                    unit_type: parts[1].to_string(),
                    active_state: parts[2].to_string(),
                    sub_state: parts[3].to_string(),
                    description: parts[4..].join(" "),
                    enabled: false, // list-units 不显示 enabled 状态
                });
            }
        }
        services
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === 模型测试 ===

    #[test]
    fn test_service_action_start() {
        let action = ServiceAction::Start {
            service: "nginx".to_string(),
        };
        assert!(matches!(action, ServiceAction::Start { .. }));
        if let ServiceAction::Start { service } = action {
            assert_eq!(service, "nginx");
        }
    }

    #[test]
    fn test_service_action_stop() {
        let action = ServiceAction::Stop {
            service: "redis".to_string(),
        };
        assert!(matches!(action, ServiceAction::Stop { .. }));
    }

    #[test]
    fn test_service_action_restart() {
        let action = ServiceAction::Restart {
            service: "mysql".to_string(),
        };
        assert!(matches!(action, ServiceAction::Restart { .. }));
    }

    #[test]
    fn test_service_action_status() {
        let action = ServiceAction::Status {
            service: "docker".to_string(),
        };
        assert!(matches!(action, ServiceAction::Status { .. }));
    }

    #[test]
    fn test_service_action_enable() {
        let action = ServiceAction::Enable {
            service: "sshd".to_string(),
        };
        assert!(matches!(action, ServiceAction::Enable { .. }));
    }

    #[test]
    fn test_service_action_disable() {
        let action = ServiceAction::Disable {
            service: "telnet".to_string(),
        };
        assert!(matches!(action, ServiceAction::Disable { .. }));
    }

    #[test]
    fn test_service_action_logs() {
        let action = ServiceAction::Logs {
            service: "nginx".to_string(),
            lines: 100,
        };
        assert!(matches!(action, ServiceAction::Logs { .. }));
        if let ServiceAction::Logs { service, lines } = action {
            assert_eq!(service, "nginx");
            assert_eq!(lines, 100);
        }
    }

    #[test]
    fn test_service_action_list_units() {
        let action = ServiceAction::ListUnits {
            unit_type: "service".to_string(),
        };
        assert!(matches!(action, ServiceAction::ListUnits { .. }));
    }

    #[test]
    fn test_service_action_list_failed() {
        let action = ServiceAction::ListFailed {};
        assert!(matches!(action, ServiceAction::ListFailed {}));
    }

    #[test]
    fn test_service_action_daemon_reload() {
        let action = ServiceAction::DaemonReload {};
        assert!(matches!(action, ServiceAction::DaemonReload {}));
    }

    #[test]
    fn test_service_action_nginx_reload() {
        let action = ServiceAction::NginxReload {};
        assert!(matches!(action, ServiceAction::NginxReload {}));
    }

    #[test]
    fn test_service_action_nginx_test() {
        let action = ServiceAction::NginxTest {};
        assert!(matches!(action, ServiceAction::NginxTest {}));
    }

    #[test]
    fn test_service_action_nginx_status() {
        let action = ServiceAction::NginxStatus {};
        assert!(matches!(action, ServiceAction::NginxStatus {}));
    }

    #[test]
    fn test_service_action_mysql_status() {
        let action = ServiceAction::MysqlStatus {};
        assert!(matches!(action, ServiceAction::MysqlStatus {}));
    }

    #[test]
    fn test_service_action_mysql_query() {
        let action = ServiceAction::MysqlQuery {
            query: "SELECT * FROM users".to_string(),
        };
        assert!(matches!(action, ServiceAction::MysqlQuery { .. }));
    }

    #[test]
    fn test_service_action_mysql_check_tables() {
        let action = ServiceAction::MysqlCheckTables {
            database: "mydb".to_string(),
        };
        assert!(matches!(action, ServiceAction::MysqlCheckTables { .. }));
    }

    #[test]
    fn test_service_action_redis_status() {
        let action = ServiceAction::RedisStatus {};
        assert!(matches!(action, ServiceAction::RedisStatus {}));
    }

    #[test]
    fn test_service_action_redis_info() {
        let action = ServiceAction::RedisInfo {};
        assert!(matches!(action, ServiceAction::RedisInfo {}));
    }

    #[test]
    fn test_service_action_redis_command() {
        let action = ServiceAction::RedisCommand {
            command: "GET mykey".to_string(),
        };
        assert!(matches!(action, ServiceAction::RedisCommand { .. }));
    }

    // === ServiceInfo 测试 ===

    #[test]
    fn test_service_info_construction() {
        let info = ServiceInfo {
            name: "nginx.service".to_string(),
            unit_type: "service".to_string(),
            active_state: "active".to_string(),
            sub_state: "running".to_string(),
            description: "A high performance web server".to_string(),
            enabled: true,
        };
        assert_eq!(info.name, "nginx.service");
        assert_eq!(info.active_state, "active");
        assert!(info.enabled);
    }

    #[test]
    fn test_service_info_clone() {
        let info = ServiceInfo {
            name: "redis".to_string(),
            unit_type: "service".to_string(),
            active_state: "active".to_string(),
            sub_state: "running".to_string(),
            description: "Redis".to_string(),
            enabled: false,
        };
        let cloned = info.clone();
        assert_eq!(info, cloned);
    }

    // === ServiceOpsResult 测试 ===

    #[test]
    fn test_service_ops_result_success() {
        let result = ServiceOpsResult {
            action: "Start".to_string(),
            service: "nginx".to_string(),
            success: true,
            active_state: Some("active".to_string()),
            sub_state: Some("running".to_string()),
            enabled: Some(true),
            services: vec![],
            output: "ok".to_string(),
            errors: vec![],
        };
        assert!(result.success);
        assert_eq!(result.active_state.as_deref(), Some("active"));
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_service_ops_result_failure() {
        let result = ServiceOpsResult {
            action: "Start".to_string(),
            service: "broken".to_string(),
            success: false,
            active_state: Some("failed".to_string()),
            sub_state: Some("dead".to_string()),
            enabled: None,
            services: vec![],
            output: "Job failed".to_string(),
            errors: vec!["启动失败".to_string()],
        };
        assert!(!result.success);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_service_ops_result_list() {
        let result = ServiceOpsResult {
            action: "ListUnits".to_string(),
            service: "service".to_string(),
            success: true,
            active_state: None,
            sub_state: None,
            enabled: None,
            services: vec![
                ServiceInfo {
                    name: "nginx.service".to_string(),
                    unit_type: "service".to_string(),
                    active_state: "active".to_string(),
                    sub_state: "running".to_string(),
                    description: "nginx".to_string(),
                    enabled: true,
                },
                ServiceInfo {
                    name: "redis.service".to_string(),
                    unit_type: "service".to_string(),
                    active_state: "active".to_string(),
                    sub_state: "running".to_string(),
                    description: "redis".to_string(),
                    enabled: true,
                },
            ],
            output: "共 2 个单元".to_string(),
            errors: vec![],
        };
        assert_eq!(result.services.len(), 2);
        assert_eq!(result.services[0].name, "nginx.service");
    }

    // === TaskType::ServiceOps 测试 ===

    #[test]
    fn test_task_type_service_ops() {
        let task = TaskType::ServiceOps {
            hosts: vec!["server1".to_string()],
            action: ServiceAction::Start {
                service: "nginx".to_string(),
            },
        };
        assert!(matches!(task, TaskType::ServiceOps { .. }));
    }

    // === 序列化测试 ===

    #[test]
    fn test_service_action_serialization_roundtrip() {
        let action = ServiceAction::Restart {
            service: "mysql".to_string(),
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ServiceAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_service_ops_result_serialization_roundtrip() {
        let result = ServiceOpsResult {
            action: "Status".to_string(),
            service: "nginx".to_string(),
            success: true,
            active_state: Some("active".to_string()),
            sub_state: Some("running".to_string()),
            enabled: Some(true),
            services: vec![ServiceInfo {
                name: "nginx.service".to_string(),
                unit_type: "service".to_string(),
                active_state: "active".to_string(),
                sub_state: "running".to_string(),
                description: "nginx".to_string(),
                enabled: true,
            }],
            output: "active".to_string(),
            errors: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ServiceOpsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_service_info_serialization_roundtrip() {
        let info = ServiceInfo {
            name: "docker.service".to_string(),
            unit_type: "service".to_string(),
            active_state: "active".to_string(),
            sub_state: "running".to_string(),
            description: "Docker Application Container Engine".to_string(),
            enabled: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ServiceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    // === parse_unit_show 测试 ===

    #[test]
    fn test_parse_unit_show_active() {
        let output = "ActiveState=active\nSubState=running\nUnitFileState=enabled\nDescription=A high performance web server";
        let (active, sub, enabled, desc) = ServiceManager::parse_unit_show(output);
        assert_eq!(active, "active");
        assert_eq!(sub, "running");
        assert!(enabled);
        assert_eq!(desc, "A high performance web server");
    }

    #[test]
    fn test_parse_unit_show_inactive() {
        let output =
            "ActiveState=inactive\nSubState=dead\nUnitFileState=disabled\nDescription=Some service";
        let (active, sub, enabled, desc) = ServiceManager::parse_unit_show(output);
        assert_eq!(active, "inactive");
        assert_eq!(sub, "dead");
        assert!(!enabled);
        assert_eq!(desc, "Some service");
    }

    #[test]
    fn test_parse_unit_show_empty() {
        let output = "";
        let (active, sub, enabled, desc) = ServiceManager::parse_unit_show(output);
        assert!(active.is_empty());
        assert!(sub.is_empty());
        assert!(!enabled);
        assert!(desc.is_empty());
    }

    #[test]
    fn test_parse_unit_show_failed() {
        let output = "ActiveState=failed\nSubState=failed\nUnitFileState=enabled\nDescription=Broken service";
        let (active, sub, enabled, _desc) = ServiceManager::parse_unit_show(output);
        assert_eq!(active, "failed");
        assert_eq!(sub, "failed");
        assert!(enabled);
    }

    // === parse_list_units 测试 ===

    #[test]
    fn test_parse_list_units_normal() {
        let output = "nginx.service     loaded active running A high performance web server\n\
                       redis.service     loaded active running Redis key-value store\n\
                       mysql.service     loaded active running MySQL database";
        let services = ServiceManager::parse_list_units(output);
        assert_eq!(services.len(), 3);
        assert_eq!(services[0].name, "nginx.service");
        assert_eq!(services[0].active_state, "active");
        assert_eq!(services[1].name, "redis.service");
        assert_eq!(services[2].name, "mysql.service");
    }

    #[test]
    fn test_parse_list_units_empty() {
        let services = ServiceManager::parse_list_units("");
        assert!(services.is_empty());
    }

    #[test]
    fn test_parse_list_units_mixed_states() {
        let output = "nginx.service  loaded active running nginx\n\
                       broken.service loaded failed failed broken";
        let services = ServiceManager::parse_list_units(output);
        assert_eq!(services.len(), 2);
        assert_eq!(services[0].active_state, "active");
        assert_eq!(services[1].active_state, "failed");
    }

    #[test]
    fn test_parse_list_units_short_line_ignored() {
        let output = "short\nnginx.service loaded active running nginx";
        let services = ServiceManager::parse_list_units(output);
        assert_eq!(services.len(), 1);
    }

    // === 安全边界测试 ===

    #[test]
    fn test_service_action_all_variants_covered() {
        // 确保所有变体都可以构造
        let actions = vec![
            ServiceAction::Start {
                service: "s".into(),
            },
            ServiceAction::Stop {
                service: "s".into(),
            },
            ServiceAction::Restart {
                service: "s".into(),
            },
            ServiceAction::Status {
                service: "s".into(),
            },
            ServiceAction::Enable {
                service: "s".into(),
            },
            ServiceAction::Disable {
                service: "s".into(),
            },
            ServiceAction::Logs {
                service: "s".into(),
                lines: 10,
            },
            ServiceAction::ListUnits {
                unit_type: "s".into(),
            },
            ServiceAction::ListFailed {},
            ServiceAction::DaemonReload {},
            ServiceAction::NginxReload {},
            ServiceAction::NginxTest {},
            ServiceAction::NginxStatus {},
            ServiceAction::MysqlStatus {},
            ServiceAction::MysqlQuery {
                query: "SELECT 1".into(),
            },
            ServiceAction::MysqlCheckTables {
                database: "db".into(),
            },
            ServiceAction::RedisStatus {},
            ServiceAction::RedisInfo {},
            ServiceAction::RedisCommand {
                command: "PING".into(),
            },
        ];
        assert_eq!(actions.len(), 19);
    }

    #[test]
    fn test_service_ops_result_error_field() {
        let result = ServiceOpsResult {
            action: "ListFailed".to_string(),
            service: "failed".to_string(),
            success: false,
            active_state: None,
            sub_state: None,
            enabled: None,
            services: vec![],
            output: "3 failed units".to_string(),
            errors: vec![
                "nginx.service failed".to_string(),
                "redis.service failed".to_string(),
            ],
        };
        assert_eq!(result.errors.len(), 2);
        assert!(!result.success);
    }
}
