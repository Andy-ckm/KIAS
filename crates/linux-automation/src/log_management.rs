//! R037: 日志管理模块
//!
//! 日志搜索/解析/轮转检查/统计/健康分析/尾部跟踪
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: journalctl/syslog-ng/logrotate/ELK/Loki
//! AgentGuard差异化: 日志采集→解析→分析→健康评估→审计（竞品只存储不分析）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::models::*;

/// 日志管理引擎
///
/// 提供完整的日志管理：搜索/解析/轮转检查/统计/健康分析/尾部跟踪
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct LogManager;

impl LogManager {
    /// 执行日志管理操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &LogAction,
        audit: &AuditLog,
    ) -> Result<LogManageResult> {
        let mut commands_executed = Vec::new();

        let (action_desc, result) = match action {
            LogAction::Search {
                path,
                pattern,
                max_results,
            } => {
                Self::search_logs(
                    executor,
                    host,
                    path,
                    pattern,
                    *max_results,
                    &mut commands_executed,
                )
                .await?
            }
            LogAction::Parse { path, lines } => {
                Self::parse_logs(executor, host, path, *lines, &mut commands_executed).await?
            }
            LogAction::RotationCheck { path } => {
                Self::check_rotation(executor, host, path, &mut commands_executed).await?
            }
            LogAction::Stats { path, since_hours } => {
                Self::log_stats(executor, host, path, *since_hours, &mut commands_executed).await?
            }
            LogAction::HealthCheck {
                paths,
                error_threshold_pct,
            } => {
                Self::health_check(
                    executor,
                    host,
                    paths,
                    *error_threshold_pct,
                    &mut commands_executed,
                )
                .await?
            }
            LogAction::Tail {
                path,
                lines,
                filter,
            } => {
                Self::tail_logs(
                    executor,
                    host,
                    path,
                    *lines,
                    filter.as_deref(),
                    &mut commands_executed,
                )
                .await?
            }
        };

        // 审计日志
        let _ = audit.log_action(
            "system",
            "LogManagement",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&commands_executed).unwrap_or_default(),
        );

        info!(
            host = %host,
            action = %action_desc,
            success = result.success,
            "日志管理操作完成"
        );

        Ok(result)
    }

    /// 搜索日志（grep 模式匹配）
    async fn search_logs(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        pattern: &str,
        max_results: u32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        let cmd = format!(
            "grep -n '{}' {} 2>/dev/null | head -{}",
            pattern.replace('\'', "'\\''"),
            path,
            max_results
        );
        cmds.push(cmd.clone());

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let entries: Vec<LogEntry> = output
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(Self::parse_log_line)
            .collect();

        let count = entries.len();
        let desc = format!("搜索日志 {} 模式='{}' 找到 {} 条", path, pattern, count);

        Ok((
            desc,
            LogManageResult {
                action: "Search".to_string(),
                success: true,
                entries,
                output: format!("找到 {} 条匹配记录", count),
                ..Default::default()
            },
        ))
    }

    /// 解析 syslog 格式日志
    async fn parse_logs(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        lines: u32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        let cmd = format!("tail -n {} {} 2>/dev/null", lines, path);
        cmds.push(cmd.clone());

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let entries: Vec<LogEntry> = output
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(Self::parse_log_line)
            .collect();

        let count = entries.len();
        let desc = format!("解析日志 {} 最近 {} 行, 解析 {} 条", path, lines, count);

        Ok((
            desc,
            LogManageResult {
                action: "Parse".to_string(),
                success: true,
                entries,
                output: format!("解析 {} 行日志", count),
                ..Default::default()
            },
        ))
    }

    /// 检查日志轮转配置
    async fn check_rotation(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        // 获取当前文件大小
        let size_cmd = format!("stat -c '%s' {} 2>/dev/null || echo 0", path);
        cmds.push(size_cmd.clone());
        let size_result = executor
            .execute_command(&[host.to_string()], &size_cmd)
            .await?;
        let size_str = size_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_else(|| "0".to_string());
        let size_bytes: u64 = size_str.parse().unwrap_or(0);
        let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

        // 检查 logrotate 配置
        let rotate_cmd = format!(
            "logrotate -d /etc/logrotate.conf 2>&1 | grep -i '{}' | head -5",
            path.replace('\'', "'\\''")
        );
        cmds.push(rotate_cmd.clone());
        let rotate_result = executor
            .execute_command(&[host.to_string()], &rotate_cmd)
            .await?;
        let rotate_output = rotate_result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        // 检查已轮转的文件
        let rotated_cmd = format!("ls -la {}.* 2>/dev/null | head -10", path);
        cmds.push(rotated_cmd.clone());
        let rotated_result = executor
            .execute_command(&[host.to_string()], &rotated_cmd)
            .await?;
        let rotated_output = rotated_result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let rotated_files: Vec<String> = rotated_output
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.split_whitespace().last().unwrap_or("").to_string())
            .filter(|f| !f.is_empty())
            .collect();

        let needs_rotation = size_mb > 100.0; // > 100MB 建议轮转

        let rotation_info = LogRotationInfo {
            log_path: path.to_string(),
            current_size_mb: (size_mb * 100.0).round() / 100.0,
            rotated_files,
            rotation_config: if rotate_output.is_empty() {
                "未找到 logrotate 配置".to_string()
            } else {
                rotate_output.lines().take(3).collect::<Vec<_>>().join("; ")
            },
            needs_rotation,
        };

        let desc = format!(
            "检查日志轮转 {} 大小={:.2}MB 需要轮转={}",
            path, size_mb, needs_rotation
        );

        Ok((
            desc,
            LogManageResult {
                action: "RotationCheck".to_string(),
                success: true,
                rotation_info: Some(rotation_info),
                output: format!(
                    "文件大小: {:.2} MB, 轮转文件: {} 个, 需要轮转: {}",
                    size_mb,
                    rotated_output.lines().count(),
                    needs_rotation
                ),
                ..Default::default()
            },
        ))
    }

    /// 日志统计
    async fn log_stats(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        since_hours: u32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        // 使用 awk 统计各级别日志数量
        let cmd = format!(
            "tail -n 10000 {} 2>/dev/null | awk 'BEGIN{{e=0;w=0;i=0;d=0}} \
             /ERROR|error|Error/{{e++}} /WARN|warn|Warning/{{w++}} /INFO|info/{{i++}} /DEBUG|debug/{{d++}} \
             END{{printf \"%d %d %d %d %d\", NR, e, w, i, d}}'",
            path
        );
        cmds.push(cmd.clone());

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        let parts: Vec<u64> = output
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        let stats = LogStats {
            total_lines: parts.first().copied().unwrap_or(0),
            error_count: parts.get(1).copied().unwrap_or(0),
            warn_count: parts.get(2).copied().unwrap_or(0),
            info_count: parts.get(3).copied().unwrap_or(0),
            debug_count: parts.get(4).copied().unwrap_or(0),
            by_source: Vec::new(),
            time_range: format!("最近 {} 小时", since_hours),
        };

        let desc = format!(
            "日志统计 {} 总行数={} 错误={} 警告={}",
            path, stats.total_lines, stats.error_count, stats.warn_count
        );

        Ok((
            desc,
            LogManageResult {
                action: "Stats".to_string(),
                success: true,
                stats: Some(stats),
                output: format!(
                    "总行数: {}, ERROR: {}, WARN: {}, INFO: {}, DEBUG: {}",
                    parts.first().unwrap_or(&0),
                    parts.get(1).unwrap_or(&0),
                    parts.get(2).unwrap_or(&0),
                    parts.get(3).unwrap_or(&0),
                    parts.get(4).unwrap_or(&0)
                ),
                ..Default::default()
            },
        ))
    }

    /// 日志健康检查
    async fn health_check(
        executor: &TaskExecutor,
        host: &str,
        paths: &[String],
        error_threshold_pct: f64,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        let mut issues = Vec::new();
        let mut total_errors = 0u64;
        let mut total_lines = 0u64;

        for path in paths {
            let cmd = format!("wc -l {} 2>/dev/null | awk '{{print $1}}'", path);
            cmds.push(cmd.clone());

            let line_result = executor.execute_command(&[host.to_string()], &cmd).await?;
            let line_count: u64 = line_result
                .host_results
                .first()
                .map(|h| h.stdout.trim().parse().unwrap_or(0))
                .unwrap_or(0);

            let err_cmd = format!(
                "grep -ci 'ERROR\\|CRITICAL\\|FATAL' {} 2>/dev/null || echo 0",
                path
            );
            cmds.push(err_cmd.clone());

            let err_result = executor
                .execute_command(&[host.to_string()], &err_cmd)
                .await?;
            let err_count: u64 = err_result
                .host_results
                .first()
                .map(|h| h.stdout.trim().parse().unwrap_or(0))
                .unwrap_or(0);

            total_lines += line_count;
            total_errors += err_count;

            if line_count > 0 {
                let error_rate = (err_count as f64 / line_count as f64) * 100.0;
                if error_rate > error_threshold_pct {
                    issues.push(LogIssue {
                        severity: "HIGH".to_string(),
                        path: path.clone(),
                        description: format!(
                            "错误率 {:.1}% 超过阈值 {:.1}% ({} 错误/{} 总行)",
                            error_rate, error_threshold_pct, err_count, line_count
                        ),
                        suggestion: "检查应用日志，排查错误根因".to_string(),
                    });
                }
            }

            // 检查文件是否可写
            let writable_cmd = format!("test -w {} && echo yes || echo no", path);
            cmds.push(writable_cmd.clone());
            let writable_result = executor
                .execute_command(&[host.to_string()], &writable_cmd)
                .await?;
            let is_writable = writable_result
                .host_results
                .first()
                .map(|h| h.stdout.trim() == "yes")
                .unwrap_or(false);

            if !is_writable {
                issues.push(LogIssue {
                    severity: "MEDIUM".to_string(),
                    path: path.clone(),
                    description: "日志文件不可写".to_string(),
                    suggestion: "检查文件权限和磁盘空间".to_string(),
                });
            }
        }

        let error_rate = if total_lines > 0 {
            (total_errors as f64 / total_lines as f64) * 100.0
        } else {
            0.0
        };

        let overall_status = if issues.iter().any(|i| i.severity == "HIGH") {
            "CRITICAL".to_string()
        } else if !issues.is_empty() {
            "WARNING".to_string()
        } else {
            "HEALTHY".to_string()
        };

        let health = LogHealth {
            overall_status: overall_status.clone(),
            error_rate_pct: (error_rate * 100.0).round() / 100.0,
            issues,
            checked_paths: paths.to_vec(),
        };

        let desc = format!(
            "日志健康检查 {} 个文件 状态={} 错误率={:.2}%",
            paths.len(),
            overall_status,
            error_rate
        );

        Ok((
            desc,
            LogManageResult {
                action: "HealthCheck".to_string(),
                success: true,
                health: Some(health),
                output: format!(
                    "状态: {}, 错误率: {:.2}%, 检查 {} 个文件",
                    overall_status,
                    error_rate,
                    paths.len()
                ),
                ..Default::default()
            },
        ))
    }

    /// 尾部跟踪
    async fn tail_logs(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        lines: u32,
        filter: Option<&str>,
        cmds: &mut Vec<String>,
    ) -> Result<(String, LogManageResult)> {
        let cmd = if let Some(f) = filter {
            format!(
                "tail -n {} {} 2>/dev/null | grep '{}'",
                lines,
                path,
                f.replace('\'', "'\\''")
            )
        } else {
            format!("tail -n {} {} 2>/dev/null", lines, path)
        };
        cmds.push(cmd.clone());

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let entries: Vec<LogEntry> = output
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(Self::parse_log_line)
            .collect();

        let count = entries.len();
        let filter_desc = filter.map(|f| format!(" 过滤='{}'", f)).unwrap_or_default();
        let desc = format!(
            "尾部跟踪 {} 最近 {} 行{} 返回 {} 条",
            path, lines, filter_desc, count
        );

        Ok((
            desc,
            LogManageResult {
                action: "Tail".to_string(),
                success: true,
                entries,
                output: format!("尾部 {} 行, 匹配 {} 条", lines, count),
                ..Default::default()
            },
        ))
    }

    /// 解析单行日志（尝试提取时间戳、级别、来源、消息）
    fn parse_log_line(line: &str) -> Option<LogEntry> {
        if line.trim().is_empty() {
            return None;
        }

        // syslog 格式: "Jan  1 12:00:00 hostname service[pid]: message"
        // 通用格式: "2024-01-01 12:00:00 [ERROR] source - message"
        let (timestamp, level, source, message) = if line.contains("ERROR")
            || line.contains("error")
            || line.contains("CRITICAL")
            || line.contains("FATAL")
        {
            Self::extract_fields(line, "ERROR")
        } else if line.contains("WARN") || line.contains("warn") || line.contains("Warning") {
            Self::extract_fields(line, "WARN")
        } else if line.contains("INFO") || line.contains("info") {
            Self::extract_fields(line, "INFO")
        } else if line.contains("DEBUG") || line.contains("debug") {
            Self::extract_fields(line, "DEBUG")
        } else {
            Self::extract_fields(line, "INFO")
        };

        Some(LogEntry {
            timestamp,
            level,
            source,
            message,
            raw_line: line.to_string(),
        })
    }

    /// 从日志行中提取字段
    fn extract_fields(line: &str, level: &str) -> (String, String, String, String) {
        // 尝试提取时间戳（前20个字符）
        let timestamp = line.chars().take(20).collect::<String>();
        let timestamp = timestamp
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");

        // 提取消息（去掉时间戳和级别标记后的内容）
        let message = line
            .split_once(level)
            .map(|(_, rest)| rest.trim_start_matches(&[':', ' ', ']'][..]))
            .unwrap_or(line)
            .to_string();

        // 尝试提取来源（在级别标记前的部分）
        let source = line
            .split_once(level)
            .map(|(before, _)| {
                before
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_matches(|c: char| c == '[' || c == ']' || c == ':')
                    .to_string()
            })
            .unwrap_or_default();

        (timestamp, level.to_string(), source, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === LogAction 构造测试 ===

    #[test]
    fn test_log_action_search() {
        let action = LogAction::Search {
            path: "/var/log/syslog".to_string(),
            pattern: "error".to_string(),
            max_results: 100,
        };
        assert!(matches!(action, LogAction::Search { .. }));
        if let LogAction::Search {
            path,
            pattern,
            max_results,
        } = action
        {
            assert_eq!(path, "/var/log/syslog");
            assert_eq!(pattern, "error");
            assert_eq!(max_results, 100);
        }
    }

    #[test]
    fn test_log_action_parse() {
        let action = LogAction::Parse {
            path: "/var/log/auth.log".to_string(),
            lines: 500,
        };
        assert!(matches!(action, LogAction::Parse { .. }));
    }

    #[test]
    fn test_log_action_rotation_check() {
        let action = LogAction::RotationCheck {
            path: "/var/log/syslog".to_string(),
        };
        assert!(matches!(action, LogAction::RotationCheck { .. }));
    }

    #[test]
    fn test_log_action_stats() {
        let action = LogAction::Stats {
            path: "/var/log/syslog".to_string(),
            since_hours: 24,
        };
        assert!(matches!(action, LogAction::Stats { .. }));
    }

    #[test]
    fn test_log_action_health_check() {
        let action = LogAction::HealthCheck {
            paths: vec![
                "/var/log/syslog".to_string(),
                "/var/log/auth.log".to_string(),
            ],
            error_threshold_pct: 5.0,
        };
        assert!(matches!(action, LogAction::HealthCheck { .. }));
    }

    #[test]
    fn test_log_action_tail() {
        let action = LogAction::Tail {
            path: "/var/log/syslog".to_string(),
            lines: 100,
            filter: Some("error".to_string()),
        };
        assert!(matches!(action, LogAction::Tail { .. }));
    }

    #[test]
    fn test_log_action_tail_no_filter() {
        let action = LogAction::Tail {
            path: "/var/log/syslog".to_string(),
            lines: 50,
            filter: None,
        };
        if let LogAction::Tail { filter, .. } = action {
            assert!(filter.is_none());
        }
    }

    // === LogManageResult 默认值测试 ===

    #[test]
    fn test_log_manage_result_default() {
        let result = LogManageResult::default();
        assert!(result.success);
        assert!(result.entries.is_empty());
        assert!(result.stats.is_none());
        assert!(result.rotation_info.is_none());
        assert!(result.health.is_none());
        assert!(result.errors.is_empty());
    }

    // === LogEntry 测试 ===

    #[test]
    fn test_log_entry_fields() {
        let entry = LogEntry {
            timestamp: "Jan  1 12:00:00".to_string(),
            level: "ERROR".to_string(),
            source: "sshd".to_string(),
            message: "Failed password".to_string(),
            raw_line: "Jan  1 12:00:00 server sshd[1234]: Failed password".to_string(),
        };
        assert_eq!(entry.level, "ERROR");
        assert_eq!(entry.source, "sshd");
        assert!(entry.message.contains("Failed password"));
    }

    #[test]
    fn test_log_entry_serialization_roundtrip() {
        let entry = LogEntry {
            timestamp: "2024-01-01".to_string(),
            level: "WARN".to_string(),
            source: "nginx".to_string(),
            message: "upstream timeout".to_string(),
            raw_line: "2024-01-01 [WARN] nginx - upstream timeout".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    // === LogStats 测试 ===

    #[test]
    fn test_log_stats_fields() {
        let stats = LogStats {
            total_lines: 1000,
            error_count: 10,
            warn_count: 50,
            info_count: 800,
            debug_count: 140,
            by_source: vec![("nginx".to_string(), 500), ("sshd".to_string(), 500)],
            time_range: "最近 24 小时".to_string(),
        };
        assert_eq!(stats.total_lines, 1000);
        assert_eq!(stats.error_count, 10);
        assert_eq!(stats.by_source.len(), 2);
    }

    // === LogRotationInfo 测试 ===

    #[test]
    fn test_log_rotation_info() {
        let info = LogRotationInfo {
            log_path: "/var/log/syslog".to_string(),
            current_size_mb: 150.5,
            rotated_files: vec!["syslog.1".to_string(), "syslog.2.gz".to_string()],
            rotation_config: "daily rotate 7".to_string(),
            needs_rotation: true,
        };
        assert!(info.needs_rotation);
        assert_eq!(info.rotated_files.len(), 2);
        assert!(info.current_size_mb > 100.0);
    }

    #[test]
    fn test_log_rotation_info_no_rotation_needed() {
        let info = LogRotationInfo {
            log_path: "/var/log/auth.log".to_string(),
            current_size_mb: 10.0,
            rotated_files: vec![],
            rotation_config: "weekly rotate 4".to_string(),
            needs_rotation: false,
        };
        assert!(!info.needs_rotation);
    }

    // === LogHealth 测试 ===

    #[test]
    fn test_log_health_healthy() {
        let health = LogHealth {
            overall_status: "HEALTHY".to_string(),
            error_rate_pct: 0.5,
            issues: vec![],
            checked_paths: vec!["/var/log/syslog".to_string()],
        };
        assert_eq!(health.overall_status, "HEALTHY");
        assert!(health.issues.is_empty());
    }

    #[test]
    fn test_log_health_critical() {
        let health = LogHealth {
            overall_status: "CRITICAL".to_string(),
            error_rate_pct: 15.0,
            issues: vec![LogIssue {
                severity: "HIGH".to_string(),
                path: "/var/log/app.log".to_string(),
                description: "错误率 15.0% 超过阈值 5.0%".to_string(),
                suggestion: "检查应用日志".to_string(),
            }],
            checked_paths: vec!["/var/log/app.log".to_string()],
        };
        assert_eq!(health.overall_status, "CRITICAL");
        assert_eq!(health.issues.len(), 1);
        assert_eq!(health.issues[0].severity, "HIGH");
    }

    // === LogIssue 测试 ===

    #[test]
    fn test_log_issue_fields() {
        let issue = LogIssue {
            severity: "HIGH".to_string(),
            path: "/var/log/syslog".to_string(),
            description: "错误率过高".to_string(),
            suggestion: "检查系统日志".to_string(),
        };
        assert_eq!(issue.severity, "HIGH");
        assert!(!issue.description.is_empty());
        assert!(!issue.suggestion.is_empty());
    }

    // === parse_log_line 测试 ===

    #[test]
    fn test_parse_log_line_syslog_format() {
        let line = "Jan  1 12:00:00 server sshd[1234]: Failed password for root";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        // syslog 行不含 "ERROR" 关键字，默认归为 INFO
        assert_eq!(entry.level, "INFO");
        assert!(entry.raw_line.contains("Failed password"));
    }

    #[test]
    fn test_parse_log_line_error() {
        let line = "2024-01-01 10:30:00 [ERROR] database - connection refused";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, "ERROR");
    }

    #[test]
    fn test_parse_log_line_warn() {
        let line = "2024-01-01 [WARN] nginx - upstream timeout";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, "WARN");
    }

    #[test]
    fn test_parse_log_line_info() {
        let line = "2024-01-01 [INFO] app - startup complete";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, "INFO");
    }

    #[test]
    fn test_parse_log_line_debug() {
        let line = "DEBUG: processing request id=123";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, "DEBUG");
    }

    #[test]
    fn test_parse_log_line_empty() {
        let entry = LogManager::parse_log_line("");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_log_line_whitespace() {
        let entry = LogManager::parse_log_line("   ");
        assert!(entry.is_none());
    }

    #[test]
    fn test_parse_log_line_generic() {
        let line = "some random log line without level markers";
        let entry = LogManager::parse_log_line(line);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().level, "INFO"); // default level
    }

    // === extract_fields 测试 ===

    #[test]
    fn test_extract_fields_basic() {
        let line = "2024-01-01 12:00:00 [ERROR] myapp - something failed";
        let (ts, level, _source, msg) = LogManager::extract_fields(line, "ERROR");
        assert!(!ts.is_empty());
        assert_eq!(level, "ERROR");
        assert!(msg.contains("something failed"));
    }

    #[test]
    fn test_extract_fields_syslog() {
        let line = "Jan  1 12:00:00 server sshd[1234]: Connection closed";
        let (ts, level, _source, _msg) = LogManager::extract_fields(line, "INFO");
        assert!(ts.contains("Jan"));
        assert_eq!(level, "INFO");
    }

    // === LogAction 序列化测试 ===

    #[test]
    fn test_log_action_serialization() {
        let action = LogAction::Search {
            path: "/var/log/syslog".to_string(),
            pattern: "error".to_string(),
            max_results: 100,
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: LogAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_log_action_health_check_serialization() {
        let action = LogAction::HealthCheck {
            paths: vec!["/var/log/syslog".to_string()],
            error_threshold_pct: 5.0,
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: LogAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    // === LogManageResult 序列化测试 ===

    #[test]
    fn test_log_manage_result_serialization() {
        let result = LogManageResult {
            action: "Search".to_string(),
            success: true,
            entries: vec![LogEntry {
                timestamp: "2024-01-01".to_string(),
                level: "ERROR".to_string(),
                source: "app".to_string(),
                message: "test".to_string(),
                raw_line: "test line".to_string(),
            }],
            stats: None,
            rotation_info: None,
            health: None,
            output: "found 1".to_string(),
            errors: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: LogManageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.action, deserialized.action);
        assert_eq!(result.entries.len(), deserialized.entries.len());
    }

    // === 边界情况测试 ===

    #[test]
    fn test_log_stats_zero_lines() {
        let stats = LogStats {
            total_lines: 0,
            error_count: 0,
            warn_count: 0,
            info_count: 0,
            debug_count: 0,
            by_source: vec![],
            time_range: "最近 1 小时".to_string(),
        };
        assert_eq!(stats.total_lines, 0);
    }

    #[test]
    fn test_log_health_warning_status() {
        let health = LogHealth {
            overall_status: "WARNING".to_string(),
            error_rate_pct: 3.0,
            issues: vec![LogIssue {
                severity: "MEDIUM".to_string(),
                path: "/var/log/app.log".to_string(),
                description: "日志文件不可写".to_string(),
                suggestion: "检查文件权限".to_string(),
            }],
            checked_paths: vec!["/var/log/app.log".to_string()],
        };
        assert_eq!(health.overall_status, "WARNING");
        assert_eq!(health.issues[0].severity, "MEDIUM");
    }

    #[test]
    fn test_log_action_all_variants() {
        let actions = [
            LogAction::Search {
                path: "/var/log/syslog".to_string(),
                pattern: "error".to_string(),
                max_results: 100,
            },
            LogAction::Parse {
                path: "/var/log/syslog".to_string(),
                lines: 500,
            },
            LogAction::RotationCheck {
                path: "/var/log/syslog".to_string(),
            },
            LogAction::Stats {
                path: "/var/log/syslog".to_string(),
                since_hours: 24,
            },
            LogAction::HealthCheck {
                paths: vec!["/var/log/syslog".to_string()],
                error_threshold_pct: 5.0,
            },
            LogAction::Tail {
                path: "/var/log/syslog".to_string(),
                lines: 100,
                filter: None,
            },
        ];
        assert_eq!(actions.len(), 6);
    }
}
