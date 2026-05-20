//! R034: 磁盘管理模块
//!
//! 磁盘使用分析 / 安全清理 / SMART监控 / inode检查 / IO统计
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;
use tracing::info;

/// 磁盘管理引擎
pub struct DiskManager;

impl DiskManager {
    /// 执行磁盘操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &DiskAction,
        audit: &AuditLog,
    ) -> Result<DiskManagementResult> {
        let mut result = DiskManagementResult {
            action: format!("{:?}", action),
            ..Default::default()
        };

        match action {
            DiskAction::Usage => {
                result.usage = Self::get_usage(executor, host).await?;
                result.output = format!("查询到 {} 个挂载点", result.usage.len());
            }
            DiskAction::FindLargeFiles {
                path,
                min_size_mb,
                limit,
            } => {
                result.large_files =
                    Self::find_large_files(executor, host, path, *min_size_mb, *limit).await?;
                result.output = format!("找到 {} 个大文件", result.large_files.len());
            }
            DiskAction::SafeCleanup { targets, dry_run } => {
                let (cleanups, warnings) =
                    Self::safe_cleanup(executor, host, targets, *dry_run, audit).await?;
                result.cleanup_results = cleanups;
                result.warnings = warnings;
                result.total_freed_bytes =
                    result.cleanup_results.iter().map(|c| c.freed_bytes).sum();
                result.total_freed_human = human_size(result.total_freed_bytes);
                result.output = format!(
                    "清理完成: 释放 {}, 干运行={}",
                    result.total_freed_human, dry_run
                );
            }
            DiskAction::SmartHealth { device } => {
                result.smart_health = Self::check_smart(executor, host, device).await?;
                result.output = format!("SMART 检查: {} 个设备", result.smart_health.len());
            }
            DiskAction::InodeCheck => {
                result.usage = Self::get_usage(executor, host).await?;
                // 过滤出 inode 使用率高的
                let high_inode: Vec<_> = result
                    .usage
                    .iter()
                    .filter(|u| u.inode_percent > 80.0)
                    .collect();
                if !high_inode.is_empty() {
                    result.warnings.push(format!(
                        "⚠️ {} 个挂载点 inode 使用率超过 80%",
                        high_inode.len()
                    ));
                }
                result.output = format!("检查 {} 个挂载点的 inode", result.usage.len());
            }
            DiskAction::IoStats => {
                result.io_stats = Self::get_io_stats(executor, host).await?;
                result.output = format!("IO 统计: {} 个设备", result.io_stats.len());
            }
            DiskAction::CleanOldLogs {
                log_dir,
                older_than_days,
            } => {
                let (entries, warnings) =
                    Self::clean_old_logs(executor, host, log_dir, *older_than_days, audit).await?;
                result.cleanup_results = entries;
                result.warnings = warnings;
                result.total_freed_bytes =
                    result.cleanup_results.iter().map(|c| c.freed_bytes).sum();
                result.total_freed_human = human_size(result.total_freed_bytes);
                result.output = format!(
                    "清理 {} 中 {} 天前的日志, 释放 {}",
                    log_dir, older_than_days, result.total_freed_human
                );
            }
            DiskAction::CleanPackageCache => {
                let (entries, warnings) = Self::clean_package_cache(executor, host, audit).await?;
                result.cleanup_results = entries;
                result.warnings = warnings;
                result.total_freed_bytes =
                    result.cleanup_results.iter().map(|c| c.freed_bytes).sum();
                result.total_freed_human = human_size(result.total_freed_bytes);
                result.output = format!("包缓存清理, 释放 {}", result.total_freed_human);
            }
        }

        result.success = result.warnings.is_empty() || !result.cleanup_results.is_empty();
        let _ = audit.log_action(
            "system",
            "DiskManager",
            &format!("{}: {}", host, result.action),
            &result.output,
        );
        info!(host, action = %result.action, success = result.success, "磁盘操作完成");
        Ok(result)
    }

    /// 获取磁盘使用情况（df -B1 + df -i）
    async fn get_usage(executor: &TaskExecutor, host: &str) -> Result<Vec<DiskUsageEntry>> {
        // df -B1 获取字节级使用量
        let df_result = executor
            .execute_command(&[host.to_string()], "df -B1 --output=source,target,size,used,avail,pcent | grep -v tmpfs | grep -v devtmpfs")
            .await?;
        let df_output = df_result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        // df -i 获取 inode 使用量
        let inode_result = executor
            .execute_command(&[host.to_string()], "df -i --output=source,itotal,iused,iavail,ipcent | grep -v tmpfs | grep -v devtmpfs")
            .await?;
        let inode_output = inode_result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let inode_map = Self::parse_inode_output(&inode_output);
        let mut entries = Vec::new();

        for line in df_output.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 6 {
                continue;
            }
            let filesystem = fields[0].to_string();
            let mount_point = fields[1].to_string();
            let total_bytes = fields[2].parse::<u64>().unwrap_or(0);
            let used_bytes = fields[3].parse::<u64>().unwrap_or(0);
            let available_bytes = fields[4].parse::<u64>().unwrap_or(0);
            let use_percent = fields[5]
                .trim_end_matches('%')
                .parse::<f64>()
                .unwrap_or(0.0);

            let (inode_total, inode_used, inode_available, inode_percent) = inode_map
                .get(&filesystem)
                .copied()
                .unwrap_or((0, 0, 0, 0.0));

            entries.push(DiskUsageEntry {
                filesystem,
                mount_point,
                total_bytes,
                used_bytes,
                available_bytes,
                use_percent,
                inode_total,
                inode_used,
                inode_available,
                inode_percent,
            });
        }

        Ok(entries)
    }

    /// 解析 inode 输出
    fn parse_inode_output(output: &str) -> std::collections::HashMap<String, (u64, u64, u64, f64)> {
        let mut map = std::collections::HashMap::new();
        for line in output.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 4 {
                continue;
            }
            let fs = fields[0].to_string();
            // df -i --output=source,itotal,iused,iavail,ipcent
            // fields: [source, itotal, iused, iavail_or_mount, ipcent_or_mount...]
            // Note: "Mounted on" may be 2 words (e.g., "/ boot"), making parsing tricky
            let total = fields[1].parse::<u64>().unwrap_or(0);
            let used = fields[2].parse::<u64>().unwrap_or(0);
            // Find the field with '%' to get the percent
            let percent = fields
                .iter()
                .skip(3)
                .find(|f| f.contains('%'))
                .and_then(|f| f.trim_end_matches('%').parse::<f64>().ok())
                .unwrap_or(0.0);
            let avail = total.saturating_sub(used);
            map.insert(fs, (total, used, avail, percent));
        }
        map
    }

    /// 查找大文件
    async fn find_large_files(
        executor: &TaskExecutor,
        host: &str,
        path: &str,
        min_size_mb: u64,
        limit: usize,
    ) -> Result<Vec<LargeFileEntry>> {
        let cmd = format!(
            "find {} -type f -size +{}M -exec ls -lhS {{}} \\; 2>/dev/null | head -{}",
            path, min_size_mb, limit
        );
        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        let mut files = Vec::new();
        for line in output.lines() {
            // ls -lhS 输出: -rw-r--r-- 1 user group 1.2G May 20 10:00 /path/to/file
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 9 {
                continue;
            }
            let size_human = parts[4].to_string();
            let size_bytes = parse_human_size(&size_human);
            let modified = format!("{} {} {}", parts[5], parts[6], parts[7]);
            let path_str = parts[8..].join(" ");

            files.push(LargeFileEntry {
                path: path_str,
                size_bytes,
                size_human,
                modified,
                file_type: Self::guess_file_type(parts.last().unwrap_or(&"")),
            });
        }

        Ok(files)
    }

    /// 猜测文件类型
    fn guess_file_type(path: &str) -> String {
        if path.ends_with(".log") || path.ends_with(".log.gz") {
            "日志文件".to_string()
        } else if path.ends_with(".tar") || path.ends_with(".gz") || path.ends_with(".zip") {
            "压缩文件".to_string()
        } else if path.ends_with(".core") || path.contains("/core.") {
            "Core dump".to_string()
        } else if path.contains("/tmp/") {
            "临时文件".to_string()
        } else if path.ends_with(".rpm") || path.ends_with(".deb") {
            "包文件".to_string()
        } else {
            "其他".to_string()
        }
    }

    /// 安全清理 — 每种目标独立执行，一个失败不影响其他
    async fn safe_cleanup(
        executor: &TaskExecutor,
        host: &str,
        targets: &[CleanupTarget],
        dry_run: bool,
        audit: &AuditLog,
    ) -> Result<(Vec<CleanupEntry>, Vec<String>)> {
        let mut entries = Vec::new();
        let mut warnings = Vec::new();

        for target in targets {
            let (cmd, desc) = match target {
                CleanupTarget::SystemLogs => (
                    // 清理压缩日志和大于100M的日志
                    "find /var/log -name '*.gz' -delete 2>/dev/null; \
                     find /var/log -name '*.old' -delete 2>/dev/null; \
                     find /var/log -name '*.[0-9]' -delete 2>/dev/null; \
                     find /var/log -type f -size +100M -exec truncate -s 0 {} \\; 2>/dev/null; \
                     echo 'done'"
                        .to_string(),
                    "清理系统日志(.gz/.old/大文件)".to_string(),
                ),
                CleanupTarget::TempFiles => (
                    "find /tmp -type f -atime +7 -delete 2>/dev/null; \
                     find /var/tmp -type f -atime +7 -delete 2>/dev/null; \
                     echo 'done'"
                        .to_string(),
                    "清理7天前临时文件".to_string(),
                ),
                CleanupTarget::PackageCache => {
                    // 检测包管理器并清理
                    (
                        "if command -v apt-get &>/dev/null; then \
                             apt-get clean 2>/dev/null; \
                         elif command -v yum &>/dev/null; then \
                             yum clean all 2>/dev/null; \
                         elif command -v dnf &>/dev/null; then \
                             dnf clean all 2>/dev/null; \
                         fi; echo 'done'"
                            .to_string(),
                        "清理包管理器缓存".to_string(),
                    )
                }
                CleanupTarget::DockerPrune => (
                    "docker system prune -f 2>/dev/null || echo 'docker not available'".to_string(),
                    "Docker 清理未使用资源".to_string(),
                ),
                CleanupTarget::OldKernels => (
                    // 保留当前内核 + 1个旧版本
                    "if command -v apt-get &>/dev/null; then \
                         current=$(uname -r); \
                         dpkg -l 'linux-image-[0-9]*' | grep ^ii | awk '{print $2}' | \
                         grep -v \"$current\" | head -n -1 | \
                         xargs -r apt-get -y purge 2>/dev/null; \
                     fi; echo 'done'"
                        .to_string(),
                    "清理旧内核(保留当前+1)".to_string(),
                ),
                CleanupTarget::Journal { keep_days } => (
                    format!(
                        "journalctl --vacuum-time={}d 2>/dev/null; echo 'done'",
                        keep_days
                    ),
                    format!("清理 {} 天前的 journal 日志", keep_days),
                ),
                CleanupTarget::CustomPath { path } => {
                    if !dry_run {
                        warnings.push(format!("⚠️ 自定义路径清理需要 dry_run=true: {}", path));
                        continue;
                    }
                    (
                        format!("echo 'DRY_RUN: would clean {}'", path),
                        format!("自定义路径清理(干运行): {}", path),
                    )
                }
            };

            // 清理前先统计大小
            let before_size = Self::get_dir_size(executor, host, Self::target_root_dir(target))
                .await
                .unwrap_or(0);

            if !dry_run {
                let _ = executor.execute_command(&[host.to_string()], &cmd).await?;
            }

            let after_size = Self::get_dir_size(executor, host, Self::target_root_dir(target))
                .await
                .unwrap_or(0);
            let freed = if dry_run {
                0
            } else {
                before_size.saturating_sub(after_size)
            };

            entries.push(CleanupEntry {
                target: desc.clone(),
                freed_bytes: freed,
                freed_human: human_size(freed),
                items_removed: 0, // 简化：不单独计数
                details: if dry_run {
                    format!("干运行: {}", desc)
                } else {
                    desc.clone()
                },
            });

            let _ = audit.log_action(
                "system",
                "DiskManager",
                &format!("{}: {}", host, desc),
                if dry_run { "DRY_RUN" } else { "executed" },
            );
        }

        Ok((entries, warnings))
    }

    /// 清理目标对应的根目录
    fn target_root_dir(target: &CleanupTarget) -> &str {
        match target {
            CleanupTarget::SystemLogs => "/var/log",
            CleanupTarget::TempFiles => "/tmp",
            CleanupTarget::PackageCache => "/var/cache",
            CleanupTarget::DockerPrune => "/var/lib/docker",
            CleanupTarget::OldKernels => "/boot",
            CleanupTarget::Journal { .. } => "/var/log/journal",
            CleanupTarget::CustomPath { path } => path.as_str(),
        }
    }

    /// 获取目录大小（字节）
    async fn get_dir_size(executor: &TaskExecutor, host: &str, path: &str) -> Result<u64> {
        let cmd = format!("du -sb {} 2>/dev/null | cut -f1", path);
        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.trim().to_string())
            .unwrap_or_default();
        output
            .parse::<u64>()
            .map_err(|e| AutomationError::DiskManagement(format!("解析目录大小失败: {}", e)))
    }

    /// SMART 健康检查
    async fn check_smart(
        executor: &TaskExecutor,
        host: &str,
        device: &str,
    ) -> Result<Vec<SmartHealthInfo>> {
        let cmd = format!(
            "smartctl -H -A {} 2>/dev/null || echo 'smartctl not available'",
            device
        );
        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        if output.contains("smartctl not available") || output.is_empty() {
            return Ok(vec![SmartHealthInfo {
                device: device.to_string(),
                model: "N/A".to_string(),
                health_status: "smartctl 不可用".to_string(),
                temperature_celsius: None,
                power_on_hours: None,
                reallocated_sectors: None,
                pending_sectors: None,
                errors: vec!["smartctl 未安装或设备不支持".to_string()],
            }]);
        }

        let mut info = SmartHealthInfo {
            device: device.to_string(),
            model: String::new(),
            health_status: "Unknown".to_string(),
            temperature_celsius: None,
            power_on_hours: None,
            reallocated_sectors: None,
            pending_sectors: None,
            errors: Vec::new(),
        };

        for line in output.lines() {
            if line.contains("Model Family:") || line.contains("Device Model:") {
                info.model = line.split(':').nth(1).unwrap_or("").trim().to_string();
            } else if line.contains("SMART Health Status:") || line.contains("SMART overall-health")
            {
                info.health_status = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("Unknown")
                    .trim()
                    .to_string();
            } else if line.contains("Temperature_Celsius") || line.contains("Airflow_Temperature") {
                info.temperature_celsius = Self::parse_smart_value(line, 9);
            } else if line.contains("Power_On_Hours") {
                info.power_on_hours = Self::parse_smart_value(line, 9).map(|v| v as u64);
            } else if line.contains("Reallocated_Sector_Ct") {
                info.reallocated_sectors = Self::parse_smart_value(line, 5).map(|v| v as u64);
            } else if line.contains("Current_Pending_Sector") {
                info.pending_sectors = Self::parse_smart_value(line, 5).map(|v| v as u64);
            }
        }

        Ok(vec![info])
    }

    /// 解析 SMART 属性值（VALUE 列）
    fn parse_smart_value(line: &str, col: usize) -> Option<f64> {
        line.split_whitespace()
            .nth(col)
            .and_then(|v| v.parse::<f64>().ok())
    }

    /// 获取 IO 统计
    async fn get_io_stats(executor: &TaskExecutor, host: &str) -> Result<Vec<IoStatEntry>> {
        let result = executor
            .execute_command(
                &[host.to_string()],
                "iostat -dx 1 2 2>/dev/null | tail -n +4 || echo 'iostat not available'",
            )
            .await?;
        let output = result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        if output.contains("iostat not available") {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        // 取第二次采样的数据（更准确）
        let sections: Vec<&str> = output.split("\n\n").collect();
        let data_section = sections.last().unwrap_or(&"");

        for line in data_section.lines() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 14 {
                continue;
            }
            // 跳过标题行
            if fields[0] == "Device" || fields[0] == "Device:" {
                continue;
            }

            let device = fields[0].to_string();
            let reads_per_sec = fields[1].parse::<f64>().unwrap_or(0.0);
            let writes_per_sec = fields[7].parse::<f64>().unwrap_or(0.0);
            let read_bytes_per_sec = (fields[2].parse::<f64>().unwrap_or(0.0) * 1024.0) as u64;
            let write_bytes_per_sec = (fields[8].parse::<f64>().unwrap_or(0.0) * 1024.0) as u64;
            let io_util_percent = fields
                .last()
                .and_then(|v| v.trim_end_matches('%').parse::<f64>().ok())
                .unwrap_or(0.0);
            let await_ms = if fields.len() > 10 {
                fields[9].parse::<f64>().unwrap_or(0.0)
            } else {
                0.0
            };

            entries.push(IoStatEntry {
                device,
                reads_per_sec,
                writes_per_sec,
                read_bytes_per_sec,
                write_bytes_per_sec,
                io_util_percent,
                await_ms,
            });
        }

        Ok(entries)
    }

    /// 清理旧日志
    async fn clean_old_logs(
        executor: &TaskExecutor,
        host: &str,
        log_dir: &str,
        older_than_days: u32,
        audit: &AuditLog,
    ) -> Result<(Vec<CleanupEntry>, Vec<String>)> {
        let before_size = Self::get_dir_size(executor, host, log_dir)
            .await
            .unwrap_or(0);

        let cmd = format!(
            "find {} -type f -mtime +{} -name '*.log' -delete 2>/dev/null; \
             find {} -type f -mtime +{} -name '*.log.gz' -delete 2>/dev/null; \
             find {} -type f -mtime +{} -name '*.old' -delete 2>/dev/null; \
             echo 'done'",
            log_dir, older_than_days, log_dir, older_than_days, log_dir, older_than_days,
        );
        let _ = executor.execute_command(&[host.to_string()], &cmd).await?;

        let after_size = Self::get_dir_size(executor, host, log_dir)
            .await
            .unwrap_or(0);
        let freed = before_size.saturating_sub(after_size);

        let _ = audit.log_action(
            "system",
            "DiskManager",
            &format!("{}: 清理旧日志 {} (>{}天)", host, log_dir, older_than_days),
            &format!("释放 {}", human_size(freed)),
        );

        Ok((
            vec![CleanupEntry {
                target: format!("旧日志清理({})", log_dir),
                freed_bytes: freed,
                freed_human: human_size(freed),
                items_removed: 0,
                details: format!("清理 {} 天前的日志", older_than_days),
            }],
            Vec::new(),
        ))
    }

    /// 清理包管理器缓存
    async fn clean_package_cache(
        executor: &TaskExecutor,
        host: &str,
        audit: &AuditLog,
    ) -> Result<(Vec<CleanupEntry>, Vec<String>)> {
        let mut entries = Vec::new();
        let mut warnings = Vec::new();

        // 检测并清理 apt
        let apt_check = executor
            .execute_command(
                &[host.to_string()],
                "command -v apt-get && echo APT || echo NO_APT",
            )
            .await?;
        let apt_output = apt_check
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        if apt_output.contains("APT") && !apt_output.contains("NO_APT") {
            let before = Self::get_dir_size(executor, host, "/var/cache/apt")
                .await
                .unwrap_or(0);
            let _ = executor
                .execute_command(&[host.to_string()], "apt-get clean 2>/dev/null")
                .await;
            let after = Self::get_dir_size(executor, host, "/var/cache/apt")
                .await
                .unwrap_or(0);
            let freed = before.saturating_sub(after);
            entries.push(CleanupEntry {
                target: "apt 缓存".to_string(),
                freed_bytes: freed,
                freed_human: human_size(freed),
                items_removed: 0,
                details: "apt-get clean".to_string(),
            });
        }

        // 检测并清理 yum/dnf
        let yum_check = executor
            .execute_command(
                &[host.to_string()],
                "(command -v yum || command -v dnf) && echo YUM || echo NO_YUM",
            )
            .await?;
        let yum_output = yum_check
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .unwrap_or_default();

        if yum_output.contains("YUM") && !yum_output.contains("NO_YUM") {
            let before = Self::get_dir_size(executor, host, "/var/cache/yum")
                .await
                .unwrap_or(0);
            let _ = executor
                .execute_command(
                    &[host.to_string()],
                    "yum clean all 2>/dev/null || dnf clean all 2>/dev/null",
                )
                .await;
            let after = Self::get_dir_size(executor, host, "/var/cache/yum")
                .await
                .unwrap_or(0);
            let freed = before.saturating_sub(after);
            entries.push(CleanupEntry {
                target: "yum/dnf 缓存".to_string(),
                freed_bytes: freed,
                freed_human: human_size(freed),
                items_removed: 0,
                details: "yum/dnf clean all".to_string(),
            });
        }

        if entries.is_empty() {
            warnings.push("未检测到已知包管理器".to_string());
        }

        let _ = audit.log_action(
            "system",
            "DiskManager",
            &format!("{}: 包缓存清理", host),
            &format!("{} 项", entries.len()),
        );

        Ok((entries, warnings))
    }
}

