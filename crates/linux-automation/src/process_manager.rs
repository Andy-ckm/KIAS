//! R036: 进程管理模块 — 进程列表/详情/终止/优先级/僵尸清理/进程树
//!
//! 完整的进程生命周期管理：发现、监控、控制、清理
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: htop/top/ps/kill/renice, systemd-cgtop, procps-ng
//! AgentGuard差异化: 进程发现→资源分析→安全终止→僵尸清理→审计（竞品只展示不管理）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 进程管理引擎
///
/// 提供完整的进程管理：列表/详情/终止/优先级/僵尸清理/进程树/Top N
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct ProcessManager;

impl ProcessManager {
    /// 执行进程管理操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &ProcessAction,
        audit: &AuditLog,
    ) -> Result<ProcessOpsResult> {
        let _audit_id = uuid::Uuid::new_v4().to_string();
        let mut commands_executed = Vec::new();

        let (action_desc, result) = match action {
            ProcessAction::List {
                user,
                sort_by,
                filter,
                limit,
            } => {
                Self::list_processes(
                    executor,
                    host,
                    user,
                    sort_by,
                    filter,
                    limit,
                    &mut commands_executed,
                )
                .await?
            }
            ProcessAction::Detail { pid } => {
                Self::process_detail(executor, host, *pid, &mut commands_executed).await?
            }
            ProcessAction::Kill { pid, signal } => {
                Self::kill_process(executor, host, *pid, signal, &mut commands_executed).await?
            }
            ProcessAction::KillByName {
                name_pattern,
                signal,
            } => {
                Self::kill_by_name(executor, host, name_pattern, signal, &mut commands_executed)
                    .await?
            }
            ProcessAction::SetPriority { pid, nice } => {
                Self::set_priority(executor, host, *pid, *nice, &mut commands_executed).await?
            }
            ProcessAction::Tree { root_pid } => {
                Self::process_tree(executor, host, root_pid, &mut commands_executed).await?
            }
            ProcessAction::CleanZombies => {
                Self::clean_zombies(executor, host, &mut commands_executed).await?
            }
            ProcessAction::Top { sort_by, limit } => {
                Self::top_processes(executor, host, sort_by, *limit, &mut commands_executed).await?
            }
        };

        // 审计日志
        let _ = audit.log_action(
            "system",
            "ProcessManager",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&commands_executed).unwrap_or_default(),
        );

        info!(
            host = host,
            action = action_desc,
            success = result.success,
            "进程管理操作完成"
        );

        Ok(result)
    }

    /// 列出进程
    async fn list_processes(
        executor: &TaskExecutor,
        host: &str,
        user: &Option<String>,
        sort_by: &ProcessSortField,
        filter: &Option<String>,
        limit: &Option<u32>,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        let sort_flag = match sort_by {
            ProcessSortField::Cpu => "--sort=-%cpu",
            ProcessSortField::Memory => "--sort=-%mem",
            ProcessSortField::Pid => "--sort=pid",
            ProcessSortField::Name => "--sort=comm",
            ProcessSortField::StartTime => "--sort=start_time",
        };

        let user_flag = match user {
            Some(u) => format!("--user={}", u),
            None => String::new(),
        };

        let limit_val = limit.unwrap_or(50);
        let cmd = format!(
            "ps {} {} -eo pid,ppid,user,%cpu,%mem,rss,stat,lstart,comm --no-headers | head -{}",
            sort_flag, user_flag, limit_val
        );
        cmds.push(cmd.clone());

        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let processes = Self::parse_process_list(&output, filter);

        Ok((
            format!(
                "列出进程(sort={:?}, user={:?}, limit={})",
                sort_by, user, limit_val
            ),
            ProcessOpsResult {
                action: "List".to_string(),
                success: true,
                processes,
                detail: None,
                tree: None,
                output,
                errors: vec![],
            },
        ))
    }

    /// 查看进程详情
    async fn process_detail(
        executor: &TaskExecutor,
        host: &str,
        pid: u32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        // 获取基本信息
        let info_cmd = format!(
            "ps -p {} -o pid,ppid,user,stat,nlwp,rss,vsz,%cpu,lstart,comm --no-headers",
            pid
        );
        cmds.push(info_cmd.clone());

        let info_result = executor
            .execute_command(&[host.to_string()], &info_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let info_output = info_result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        if info_output.trim().is_empty() {
            return Ok((
                format!("查看进程详情: PID {}", pid),
                ProcessOpsResult {
                    action: "Detail".to_string(),
                    success: false,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: String::new(),
                    errors: vec![format!("进程 {} 不存在", pid)],
                },
            ));
        }

        // 获取打开文件
        let files_cmd = format!("ls -la /proc/{}/fd 2>/dev/null | head -20", pid);
        cmds.push(files_cmd.clone());
        let files_result = executor
            .execute_command(&[host.to_string()], &files_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let open_files: Vec<String> = files_result
            .host_results
            .first()
            .map(|h| {
                h.stdout
                    .lines()
                    .skip(1) // skip "total" line
                    .map(|l| l.to_string())
                    .collect()
            })
            .unwrap_or_default();

        // 获取网络连接
        let net_cmd = format!("ss -tlnp 2>/dev/null | grep 'pid={}'", pid);
        cmds.push(net_cmd.clone());
        let net_result = executor
            .execute_command(&[host.to_string()], &net_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let connections: Vec<String> = net_result
            .host_results
            .first()
            .map(|h| h.stdout.lines().map(|l| l.to_string()).collect())
            .unwrap_or_default();

        // 获取命令行
        let cmdline_cmd = format!("cat /proc/{}/cmdline 2>/dev/null | tr '\\0' ' '", pid);
        cmds.push(cmdline_cmd.clone());
        let cmdline_result = executor
            .execute_command(&[host.to_string()], &cmdline_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let command_line = cmdline_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        // 获取工作目录
        let cwd_cmd = format!("readlink /proc/{}/cwd 2>/dev/null", pid);
        cmds.push(cwd_cmd.clone());
        let cwd_result = executor
            .execute_command(&[host.to_string()], &cwd_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let working_dir = cwd_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        // 获取可执行文件
        let exe_cmd = format!("readlink /proc/{}/exe 2>/dev/null", pid);
        cmds.push(exe_cmd.clone());
        let exe_result = executor
            .execute_command(&[host.to_string()], &exe_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let executable = exe_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        // 解析基本信息
        let fields: Vec<&str> = info_output.split_whitespace().collect();
        let detail = ProcessDetail {
            pid,
            name: fields.last().unwrap_or(&"unknown").to_string(),
            user: fields.get(2).unwrap_or(&"unknown").to_string(),
            state: fields.get(3).unwrap_or(&"unknown").to_string(),
            threads: fields.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
            mem_rss_kb: fields.get(5).and_then(|s| s.parse().ok()).unwrap_or(0),
            mem_vsz_kb: fields.get(6).and_then(|s| s.parse().ok()).unwrap_or(0),
            cpu_percent: fields.get(7).and_then(|s| s.parse().ok()).unwrap_or(0.0),
            open_files,
            connections,
            environment: vec![], // 环境变量需要特殊权限
            command_line,
            working_dir,
            executable,
        };

        Ok((
            format!("查看进程详情: PID {}", pid),
            ProcessOpsResult {
                action: "Detail".to_string(),
                success: true,
                processes: vec![],
                detail: Some(detail),
                tree: None,
                output: info_output,
                errors: vec![],
            },
        ))
    }

    /// 终止进程
    async fn kill_process(
        executor: &TaskExecutor,
        host: &str,
        pid: u32,
        signal: &ProcessSignal,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        let sig_flag = Self::signal_to_flag(signal);

        // 先检查进程是否存在
        let check_cmd = format!("ps -p {} -o comm= 2>/dev/null", pid);
        cmds.push(check_cmd.clone());
        let check_result = executor
            .execute_command(&[host.to_string()], &check_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let proc_name = check_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        if proc_name.is_empty() {
            return Ok((
                format!("终止进程: PID {} (不存在)", pid),
                ProcessOpsResult {
                    action: "Kill".to_string(),
                    success: false,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: String::new(),
                    errors: vec![format!("进程 {} 不存在", pid)],
                },
            ));
        }

        let cmd = format!("kill -{} {}", sig_flag, pid);
        cmds.push(cmd.clone());
        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let exit_code = result
            .host_results
            .first()
            .map(|h| h.exit_code)
            .unwrap_or(-1);

        // 验证进程是否已终止
        if matches!(signal, ProcessSignal::Term | ProcessSignal::Kill) {
            let verify_cmd = format!("ps -p {} -o comm= 2>/dev/null", pid);
            cmds.push(verify_cmd.clone());
            let verify_result = executor
                .execute_command(&[host.to_string()], &verify_cmd)
                .await
                .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
            let still_alive = verify_result
                .host_results
                .first()
                .map(|h| !h.stdout.trim().is_empty())
                .unwrap_or(false);

            if still_alive && matches!(signal, ProcessSignal::Term) {
                return Ok((
                    format!(
                        "终止进程: PID {} {} (SIGTERM 未生效，进程仍存活)",
                        pid, proc_name
                    ),
                    ProcessOpsResult {
                        action: "Kill".to_string(),
                        success: false,
                        processes: vec![],
                        detail: None,
                        tree: None,
                        output: String::new(),
                        errors: vec![format!(
                            "进程 {} ({}) 收到 SIGTERM 后仍存活，建议使用 SIGKILL",
                            pid, proc_name
                        )],
                    },
                ));
            }
        }

        Ok((
            format!("终止进程: PID {} {} (signal={:?})", pid, proc_name, signal),
            ProcessOpsResult {
                action: "Kill".to_string(),
                success: exit_code == 0,
                processes: vec![],
                detail: None,
                tree: None,
                output: String::new(),
                errors: if exit_code != 0 {
                    vec![format!("kill 命令返回 {}", exit_code)]
                } else {
                    vec![]
                },
            },
        ))
    }

    /// 按名称模式批量终止进程
    async fn kill_by_name(
        executor: &TaskExecutor,
        host: &str,
        name_pattern: &str,
        signal: &ProcessSignal,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        let sig_flag = Self::signal_to_flag(signal);

        // 查找匹配的进程
        let find_cmd = format!(
            "pgrep -f '{}' 2>/dev/null",
            name_pattern.replace('\'', "'\\''")
        );
        cmds.push(find_cmd.clone());
        let find_result = executor
            .execute_command(&[host.to_string()], &find_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let pids_output = find_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        if pids_output.is_empty() {
            return Ok((
                format!("按名称终止: '{}' (未找到匹配进程)", name_pattern),
                ProcessOpsResult {
                    action: "KillByName".to_string(),
                    success: true,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: "未找到匹配进程".to_string(),
                    errors: vec![],
                },
            ));
        }

        let cmd = format!(
            "pkill -{} -f '{}'",
            sig_flag,
            name_pattern.replace('\'', "'\\''")
        );
        cmds.push(cmd.clone());
        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let exit_code = result
            .host_results
            .first()
            .map(|h| h.exit_code)
            .unwrap_or(-1);

        let pid_count = pids_output.lines().count();

        Ok((
            format!(
                "按名称终止: '{}' ({} 个进程, signal={:?})",
                name_pattern, pid_count, signal
            ),
            ProcessOpsResult {
                action: "KillByName".to_string(),
                success: exit_code == 0,
                processes: vec![],
                detail: None,
                tree: None,
                output: pids_output,
                errors: if exit_code != 0 {
                    vec![format!("pkill 命令返回 {}", exit_code)]
                } else {
                    vec![]
                },
            },
        ))
    }

    /// 设置进程优先级
    async fn set_priority(
        executor: &TaskExecutor,
        host: &str,
        pid: u32,
        nice: i32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        // 验证 nice 值范围
        if !(-20..=19).contains(&nice) {
            return Ok((
                format!("设置优先级: PID {} nice={} (值超出范围)", pid, nice),
                ProcessOpsResult {
                    action: "SetPriority".to_string(),
                    success: false,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: String::new(),
                    errors: vec![format!("nice 值 {} 超出范围 [-20, 19]", nice)],
                },
            ));
        }

        // 先检查进程是否存在
        let check_cmd = format!("ps -p {} -o comm= 2>/dev/null", pid);
        cmds.push(check_cmd.clone());
        let check_result = executor
            .execute_command(&[host.to_string()], &check_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let proc_name = check_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        if proc_name.is_empty() {
            return Ok((
                format!("设置优先级: PID {} (不存在)", pid),
                ProcessOpsResult {
                    action: "SetPriority".to_string(),
                    success: false,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: String::new(),
                    errors: vec![format!("进程 {} 不存在", pid)],
                },
            ));
        }

        let cmd = format!("renice {} -p {}", nice, pid);
        cmds.push(cmd.clone());
        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();
        let exit_code = result
            .host_results
            .first()
            .map(|h| h.exit_code)
            .unwrap_or(-1);

        Ok((
            format!("设置优先级: PID {} {} nice={}", pid, proc_name, nice),
            ProcessOpsResult {
                action: "SetPriority".to_string(),
                success: exit_code == 0,
                processes: vec![],
                detail: None,
                tree: None,
                output,
                errors: if exit_code != 0 {
                    vec![format!("renice 命令返回 {}", exit_code)]
                } else {
                    vec![]
                },
            },
        ))
    }

    /// 查看进程树
    async fn process_tree(
        executor: &TaskExecutor,
        host: &str,
        root_pid: &Option<u32>,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        let cmd = match root_pid {
            Some(pid) => format!("pstree -p -s {} 2>/dev/null || ps --forest -p {}", pid, pid),
            None => "pstree -p 2>/dev/null | head -100".to_string(),
        };
        cmds.push(cmd.clone());

        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let desc = match root_pid {
            Some(pid) => format!("查看进程树: PID {}", pid),
            None => "查看进程树: 全系统".to_string(),
        };

        Ok((
            desc,
            ProcessOpsResult {
                action: "Tree".to_string(),
                success: !output.trim().is_empty(),
                processes: vec![],
                detail: None,
                tree: Some(output.clone()),
                output,
                errors: vec![],
            },
        ))
    }

    /// 清理僵尸进程
    async fn clean_zombies(
        executor: &TaskExecutor,
        host: &str,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        // 查找僵尸进程
        let find_cmd = "ps aux | awk '$8 ~ /Z/ {print $2, $11}'".to_string();
        cmds.push(find_cmd.clone());
        let find_result = executor
            .execute_command(&[host.to_string()], &find_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let zombies_output = find_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();

        if zombies_output.is_empty() {
            return Ok((
                "清理僵尸进程: 无僵尸进程".to_string(),
                ProcessOpsResult {
                    action: "CleanZombies".to_string(),
                    success: true,
                    processes: vec![],
                    detail: None,
                    tree: None,
                    output: "无僵尸进程".to_string(),
                    errors: vec![],
                },
            ));
        }

        let zombie_count = zombies_output.lines().count();
        let mut errors = Vec::new();
        let mut cleaned = 0;

        // 尝试向僵尸进程的父进程发送 SIGCHLD
        for line in zombies_output.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if let Some(zombie_pid) = fields.first() {
                // 获取僵尸进程的父进程 PID
                let ppid_cmd = format!("ps -o ppid= -p {} 2>/dev/null", zombie_pid);
                cmds.push(ppid_cmd.clone());
                let ppid_result = executor
                    .execute_command(&[host.to_string()], &ppid_cmd)
                    .await
                    .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
                let ppid = ppid_result
                    .host_results
                    .first()
                    .map(|h| h.stdout.trim().to_string())
                    .unwrap_or_default();

                if !ppid.is_empty() && ppid != "1" {
                    // 向父进程发送 SIGCHLD 通知回收
                    let sigchld_cmd = format!("kill -SIGCHLD {} 2>/dev/null", ppid);
                    cmds.push(sigchld_cmd.clone());
                    let _ = executor
                        .execute_command(&[host.to_string()], &sigchld_cmd)
                        .await;
                    cleaned += 1;
                } else if ppid == "1" {
                    errors.push(format!(
                        "僵尸进程 {} 的父进程是 init (PID 1)，需要重启系统",
                        zombie_pid
                    ));
                }
            }
        }

        // 验证清理结果
        let verify_cmd = "ps aux | awk '$8 ~ /Z/' | wc -l".to_string();
        cmds.push(verify_cmd.clone());
        let verify_result = executor
            .execute_command(&[host.to_string()], &verify_cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let remaining: u32 = verify_result
            .host_results
            .first()
            .map(|h| h.stdout.trim().parse().unwrap_or(0))
            .unwrap_or(0);

        let output = format!(
            "发现 {} 个僵尸进程, 尝试清理 {} 个, 剩余 {} 个",
            zombie_count, cleaned, remaining
        );

        Ok((
            format!("清理僵尸进程: {} 个", zombie_count),
            ProcessOpsResult {
                action: "CleanZombies".to_string(),
                success: remaining == 0,
                processes: vec![],
                detail: None,
                tree: None,
                output,
                errors,
            },
        ))
    }

    /// 查看资源使用 Top N
    async fn top_processes(
        executor: &TaskExecutor,
        host: &str,
        sort_by: &ProcessSortField,
        limit: u32,
        cmds: &mut Vec<String>,
    ) -> Result<(String, ProcessOpsResult)> {
        let sort_flag = match sort_by {
            ProcessSortField::Cpu => "--sort=-%cpu",
            ProcessSortField::Memory => "--sort=-%mem",
            ProcessSortField::Pid => "--sort=pid",
            ProcessSortField::Name => "--sort=comm",
            ProcessSortField::StartTime => "--sort=start_time",
        };

        let cmd = format!(
            "ps {} -eo pid,ppid,user,%cpu,%mem,rss,stat,lstart,comm --no-headers | head -{}",
            sort_flag, limit
        );
        cmds.push(cmd.clone());

        let result = executor
            .execute_command(&[host.to_string()], &cmd)
            .await
            .map_err(|e| AutomationError::ProcessOperation(e.to_string()))?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let processes = Self::parse_process_list(&output, &None);

        Ok((
            format!("Top {} 进程(sort={:?})", limit, sort_by),
            ProcessOpsResult {
                action: "Top".to_string(),
                success: true,
                processes,
                detail: None,
                tree: None,
                output,
                errors: vec![],
            },
        ))
    }

    // ========== 辅助方法 ==========

    /// 解析 ps 输出为 ProcessInfo 列表
    fn parse_process_list(output: &str, filter: &Option<String>) -> Vec<ProcessInfo> {
        output
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() < 9 {
                    return None;
                }

                let name = fields[8..].join(" "); // 名称可能包含空格
                let pid: u32 = fields[0].parse().ok()?;
                let ppid: u32 = fields[1].parse().ok()?;

                // 应用过滤器
                if let Some(ref pattern) = filter {
                    let pattern_lower = pattern.to_lowercase();
                    if !name.to_lowercase().contains(&pattern_lower)
                        && !fields[2].to_lowercase().contains(&pattern_lower)
                    {
                        return None;
                    }
                }

                Some(ProcessInfo {
                    pid,
                    ppid,
                    name,
                    user: fields[2].to_string(),
                    cpu_percent: fields[3].parse().unwrap_or(0.0),
                    mem_percent: fields[4].parse().unwrap_or(0.0),
                    mem_rss_kb: fields[5].parse().unwrap_or(0),
                    state: fields[6].to_string(),
                    started: fields[7].to_string(),
                    command: fields[8..].join(" "),
                })
            })
            .collect()
    }

    /// 信号转 kill 标志
    fn signal_to_flag(signal: &ProcessSignal) -> &'static str {
        match signal {
            ProcessSignal::Term => "15",
            ProcessSignal::Kill => "9",
            ProcessSignal::Hup => "1",
            ProcessSignal::Usr1 => "10",
            ProcessSignal::Usr2 => "12",
            ProcessSignal::Stop => "19",
            ProcessSignal::Cont => "18",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_to_flag_term() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Term), "15");
    }

    #[test]
    fn test_signal_to_flag_kill() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Kill), "9");
    }

    #[test]
    fn test_signal_to_flag_hup() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Hup), "1");
    }

    #[test]
    fn test_signal_to_flag_usr1() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Usr1), "10");
    }

    #[test]
    fn test_signal_to_flag_usr2() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Usr2), "12");
    }

    #[test]
    fn test_signal_to_flag_stop() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Stop), "19");
    }

    #[test]
    fn test_signal_to_flag_cont() {
        assert_eq!(ProcessManager::signal_to_flag(&ProcessSignal::Cont), "18");
    }

    #[test]
    fn test_parse_process_list_valid() {
        let output = " 1234  1000 root  5.0  2.3 12345 Sl Jan01 /usr/bin/test";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 1234);
        assert_eq!(processes[0].ppid, 1000);
        assert_eq!(processes[0].user, "root");
        assert_eq!(processes[0].cpu_percent, 5.0);
        assert_eq!(processes[0].mem_percent, 2.3);
        assert_eq!(processes[0].mem_rss_kb, 12345);
        assert_eq!(processes[0].name, "/usr/bin/test");
    }

    #[test]
    fn test_parse_process_list_with_filter_match() {
        let output = " 1234  1000 root  5.0  2.3 12345 Sl Jan01 nginx\n 5678  1000 www  3.0  1.0 5000 S Jan01 apache";
        let filter = Some("nginx".to_string());
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 1234);
    }

    #[test]
    fn test_parse_process_list_with_filter_user() {
        let output = " 1234  1000 root  5.0  2.3 12345 Sl Jan01 test\n 5678  1000 www  3.0  1.0 5000 S Jan01 other";
        let filter = Some("www".to_string());
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 5678);
    }

    #[test]
    fn test_parse_process_list_with_filter_no_match() {
        let output = " 1234  1000 root  5.0  2.3 12345 Sl Jan01 nginx";
        let filter = Some("nonexistent".to_string());
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert!(processes.is_empty());
    }

    #[test]
    fn test_parse_process_list_empty() {
        let output = "";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert!(processes.is_empty());
    }

    #[test]
    fn test_parse_process_list_malformed() {
        let output = "incomplete data";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert!(processes.is_empty());
    }

    #[test]
    fn test_parse_process_list_multiple() {
        let output = " 100  1 root  10.0  5.0 10000 S Jan01 proc_a\n 200  1 root  20.0  8.0 20000 R Jan01 proc_b\n 300  1 root  3.0  1.0 3000 D Jan01 proc_c";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 3);
        assert_eq!(processes[0].pid, 100);
        assert_eq!(processes[1].pid, 200);
        assert_eq!(processes[2].pid, 300);
    }

    #[test]
    fn test_parse_process_list_filter_case_insensitive() {
        let output = " 100  1 root  10.0  5.0 10000 S Jan01 Nginx\n 200  1 root  20.0  8.0 20000 R Jan01 apache";
        let filter = Some("nginx".to_string());
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].pid, 100);
    }

    #[test]
    fn test_parse_process_list_long_name_with_spaces() {
        let output = " 100  1 root  10.0  5.0 10000 S Jan01 /usr/bin/long name with spaces";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].name, "/usr/bin/long name with spaces");
    }

    #[test]
    fn test_process_action_serialization_roundtrip() {
        let actions = vec![
            ProcessAction::List {
                user: Some("root".to_string()),
                sort_by: ProcessSortField::Cpu,
                filter: None,
                limit: Some(10),
            },
            ProcessAction::Detail { pid: 1234 },
            ProcessAction::Kill {
                pid: 1234,
                signal: ProcessSignal::Term,
            },
            ProcessAction::KillByName {
                name_pattern: "nginx".to_string(),
                signal: ProcessSignal::Kill,
            },
            ProcessAction::SetPriority {
                pid: 1234,
                nice: 10,
            },
            ProcessAction::Tree { root_pid: Some(1) },
            ProcessAction::CleanZombies,
            ProcessAction::Top {
                sort_by: ProcessSortField::Memory,
                limit: 20,
            },
        ];

        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let deserialized: ProcessAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, deserialized);
        }
    }

    #[test]
    fn test_process_ops_result_default() {
        let result = ProcessOpsResult::default();
        assert!(result.success);
        assert!(result.processes.is_empty());
        assert!(result.detail.is_none());
        assert!(result.tree.is_none());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_process_info_serialization() {
        let info = ProcessInfo {
            pid: 1234,
            ppid: 1000,
            name: "test".to_string(),
            user: "root".to_string(),
            cpu_percent: 5.0,
            mem_percent: 2.3,
            mem_rss_kb: 12345,
            state: "S".to_string(),
            started: "Jan01".to_string(),
            command: "/usr/bin/test".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pid, 1234);
        assert_eq!(deserialized.name, "test");
    }

    #[test]
    fn test_process_detail_serialization() {
        let detail = ProcessDetail {
            pid: 1234,
            name: "test".to_string(),
            user: "root".to_string(),
            state: "S".to_string(),
            threads: 4,
            mem_rss_kb: 12345,
            mem_vsz_kb: 100000,
            cpu_percent: 5.0,
            open_files: vec!["file1".to_string()],
            connections: vec!["conn1".to_string()],
            environment: vec![],
            command_line: "/usr/bin/test --arg".to_string(),
            working_dir: "/tmp".to_string(),
            executable: "/usr/bin/test".to_string(),
        };
        let json = serde_json::to_string(&detail).unwrap();
        let deserialized: ProcessDetail = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pid, 1234);
        assert_eq!(deserialized.threads, 4);
        assert_eq!(deserialized.open_files.len(), 1);
    }

    #[test]
    fn test_process_sort_field_variants() {
        let variants = [
            ProcessSortField::Cpu,
            ProcessSortField::Memory,
            ProcessSortField::Pid,
            ProcessSortField::Name,
            ProcessSortField::StartTime,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let deserialized: ProcessSortField = serde_json::from_str(&json).unwrap();
            assert_eq!(v, deserialized);
        }
    }

    #[test]
    fn test_process_signal_variants() {
        let signals = [
            ProcessSignal::Term,
            ProcessSignal::Kill,
            ProcessSignal::Hup,
            ProcessSignal::Usr1,
            ProcessSignal::Usr2,
            ProcessSignal::Stop,
            ProcessSignal::Cont,
        ];
        for s in signals {
            let json = serde_json::to_string(&s).unwrap();
            let deserialized: ProcessSignal = serde_json::from_str(&json).unwrap();
            assert_eq!(s, deserialized);
        }
    }

    #[test]
    fn test_parse_process_list_zero_values() {
        let output = " 100  0 root  0.0  0.0 0 Ss Jan01 init";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert_eq!(processes.len(), 1);
        assert_eq!(processes[0].cpu_percent, 0.0);
        assert_eq!(processes[0].mem_percent, 0.0);
        assert_eq!(processes[0].mem_rss_kb, 0);
    }

    #[test]
    fn test_parse_process_list_bad_numeric() {
        let output = " abc  xyz root  NaN  bad  -  S Jan01 test";
        let filter = None;
        let processes = ProcessManager::parse_process_list(output, &filter);
        assert!(processes.is_empty()); // parse fails on non-numeric pid
    }
}
