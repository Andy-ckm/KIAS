//! R035: 定时任务管理模块 — crontab + systemd timer
//!
//! Cron 任务增删改查 / systemd timer 管理 / 解析 / 安全校验
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: Ansible cron module, crontab.guru, systemd-cron
//! AgentGuard差异化: crontab+systemd统一管理→冲突检测→合规审计（竞品只做crontab）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 定时任务管理引擎
///
/// 提供 crontab 和 systemd timer 的完整管理：
/// 列出/添加/删除/启用/禁用 cron 任务，systemd timer 的创建/删除/启用/禁用
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct CronManager;

impl CronManager {
    /// 执行定时任务操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &CronAction,
        audit: &AuditLog,
    ) -> Result<CronOpsResult> {
        let _audit_id = uuid::Uuid::new_v4().to_string();
        let mut commands_executed = Vec::new();

        let (action_desc, result) = match action {
            CronAction::List => {
                let output =
                    Self::run_cmd(executor, host, "crontab -l 2>/dev/null || true").await?;
                commands_executed.push("crontab -l".to_string());
                let jobs = Self::parse_crontab(&output);
                let count = jobs.len();
                (
                    format!("列出 cron 任务: {} 条", count),
                    CronOpsResult {
                        action: "List".to_string(),
                        success: true,
                        jobs,
                        output: format!("共 {} 条 cron 任务", count),
                        ..Default::default()
                    },
                )
            }
            CronAction::Add {
                schedule,
                command,
                comment,
            } => {
                // 安全校验: 验证 cron 表达式格式
                if !Self::validate_cron_schedule(schedule) {
                    return Err(AutomationError::CommandExecution(format!(
                        "无效的 cron 表达式: {}",
                        schedule
                    )));
                }
                // 安全校验: 危险命令黑名单
                if Self::is_dangerous_command(command) {
                    return Err(AutomationError::PermissionDenied(format!(
                        "危险命令被拒绝: {}",
                        command
                    )));
                }

                let entry = if let Some(c) = comment {
                    format!("# {}\n{} {}", c, schedule, command)
                } else {
                    format!("{} {}", schedule, command)
                };
                let escaped = entry.replace('\'', "'\\''");
                let cmd = format!("(crontab -l 2>/dev/null; echo '{}') | crontab -", escaped);
                Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd.clone());

                (
                    format!("添加 cron 任务: {} {}", schedule, command),
                    CronOpsResult {
                        action: "Add".to_string(),
                        success: true,
                        output: format!("已添加: {} {}", schedule, command),
                        ..Default::default()
                    },
                )
            }
            CronAction::Remove { job_id } => {
                // 先列出确认行存在
                let list_output =
                    Self::run_cmd(executor, host, "crontab -l 2>/dev/null || true").await?;
                let jobs = Self::parse_crontab(&list_output);
                if jobs.iter().all(|j| j.line_number != *job_id) {
                    return Err(AutomationError::TaskNotFound(format!(
                        "cron 任务行号 {} 不存在",
                        job_id
                    )));
                }

                let cmd = format!("crontab -l 2>/dev/null | sed '{}d' | crontab -", job_id);
                Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);

                (
                    format!("删除 cron 任务: 行 {}", job_id),
                    CronOpsResult {
                        action: "Remove".to_string(),
                        success: true,
                        output: format!("已删除行 {}", job_id),
                        ..Default::default()
                    },
                )
            }
            CronAction::Enable { job_id } => {
                let list_output =
                    Self::run_cmd(executor, host, "crontab -l 2>/dev/null || true").await?;
                let jobs = Self::parse_crontab(&list_output);
                let target = jobs.iter().find(|j| j.line_number == *job_id);
                match target {
                    Some(job) if !job.enabled => {
                        // 取消注释: 删除行首的 #
                        let cmd = format!(
                            "crontab -l 2>/dev/null | sed '{}s/^# //' | crontab -",
                            job_id
                        );
                        Self::run_cmd(executor, host, &cmd).await?;
                        commands_executed.push(cmd);
                        (
                            format!("启用 cron 任务: 行 {}", job_id),
                            CronOpsResult {
                                action: "Enable".to_string(),
                                success: true,
                                output: format!("已启用行 {}", job_id),
                                ..Default::default()
                            },
                        )
                    }
                    Some(_) => (
                        format!("cron 任务行 {} 已经是启用状态", job_id),
                        CronOpsResult {
                            action: "Enable".to_string(),
                            success: true,
                            output: format!("行 {} 已启用", job_id),
                            ..Default::default()
                        },
                    ),
                    None => {
                        return Err(AutomationError::TaskNotFound(format!(
                            "cron 任务行号 {} 不存在",
                            job_id
                        )));
                    }
                }
            }
            CronAction::Disable { job_id } => {
                let list_output =
                    Self::run_cmd(executor, host, "crontab -l 2>/dev/null || true").await?;
                let jobs = Self::parse_crontab(&list_output);
                let target = jobs.iter().find(|j| j.line_number == *job_id);
                match target {
                    Some(job) if job.enabled => {
                        let cmd = format!(
                            "crontab -l 2>/dev/null | sed '{}s/^/# /' | crontab -",
                            job_id
                        );
                        Self::run_cmd(executor, host, &cmd).await?;
                        commands_executed.push(cmd);
                        (
                            format!("禁用 cron 任务: 行 {}", job_id),
                            CronOpsResult {
                                action: "Disable".to_string(),
                                success: true,
                                output: format!("已禁用行 {}", job_id),
                                ..Default::default()
                            },
                        )
                    }
                    Some(_) => (
                        format!("cron 任务行 {} 已经是禁用状态", job_id),
                        CronOpsResult {
                            action: "Disable".to_string(),
                            success: true,
                            output: format!("行 {} 已禁用", job_id),
                            ..Default::default()
                        },
                    ),
                    None => {
                        return Err(AutomationError::TaskNotFound(format!(
                            "cron 任务行号 {} 不存在",
                            job_id
                        )));
                    }
                }
            }
            CronAction::ListSystemdTimers => {
                let output = Self::run_cmd(
                    executor,
                    host,
                    "systemctl list-timers --no-pager --all 2>/dev/null || true",
                )
                .await?;
                commands_executed.push("systemctl list-timers".to_string());
                let timers = Self::parse_systemd_timers(&output);
                let count = timers.len();
                (
                    format!("列出 systemd 定时器: {} 个", count),
                    CronOpsResult {
                        action: "ListSystemdTimers".to_string(),
                        success: true,
                        timers,
                        output: format!("共 {} 个 systemd 定时器", count),
                        ..Default::default()
                    },
                )
            }
            CronAction::CreateSystemdTimer {
                name,
                schedule,
                command,
            } => {
                // 安全校验
                if !Self::validate_systemd_timer_name(name) {
                    return Err(AutomationError::CommandExecution(format!(
                        "无效的定时器名称: {} (只允许字母数字和连字符)",
                        name
                    )));
                }
                if Self::is_dangerous_command(command) {
                    return Err(AutomationError::PermissionDenied(format!(
                        "危险命令被拒绝: {}",
                        command
                    )));
                }

                // 创建 service unit
                let service_content = format!(
                    "[Unit]\nDescription=AgentGuard managed service: {}\n[Service]\nType=oneshot\nExecStart={}\n",
                    name, command
                );
                let service_cmd = format!(
                    "echo '{}' > /etc/systemd/system/{}.service",
                    service_content.replace('\'', "'\\''"),
                    name
                );
                Self::run_cmd(executor, host, &service_cmd).await?;
                commands_executed.push(service_cmd);

                // 创建 timer unit
                let timer_content = format!(
                    "[Unit]\nDescription=AgentGuard managed timer: {}\n[Timer]\nOnCalendar={}\nPersistent=true\n[Install]\nWantedBy=timers.target\n",
                    name, schedule
                );
                let timer_cmd = format!(
                    "echo '{}' > /etc/systemd/system/{}.timer",
                    timer_content.replace('\'', "'\\''"),
                    name
                );
                Self::run_cmd(executor, host, &timer_cmd).await?;
                commands_executed.push(timer_cmd);

                // daemon-reload + enable
                Self::run_cmd(executor, host, "systemctl daemon-reload").await?;
                commands_executed.push("systemctl daemon-reload".to_string());
                let enable_cmd = format!("systemctl enable {}.timer", name);
                Self::run_cmd(executor, host, &enable_cmd).await?;
                commands_executed.push(enable_cmd);

                (
                    format!("创建 systemd 定时器: {}", name),
                    CronOpsResult {
                        action: "CreateSystemdTimer".to_string(),
                        success: true,
                        output: format!("已创建定时器 {} ({})", name, schedule),
                        ..Default::default()
                    },
                )
            }
            CronAction::RemoveSystemdTimer { name } => {
                if !Self::validate_systemd_timer_name(name) {
                    return Err(AutomationError::CommandExecution(format!(
                        "无效的定时器名称: {}",
                        name
                    )));
                }

                let disable_cmd =
                    format!("systemctl disable --now {}.timer 2>/dev/null || true", name);
                Self::run_cmd(executor, host, &disable_cmd).await?;
                commands_executed.push(disable_cmd);

                let rm_timer = format!("rm -f /etc/systemd/system/{}.timer", name);
                Self::run_cmd(executor, host, &rm_timer).await?;
                commands_executed.push(rm_timer);

                let rm_service = format!("rm -f /etc/systemd/system/{}.service", name);
                Self::run_cmd(executor, host, &rm_service).await?;
                commands_executed.push(rm_service);

                Self::run_cmd(executor, host, "systemctl daemon-reload").await?;
                commands_executed.push("systemctl daemon-reload".to_string());

                (
                    format!("删除 systemd 定时器: {}", name),
                    CronOpsResult {
                        action: "RemoveSystemdTimer".to_string(),
                        success: true,
                        output: format!("已删除定时器 {}", name),
                        ..Default::default()
                    },
                )
            }
            CronAction::EnableSystemdTimer { name } => {
                if !Self::validate_systemd_timer_name(name) {
                    return Err(AutomationError::CommandExecution(format!(
                        "无效的定时器名称: {}",
                        name
                    )));
                }
                let cmd = format!("systemctl enable --now {}.timer", name);
                Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);

                (
                    format!("启用 systemd 定时器: {}", name),
                    CronOpsResult {
                        action: "EnableSystemdTimer".to_string(),
                        success: true,
                        output: format!("已启用定时器 {}", name),
                        ..Default::default()
                    },
                )
            }
            CronAction::DisableSystemdTimer { name } => {
                if !Self::validate_systemd_timer_name(name) {
                    return Err(AutomationError::CommandExecution(format!(
                        "无效的定时器名称: {}",
                        name
                    )));
                }
                let cmd = format!("systemctl disable --now {}.timer", name);
                Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);

                (
                    format!("禁用 systemd 定时器: {}", name),
                    CronOpsResult {
                        action: "DisableSystemdTimer".to_string(),
                        success: true,
                        output: format!("已禁用定时器 {}", name),
                        ..Default::default()
                    },
                )
            }
        };

        // 审计日志
        let _ = audit.log_action(
            "system",
            "CronOps",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&commands_executed).unwrap_or_default(),
        );

        info!(
            host = %host,
            action = %result.action,
            success = result.success,
            "CronOps 操作完成"
        );

        Ok(result)
    }

    // === 内部辅助方法 ===

    /// 执行远程命令
    async fn run_cmd(executor: &TaskExecutor, host: &str, cmd: &str) -> Result<String> {
        let result = executor.execute_command(&[host.to_string()], cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();
        Ok(output)
    }

    /// 解析 crontab -l 输出
    pub fn parse_crontab(output: &str) -> Vec<CronJobEntry> {
        let mut jobs = Vec::new();
        let mut current_comment: Option<String> = None;

        for (idx, line) in output.lines().enumerate() {
            let line_num = (idx + 1) as u32;
            let trimmed = line.trim();

            // 跳过空行和环境变量行
            if trimmed.is_empty() {
                current_comment = None;
                continue;
            }
            if !trimmed.starts_with('#') && trimmed.contains('=') && !trimmed.contains('*') {
                // 环境变量设置 (如 MAILTO=root)
                current_comment = None;
                continue;
            }

            if let Some(uncommented) = trimmed.strip_prefix("# ") {
                // 注释行 — 可能是被禁用的 cron 任务
                if Self::looks_like_cron_line(uncommented) {
                    let parts: Vec<&str> = uncommented.splitn(6, ' ').collect();
                    if parts.len() >= 6 {
                        let schedule = parts[..5].join(" ");
                        let command = parts[5..].join(" ");
                        jobs.push(CronJobEntry {
                            line_number: line_num,
                            schedule,
                            command,
                            comment: current_comment.take(),
                            enabled: false,
                            source: CronSource::Crontab,
                        });
                        continue;
                    }
                }
                // 普通注释
                current_comment = Some(trimmed.to_string());
                continue;
            }

            if Self::looks_like_cron_line(trimmed) {
                let parts: Vec<&str> = trimmed.splitn(6, ' ').collect();
                if parts.len() >= 6 {
                    let schedule = parts[..5].join(" ");
                    let command = parts[5..].join(" ");
                    jobs.push(CronJobEntry {
                        line_number: line_num,
                        schedule,
                        command,
                        comment: current_comment.take(),
                        enabled: true,
                        source: CronSource::Crontab,
                    });
                    continue;
                }
            }

            current_comment = None;
        }

        jobs
    }

    /// 判断一行是否像 cron 任务行
    fn looks_like_cron_line(line: &str) -> bool {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 6 {
            return false;
        }
        // 前5个字段应该是 cron 表达式（数字、*、逗号、连字符、斜杠）
        for part in &parts[..5] {
            if !Self::is_cron_field(part) {
                return false;
            }
        }
        true
    }

    /// 判断是否是合法的 cron 字段
    fn is_cron_field(field: &str) -> bool {
        !field.is_empty()
            && field
                .chars()
                .all(|c| c.is_ascii_digit() || c == '*' || c == ',' || c == '-' || c == '/')
    }

    /// 验证 cron 表达式格式（5 字段）
    pub fn validate_cron_schedule(schedule: &str) -> bool {
        let parts: Vec<&str> = schedule.split_whitespace().collect();
        if parts.len() != 5 {
            return false;
        }
        parts.iter().all(|p| Self::is_cron_field(p))
    }

    /// 验证 systemd timer 名称（只允许字母数字和连字符）
    fn validate_systemd_timer_name(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !name.starts_with('-')
    }

    /// 危险命令黑名单
    fn is_dangerous_command(command: &str) -> bool {
        let lower = command.to_lowercase();
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            "mkfs.",
            "dd if=",
            ":(){:|:&};:", // fork bomb
            "chmod 777 /",
            "chmod -R 777 /",
            "> /dev/sda",
            "shutdown",
            "reboot",
            "halt",
            "init 0",
            "init 6",
        ];
        dangerous_patterns
            .iter()
            .any(|pattern| lower.contains(pattern))
    }

    /// 解析 systemctl list-timers 输出
    pub fn parse_systemd_timers(output: &str) -> Vec<SystemdTimerEntry> {
        let mut timers = Vec::new();

        for line in output.lines().skip(1) {
            // 跳过表头
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            // systemctl list-timers 输出格式:
            // NEXT                         LEFT     LAST                         PASSED   UNIT                         ACTIVATES
            // Mon 2024-01-01 02:00:00 UTC  5h left  Sun 2024-01-01 02:00:00 UTC  19h ago  backup.timer                 backup.service
            let fields: Vec<&str> = trimmed.split_whitespace().collect();
            if fields.len() < 5 {
                continue;
            }

            // 找到 .timer 或 .service 单元名
            let unit_field = fields
                .iter()
                .find(|f| f.ends_with(".timer") || f.ends_with(".service"));
            let _activates_field = fields
                .iter()
                .find(|f| f.ends_with(".service") && Some(*f) != unit_field);

            if let Some(unit) = unit_field {
                let name = unit.replace(".timer", "").replace(".service", "");
                let active = !trimmed.contains("inactive");

                // 解析 NEXT 和 LAST
                let next_trigger = if fields.len() > 2 {
                    format!("{} {}", fields[0], fields[1])
                } else {
                    fields[0].to_string()
                };

                timers.push(SystemdTimerEntry {
                    name,
                    next_trigger,
                    last_trigger: None,
                    elapsed: None,
                    unit: unit.to_string(),
                    active,
                });
            }
        }

        timers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === parse_crontab 测试 ===

    #[test]
    fn test_parse_crontab_empty() {
        let jobs = CronManager::parse_crontab("");
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_parse_crontab_single_job() {
        let input = "0 2 * * * /usr/bin/backup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].schedule, "0 2 * * *");
        assert_eq!(jobs[0].command, "/usr/bin/backup.sh");
        assert!(jobs[0].enabled);
        assert_eq!(jobs[0].source, CronSource::Crontab);
    }

    #[test]
    fn test_parse_crontab_multiple_jobs() {
        let input = "0 2 * * * /usr/bin/backup.sh\n30 3 * * 0 /usr/bin/cleanup.sh\n*/5 * * * * /usr/bin/monitor.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 3);
        assert_eq!(jobs[0].schedule, "0 2 * * *");
        assert_eq!(jobs[1].schedule, "30 3 * * 0");
        assert_eq!(jobs[2].schedule, "*/5 * * * *");
    }

    #[test]
    fn test_parse_crontab_disabled_job() {
        let input = "# 0 2 * * * /usr/bin/backup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 1);
        assert!(!jobs[0].enabled);
        assert_eq!(jobs[0].schedule, "0 2 * * *");
    }

    #[test]
    fn test_parse_crontab_with_env_vars() {
        let input = "MAILTO=admin@example.com\n0 2 * * * /usr/bin/backup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].schedule, "0 2 * * *");
    }

    #[test]
    fn test_parse_crontab_with_comments() {
        let input = "# Daily backup\n0 2 * * * /usr/bin/backup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].comment, Some("# Daily backup".to_string()));
    }

    #[test]
    fn test_parse_crontab_empty_lines() {
        let input = "0 2 * * * /usr/bin/backup.sh\n\n\n30 3 * * 0 /usr/bin/cleanup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 2);
    }

    #[test]
    fn test_parse_crontab_line_numbers() {
        let input = "# Comment\n0 2 * * * /usr/bin/backup.sh\n30 3 * * 0 /usr/bin/cleanup.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs[0].line_number, 2);
        assert_eq!(jobs[1].line_number, 3);
    }

    #[test]
    fn test_parse_crontab_command_with_spaces() {
        let input = "0 2 * * * /usr/bin/python3 /opt/scripts/backup.py --verbose";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            jobs[0].command,
            "/usr/bin/python3 /opt/scripts/backup.py --verbose"
        );
    }

    #[test]
    fn test_parse_crontab_mixed_enabled_disabled() {
        let input = "0 2 * * * /usr/bin/backup.sh\n# 30 3 * * 0 /usr/bin/cleanup.sh\n*/5 * * * * /usr/bin/monitor.sh";
        let jobs = CronManager::parse_crontab(input);
        assert_eq!(jobs.len(), 3);
        assert!(jobs[0].enabled);
        assert!(!jobs[1].enabled);
        assert!(jobs[2].enabled);
    }

    // === validate_cron_schedule 测试 ===

    #[test]
    fn test_validate_cron_schedule_valid() {
        assert!(CronManager::validate_cron_schedule("0 2 * * *"));
        assert!(CronManager::validate_cron_schedule("*/5 * * * *"));
        assert!(CronManager::validate_cron_schedule("0 0 1 * *"));
        assert!(CronManager::validate_cron_schedule("30 2 1,15 * *"));
        assert!(CronManager::validate_cron_schedule("0 9-17 * * 1-5"));
    }

    #[test]
    fn test_validate_cron_schedule_invalid() {
        assert!(!CronManager::validate_cron_schedule(""));
        assert!(!CronManager::validate_cron_schedule("0 2 * *"));
        assert!(!CronManager::validate_cron_schedule("0 2 * * * *"));
        assert!(!CronManager::validate_cron_schedule("abc 2 * * *"));
        assert!(!CronManager::validate_cron_schedule("0 2 * * /usr/bin/cmd"));
    }

    // === is_cron_field 测试 ===

    #[test]
    fn test_is_cron_field_valid() {
        assert!(CronManager::is_cron_field("*"));
        assert!(CronManager::is_cron_field("0"));
        assert!(CronManager::is_cron_field("*/5"));
        assert!(CronManager::is_cron_field("1,3,5"));
        assert!(CronManager::is_cron_field("9-17"));
        assert!(CronManager::is_cron_field("0/15"));
    }

    #[test]
    fn test_is_cron_field_invalid() {
        assert!(!CronManager::is_cron_field(""));
        assert!(!CronManager::is_cron_field("abc"));
        assert!(!CronManager::is_cron_field("/usr/bin/cmd"));
    }

    // === is_dangerous_command 测试 ===

    #[test]
    fn test_is_dangerous_command_dangerous() {
        assert!(CronManager::is_dangerous_command("rm -rf /"));
        assert!(CronManager::is_dangerous_command("rm -rf /*"));
        assert!(CronManager::is_dangerous_command("mkfs.ext4 /dev/sda1"));
        assert!(CronManager::is_dangerous_command(
            "dd if=/dev/zero of=/dev/sda"
        ));
        assert!(CronManager::is_dangerous_command(":(){:|:&};:"));
        assert!(CronManager::is_dangerous_command("shutdown -h now"));
        assert!(CronManager::is_dangerous_command("reboot"));
        assert!(CronManager::is_dangerous_command("chmod 777 /"));
    }

    #[test]
    fn test_is_dangerous_command_safe() {
        assert!(!CronManager::is_dangerous_command("/usr/bin/backup.sh"));
        assert!(!CronManager::is_dangerous_command("echo hello"));
        assert!(!CronManager::is_dangerous_command(
            "find /tmp -name '*.log' -delete"
        ));
        assert!(!CronManager::is_dangerous_command(
            "systemctl restart nginx"
        ));
    }

    // === validate_systemd_timer_name 测试 ===

    #[test]
    fn test_validate_systemd_timer_name_valid() {
        assert!(CronManager::validate_systemd_timer_name("backup-daily"));
        assert!(CronManager::validate_systemd_timer_name("log_cleanup"));
        assert!(CronManager::validate_systemd_timer_name("timer123"));
    }

    #[test]
    fn test_validate_systemd_timer_name_invalid() {
        assert!(!CronManager::validate_systemd_timer_name(""));
        assert!(!CronManager::validate_systemd_timer_name("-starts-dash"));
        assert!(!CronManager::validate_systemd_timer_name("has spaces"));
        assert!(!CronManager::validate_systemd_timer_name("special@chars"));
    }

    // === parse_systemd_timers 测试 ===

    #[test]
    fn test_parse_systemd_timers_empty() {
        let timers = CronManager::parse_systemd_timers("");
        assert!(timers.is_empty());
    }

    #[test]
    fn test_parse_systemd_timers_basic() {
        let input = "NEXT                         LEFT     LAST                         PASSED   UNIT                         ACTIVATES\n\
Mon 2024-01-01 02:00:00 UTC  5h left  Sun 2024-01-01 02:00:00 UTC  19h ago  backup.timer                 backup.service";
        let timers = CronManager::parse_systemd_timers(input);
        assert_eq!(timers.len(), 1);
        assert_eq!(timers[0].name, "backup");
        assert_eq!(timers[0].unit, "backup.timer");
    }

    // === looks_like_cron_line 测试 ===

    #[test]
    fn test_looks_like_cron_line_valid() {
        assert!(CronManager::looks_like_cron_line(
            "0 2 * * * /usr/bin/backup.sh"
        ));
        assert!(CronManager::looks_like_cron_line(
            "*/5 * * * * /usr/bin/monitor.sh"
        ));
    }

    #[test]
    fn test_looks_like_cron_line_invalid() {
        assert!(!CronManager::looks_like_cron_line(""));
        assert!(!CronManager::looks_like_cron_line("# This is a comment"));
        assert!(!CronManager::looks_like_cron_line("MAILTO=root"));
        assert!(!CronManager::looks_like_cron_line("just some text"));
    }

    // === CronAction 序列化测试 ===

    #[test]
    fn test_cron_action_serialize_roundtrip() {
        let action = CronAction::Add {
            schedule: "0 2 * * *".to_string(),
            command: "/usr/bin/backup.sh".to_string(),
            comment: Some("Daily backup".to_string()),
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: CronAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_cron_action_list_serialize() {
        let action = CronAction::List;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("List"));
        let deserialized: CronAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_cron_ops_result_default() {
        let result = CronOpsResult::default();
        assert!(result.success);
        assert!(result.jobs.is_empty());
        assert!(result.timers.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_cron_job_entry_serialize() {
        let entry = CronJobEntry {
            line_number: 1,
            schedule: "0 2 * * *".to_string(),
            command: "/usr/bin/backup.sh".to_string(),
            comment: None,
            enabled: true,
            source: CronSource::Crontab,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: CronJobEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_systemd_timer_entry_serialize() {
        let entry = SystemdTimerEntry {
            name: "backup".to_string(),
            next_trigger: "Mon 2024-01-01".to_string(),
            last_trigger: Some("Sun 2024-01-01".to_string()),
            elapsed: Some("19h ago".to_string()),
            unit: "backup.timer".to_string(),
            active: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SystemdTimerEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }
}