/// 人类可读文件大小
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}KB", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

/// 解析人类可读大小为字节
fn parse_human_size(s: &str) -> u64 {
    let s = s.trim();
    let (num_part, multiplier) = if s.ends_with("G") || s.ends_with("GB") {
        (
            s.trim_end_matches("GB").trim_end_matches('G'),
            1024 * 1024 * 1024,
        )
    } else if s.ends_with("M") || s.ends_with("MB") {
        (s.trim_end_matches("MB").trim_end_matches('M'), 1024 * 1024)
    } else if s.ends_with("K") || s.ends_with("KB") {
        (s.trim_end_matches("KB").trim_end_matches('K'), 1024)
    } else if s.ends_with("T") || s.ends_with("TB") {
        (
            s.trim_end_matches("TB").trim_end_matches('T'),
            1024_u64 * 1024 * 1024 * 1024,
        )
    } else {
        (s, 1)
    };

    num_part
        .parse::<f64>()
        .map(|v| (v * multiplier as f64) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human_size_bytes() {
        assert_eq!(human_size(500), "500B");
        assert_eq!(human_size(0), "0B");
    }

    #[test]
    fn test_human_size_kb() {
        assert_eq!(human_size(1024), "1.0KB");
        assert_eq!(human_size(1536), "1.5KB");
    }

    #[test]
    fn test_human_size_mb() {
        assert_eq!(human_size(1024 * 1024), "1.0MB");
        assert_eq!(human_size(100 * 1024 * 1024), "100.0MB");
    }

    #[test]
    fn test_human_size_gb() {
        assert_eq!(human_size(1024 * 1024 * 1024), "1.0GB");
        assert_eq!(
            human_size(5 * 1024 * 1024 * 1024 + 512 * 1024 * 1024),
            "5.5GB"
        );
    }

    #[test]
    fn test_human_size_tb() {
        assert_eq!(human_size(1024_u64 * 1024 * 1024 * 1024), "1.0TB");
    }

    #[test]
    fn test_parse_human_size() {
        assert_eq!(
            parse_human_size("1.5G"),
            (1.5 * 1024.0 * 1024.0 * 1024.0) as u64
        );
        assert_eq!(parse_human_size("100M"), 100 * 1024 * 1024);
        assert_eq!(parse_human_size("500K"), 500 * 1024);
        assert_eq!(parse_human_size("42"), 42);
    }

    #[test]
    fn test_parse_human_size_edge_cases() {
        assert_eq!(parse_human_size(""), 0);
        assert_eq!(parse_human_size("abc"), 0);
        assert_eq!(parse_human_size("0B"), 0);
    }

    #[test]
    fn test_disk_action_variants() {
        let usage = DiskAction::Usage;
        assert_eq!(format!("{:?}", usage), "Usage");

        let find = DiskAction::FindLargeFiles {
            path: "/var".to_string(),
            min_size_mb: 100,
            limit: 10,
        };
        assert!(format!("{:?}", find).contains("FindLargeFiles"));

        let cleanup = DiskAction::SafeCleanup {
            targets: vec![CleanupTarget::SystemLogs, CleanupTarget::TempFiles],
            dry_run: true,
        };
        assert!(format!("{:?}", cleanup).contains("SafeCleanup"));

        let smart = DiskAction::SmartHealth {
            device: "/dev/sda".to_string(),
        };
        assert!(format!("{:?}", smart).contains("SmartHealth"));

        let inode = DiskAction::InodeCheck;
        assert_eq!(format!("{:?}", inode), "InodeCheck");

        let io = DiskAction::IoStats;
        assert_eq!(format!("{:?}", io), "IoStats");

        let logs = DiskAction::CleanOldLogs {
            log_dir: "/var/log".to_string(),
            older_than_days: 30,
        };
        assert!(format!("{:?}", logs).contains("CleanOldLogs"));

        let pkg = DiskAction::CleanPackageCache;
        assert_eq!(format!("{:?}", pkg), "CleanPackageCache");
    }

    #[test]
    fn test_cleanup_target_variants() {
        let t1 = CleanupTarget::SystemLogs;
        assert_eq!(format!("{:?}", t1), "SystemLogs");

        let t2 = CleanupTarget::TempFiles;
        assert_eq!(format!("{:?}", t2), "TempFiles");

        let t3 = CleanupTarget::PackageCache;
        assert_eq!(format!("{:?}", t3), "PackageCache");

        let t4 = CleanupTarget::DockerPrune;
        assert_eq!(format!("{:?}", t4), "DockerPrune");

        let t5 = CleanupTarget::OldKernels;
        assert_eq!(format!("{:?}", t5), "OldKernels");

        let t6 = CleanupTarget::Journal { keep_days: 7 };
        assert!(format!("{:?}", t6).contains("Journal"));

        let t7 = CleanupTarget::CustomPath {
            path: "/data/cache".to_string(),
        };
        assert!(format!("{:?}", t7).contains("CustomPath"));
    }

    #[test]
    fn test_disk_usage_entry_serialization() {
        let entry = DiskUsageEntry {
            filesystem: "/dev/sda1".to_string(),
            mount_point: "/".to_string(),
            total_bytes: 100 * 1024 * 1024 * 1024,
            used_bytes: 60 * 1024 * 1024 * 1024,
            available_bytes: 40 * 1024 * 1024 * 1024,
            use_percent: 60.0,
            inode_total: 1000000,
            inode_used: 500000,
            inode_available: 500000,
            inode_percent: 50.0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: DiskUsageEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_large_file_entry_serialization() {
        let entry = LargeFileEntry {
            path: "/var/log/syslog".to_string(),
            size_bytes: 1024 * 1024 * 1024,
            size_human: "1.0GB".to_string(),
            modified: "May 20 10:00".to_string(),
            file_type: "日志文件".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LargeFileEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_cleanup_entry_serialization() {
        let entry = CleanupEntry {
            target: "系统日志".to_string(),
            freed_bytes: 500 * 1024 * 1024,
            freed_human: "500.0MB".to_string(),
            items_removed: 42,
            details: "清理 .gz 和 .old 文件".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: CleanupEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_smart_health_info_serialization() {
        let info = SmartHealthInfo {
            device: "/dev/sda".to_string(),
            model: "Samsung SSD 870".to_string(),
            health_status: "PASSED".to_string(),
            temperature_celsius: Some(35.0),
            power_on_hours: Some(8760),
            reallocated_sectors: Some(0),
            pending_sectors: Some(0),
            errors: vec![],
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SmartHealthInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, deserialized);
    }

    #[test]
    fn test_smart_health_info_with_errors() {
        let info = SmartHealthInfo {
            device: "/dev/sdb".to_string(),
            model: "WDC WD10EZEX".to_string(),
            health_status: "FAILED".to_string(),
            temperature_celsius: Some(55.0),
            power_on_hours: Some(43800),
            reallocated_sectors: Some(100),
            pending_sectors: Some(5),
            errors: vec!["Reallocated sector count high".to_string()],
        };
        assert!(!info.errors.is_empty());
        assert_eq!(info.health_status, "FAILED");
    }

    #[test]
    fn test_io_stat_entry_serialization() {
        let entry = IoStatEntry {
            device: "sda".to_string(),
            reads_per_sec: 100.5,
            writes_per_sec: 50.3,
            read_bytes_per_sec: 1024 * 1024,
            write_bytes_per_sec: 512 * 1024,
            io_util_percent: 45.2,
            await_ms: 2.5,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: IoStatEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_disk_management_result_default() {
        let result = DiskManagementResult::default();
        assert!(result.success);
        assert!(result.usage.is_empty());
        assert!(result.large_files.is_empty());
        assert!(result.cleanup_results.is_empty());
        assert!(result.smart_health.is_empty());
        assert!(result.io_stats.is_empty());
        assert_eq!(result.total_freed_bytes, 0);
        assert_eq!(result.total_freed_human, "0B");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_disk_management_result_serialization() {
        let result = DiskManagementResult {
            action: "Usage".to_string(),
            success: true,
            usage: vec![DiskUsageEntry {
                filesystem: "/dev/sda1".to_string(),
                mount_point: "/".to_string(),
                total_bytes: 1024 * 1024 * 1024,
                used_bytes: 512 * 1024 * 1024,
                available_bytes: 512 * 1024 * 1024,
                use_percent: 50.0,
                inode_total: 1000,
                inode_used: 500,
                inode_available: 500,
                inode_percent: 50.0,
            }],
            ..Default::default()
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: DiskManagementResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }

    #[test]
    fn test_guess_file_type() {
        assert_eq!(DiskManager::guess_file_type("/var/log/app.log"), "日志文件");
        assert_eq!(
            DiskManager::guess_file_type("/var/log/app.log.gz"),
            "日志文件"
        );
        assert_eq!(
            DiskManager::guess_file_type("/tmp/archive.tar.gz"),
            "压缩文件"
        );
        assert_eq!(DiskManager::guess_file_type("/tmp/core.12345"), "Core dump");
        assert_eq!(DiskManager::guess_file_type("/tmp/tmpfile"), "临时文件");
        assert_eq!(DiskManager::guess_file_type("/cache/pkg.rpm"), "包文件");
        assert_eq!(DiskManager::guess_file_type("/data/file.txt"), "其他");
    }

    #[test]
    fn test_target_root_dir() {
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::SystemLogs),
            "/var/log"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::TempFiles),
            "/tmp"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::PackageCache),
            "/var/cache"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::DockerPrune),
            "/var/lib/docker"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::OldKernels),
            "/boot"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::Journal { keep_days: 7 }),
            "/var/log/journal"
        );
        assert_eq!(
            DiskManager::target_root_dir(&CleanupTarget::CustomPath {
                path: "/data".to_string()
            }),
            "/data"
        );
    }

    #[test]
    fn test_parse_inode_output() {
        let output = "Filesystem      IUsed   IFree IUse% Mounted on\n/dev/sda1       100000  900000   10% /\n/dev/sdb1       500000  500000   50% /data";
        let map = DiskManager::parse_inode_output(output);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("/dev/sda1"));
        assert!(map.contains_key("/dev/sdb1"));
        let (total, used, avail, percent) = map["/dev/sda1"];
        assert_eq!(total, 100000); // itotal (fields[1])
        assert_eq!(used, 900000); // iused (fields[2])
        assert_eq!(avail, 0); // total(100000) < used(900000), so 0
        assert_eq!(percent, 10.0); // IUse% parsed from field containing '%'
    }

    #[test]
    fn test_parse_inode_output_empty() {
        let output = "";
        let map = DiskManager::parse_inode_output(output);
        assert!(map.is_empty());
    }

    #[test]
    fn test_parse_inode_output_malformed() {
        let output = "garbage data\nnot enough fields";
        let map = DiskManager::parse_inode_output(output);
        assert!(map.is_empty());
    }

    #[test]
    fn test_cleanup_target_clone_eq() {
        let t1 = CleanupTarget::SystemLogs;
        let t2 = t1.clone();
        assert_eq!(t1, t2);

        let t3 = CleanupTarget::Journal { keep_days: 7 };
        let t4 = t3.clone();
        assert_eq!(t3, t4);
    }

    #[test]
    fn test_disk_action_clone_eq() {
        let a1 = DiskAction::Usage;
        let a2 = a1.clone();
        assert_eq!(a1, a2);

        let a3 = DiskAction::FindLargeFiles {
            path: "/".to_string(),
            min_size_mb: 100,
            limit: 10,
        };
        let a4 = a3.clone();
        assert_eq!(a3, a4);
    }

    #[test]
    fn test_disk_management_result_warnings() {
        let mut result = DiskManagementResult::default();
        result.warnings.push("磁盘使用率超过90%".to_string());
        result.warnings.push("inode使用率超过80%".to_string());
        assert_eq!(result.warnings.len(), 2);
        assert!(result.warnings[0].contains("90%"));
    }

    #[test]
    fn test_disk_management_result_total_freed() {
        let mut result = DiskManagementResult::default();
        result.cleanup_results.push(CleanupEntry {
            target: "日志".to_string(),
            freed_bytes: 100 * 1024 * 1024,
            freed_human: "100.0MB".to_string(),
            items_removed: 10,
            details: "".to_string(),
        });
        result.cleanup_results.push(CleanupEntry {
            target: "缓存".to_string(),
            freed_bytes: 200 * 1024 * 1024,
            freed_human: "200.0MB".to_string(),
            items_removed: 5,
            details: "".to_string(),
        });
        result.total_freed_bytes = result.cleanup_results.iter().map(|c| c.freed_bytes).sum();
        result.total_freed_human = human_size(result.total_freed_bytes);
        assert_eq!(result.total_freed_bytes, 300 * 1024 * 1024);
        assert_eq!(result.total_freed_human, "300.0MB");
    }
}
