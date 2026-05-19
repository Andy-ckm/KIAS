//! R023: Linux 日常巡检模块
//!
//! 巡检 CPU/内存/磁盘/进程/日志/网络/安全
//! 灵魂: 可追溯(审计日志) / 透明(实时报告) / 可控(阈值可配)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::audit::AuditLog;
use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::models::*;

/// 巡检阈值配置 (可控: 用户可自定义)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckThresholds {
    pub cpu_warn_percent: f64,
    pub cpu_crit_percent: f64,
    pub mem_warn_percent: f64,
    pub mem_crit_percent: f64,
    pub disk_warn_percent: f64,
    pub disk_crit_percent: f64,
    pub load_warn_multiplier: f64,
    pub load_crit_multiplier: f64,
    pub swap_warn_percent: f64,
    pub inode_warn_percent: f64,
    pub zombie_warn_count: u32,
}

impl Default for HealthCheckThresholds {
    fn default() -> Self {
        Self {
            cpu_warn_percent: 80.0,
            cpu_crit_percent: 95.0,
            mem_warn_percent: 80.0,
            mem_crit_percent: 95.0,
            disk_warn_percent: 80.0,
            disk_crit_percent: 95.0,
            load_warn_multiplier: 2.0,
            load_crit_multiplier: 4.0,
            swap_warn_percent: 50.0,
            inode_warn_percent: 80.0,
            zombie_warn_count: 1,
        }
    }
}

/// 巡检引擎
pub struct HealthChecker {
    thresholds: HealthCheckThresholds,
}

impl HealthChecker {
    pub fn new(thresholds: HealthCheckThresholds) -> Self {
        Self { thresholds }
    }

    /// 执行完整巡检 (通过 SSH 在远程主机执行)
    pub async fn check_all(
        &self,
        executor: &TaskExecutor,
        host: &str,
        checks: &[HealthCheckType],
        audit: &AuditLog,
    ) -> Result<HealthCheckReport> {
        let run_all = checks.contains(&HealthCheckType::All);
        let mut items = Vec::new();
        let mut recommendations = Vec::new();

        // CPU 巡检
        if run_all || checks.contains(&HealthCheckType::Cpu) {
            let cpu_items = self.check_cpu(executor, host).await?;
            items.extend(cpu_items);
        }

        // 内存巡检
        if run_all || checks.contains(&HealthCheckType::Memory) {
            let mem_items = self.check_memory(executor, host).await?;
            items.extend(mem_items);
        }

        // 磁盘巡检
        if run_all || checks.contains(&HealthCheckType::Disk) {
            let disk_items = self.check_disk(executor, host).await?;
            items.extend(disk_items);
        }

        // 进程巡检
        if run_all || checks.contains(&HealthCheckType::Process) {
            let proc_items = self.check_process(executor, host).await?;
            items.extend(proc_items);
        }

        // 日志巡检
        if run_all || checks.contains(&HealthCheckType::Log) {
            let log_items = self.check_logs(executor, host).await?;
            items.extend(log_items);
        }

        // 网络巡检
        if run_all || checks.contains(&HealthCheckType::Network) {
            let net_items = self.check_network(executor, host).await?;
            items.extend(net_items);
        }

        // 安全巡检
        if run_all || checks.contains(&HealthCheckType::Security) {
            let sec_items = self.check_security(executor, host).await?;
            items.extend(sec_items);
        }

        // 生成建议
        for item in &items {
            if item.status == HealthStatus::Critical || item.status == HealthStatus::Warning {
                recommendations.push(format!("[{}] {}", item.metric_name, item.message));
            }
        }

        // 确定整体状态
        let overall_status = if items.iter().any(|i| i.status == HealthStatus::Critical) {
            HealthStatus::Critical
        } else if items.iter().any(|i| i.status == HealthStatus::Warning) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        };

        let report = HealthCheckReport {
            host: host.to_string(),
            check_time: Utc::now(),
            overall_status,
            checks: items,
            recommendations,
        };

        // 可追溯: 记录审计日志
        audit.log_action(
            "system",
            "HealthCheck",
            host,
            &format!("巡检完成, 状态: {:?}", report.overall_status),
        )?;

        match report.overall_status {
            HealthStatus::Healthy => info!(host = %host, "巡检正常"),
            HealthStatus::Warning => warn!(host = %host, "巡检发现警告"),
            HealthStatus::Critical => warn!(host = %host, "巡检发现严重问题"),
            HealthStatus::Unknown => warn!(host = %host, "巡检状态未知"),
        }

        Ok(report)
    }

    /// CPU 巡检
    async fn check_cpu(&self, executor: &TaskExecutor, host: &str) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 获取 CPU 使用率
        let cpu_cmd = "top -bn1 | grep 'Cpu(s)' | awk '{print $2}'";
        let result = executor
            .execute_command(&[host.to_string()], cpu_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let usage: f64 = hr.stdout.trim().parse().unwrap_or(0.0);
                let status = if usage >= self.thresholds.cpu_crit_percent {
                    HealthStatus::Critical
                } else if usage >= self.thresholds.cpu_warn_percent {
                    HealthStatus::Warning
                } else {
                    HealthStatus::Healthy
                };
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Cpu,
                    status,
                    metric_name: "CPU使用率".to_string(),
                    metric_value: format!("{:.1}%", usage),
                    threshold: Some(format!(
                        "警告: {}%, 严重: {}%",
                        self.thresholds.cpu_warn_percent, self.thresholds.cpu_crit_percent
                    )),
                    message: format!("CPU使用率 {:.1}%", usage),
                });
            }
        }

        // 获取负载
        let load_cmd = "cat /proc/loadavg";
        let result = executor
            .execute_command(&[host.to_string()], load_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let parts: Vec<&str> = hr.stdout.split_whitespace().collect();
                if let Some(load1_str) = parts.first() {
                    if let Ok(load1) = load1_str.parse::<f64>() {
                        let nproc_cmd = "nproc";
                        let nproc_result = executor
                            .execute_command(&[host.to_string()], nproc_cmd)
                            .await?;
                        let nproc: f64 = nproc_result
                            .host_results
                            .first()
                            .and_then(|h| h.stdout.trim().parse().ok())
                            .unwrap_or(1.0);
                        let load_ratio = load1 / nproc;
                        let status = if load_ratio >= self.thresholds.load_crit_multiplier {
                            HealthStatus::Critical
                        } else if load_ratio >= self.thresholds.load_warn_multiplier {
                            HealthStatus::Warning
                        } else {
                            HealthStatus::Healthy
                        };
                        items.push(HealthCheckItem {
                            check_type: HealthCheckType::Cpu,
                            status,
                            metric_name: "系统负载".to_string(),
                            metric_value: format!("{} ({}核)", load1_str, nproc as u32),
                            threshold: Some(format!(
                                "警告: {}x核数, 严重: {}x核数",
                                self.thresholds.load_warn_multiplier,
                                self.thresholds.load_crit_multiplier
                            )),
                            message: format!("负载 {} / {}核 = {:.1}x", load1, nproc, load_ratio),
                        });
                    }
                }
            }
        }

        Ok(items)
    }

    /// 内存巡检
    async fn check_memory(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        let cmd = "free -m | awk '/Mem:/ {printf \"%s %s %.1f\", $2, $3, $3/$2*100}'";
        let result = executor.execute_command(&[host.to_string()], cmd).await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let parts: Vec<&str> = hr.stdout.split_whitespace().collect();
                if parts.len() >= 3 {
                    let total_mb: u64 = parts[0].parse().unwrap_or(0);
                    let used_mb: u64 = parts[1].parse().unwrap_or(0);
                    let percent: f64 = parts[2].parse().unwrap_or(0.0);
                    let status = if percent >= self.thresholds.mem_crit_percent {
                        HealthStatus::Critical
                    } else if percent >= self.thresholds.mem_warn_percent {
                        HealthStatus::Warning
                    } else {
                        HealthStatus::Healthy
                    };
                    items.push(HealthCheckItem {
                        check_type: HealthCheckType::Memory,
                        status,
                        metric_name: "内存使用率".to_string(),
                        metric_value: format!("{}MB/{}MB ({:.1}%)", used_mb, total_mb, percent),
                        threshold: Some(format!(
                            "警告: {}%, 严重: {}%",
                            self.thresholds.mem_warn_percent, self.thresholds.mem_crit_percent
                        )),
                        message: format!("内存 {}/{} ({:.1}%)", used_mb, total_mb, percent),
                    });
                }
            }
        }

        // Swap 检查
        let swap_cmd = "free -m | awk '/Swap:/ {if($2>0) printf \"%s %s %.1f\", $2, $3, $3/$2*100; else print \"0 0 0\"}'";
        let result = executor
            .execute_command(&[host.to_string()], swap_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let parts: Vec<&str> = hr.stdout.split_whitespace().collect();
                if parts.len() >= 3 {
                    let total: u64 = parts[0].parse().unwrap_or(0);
                    let percent: f64 = parts[2].parse().unwrap_or(0.0);
                    if total > 0 && percent >= self.thresholds.swap_warn_percent {
                        items.push(HealthCheckItem {
                            check_type: HealthCheckType::Memory,
                            status: HealthStatus::Warning,
                            metric_name: "Swap使用率".to_string(),
                            metric_value: format!("{:.1}%", percent),
                            threshold: Some(format!(
                                "警告: {}%",
                                self.thresholds.swap_warn_percent
                            )),
                            message: format!("Swap使用率 {:.1}%, 可能内存不足", percent),
                        });
                    }
                }
            }
        }

        Ok(items)
    }

    /// 磁盘巡检
    async fn check_disk(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 磁盘使用率
        let cmd = "df -h --output=target,pcent,size,used | grep -v tmpfs | tail -n +2";
        let result = executor.execute_command(&[host.to_string()], cmd).await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                for line in hr.stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let mount = parts[0];
                        let pcent_str = parts[1].trim_end_matches('%');
                        if let Ok(percent) = pcent_str.parse::<f64>() {
                            let status = if percent >= self.thresholds.disk_crit_percent {
                                HealthStatus::Critical
                            } else if percent >= self.thresholds.disk_warn_percent {
                                HealthStatus::Warning
                            } else {
                                HealthStatus::Healthy
                            };
                            items.push(HealthCheckItem {
                                check_type: HealthCheckType::Disk,
                                status,
                                metric_name: format!("磁盘 {}", mount),
                                metric_value: format!("{} ({} / {})", parts[1], parts[3], parts[2]),
                                threshold: Some(format!(
                                    "警告: {}%, 严重: {}%",
                                    self.thresholds.disk_warn_percent,
                                    self.thresholds.disk_crit_percent
                                )),
                                message: format!("{} 使用率 {}", mount, parts[1]),
                            });
                        }
                    }
                }
            }
        }

        // inode 使用率
        let inode_cmd = "df -i | awk 'NR>1 && $5+0 > 0 {print $6, $5}'";
        let result = executor
            .execute_command(&[host.to_string()], inode_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                for line in hr.stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let mount = parts[0];
                        let pcent_str = parts[1].trim_end_matches('%');
                        if let Ok(percent) = pcent_str.parse::<f64>() {
                            if percent >= self.thresholds.inode_warn_percent {
                                items.push(HealthCheckItem {
                                    check_type: HealthCheckType::Disk,
                                    status: HealthStatus::Warning,
                                    metric_name: format!("inode {}", mount),
                                    metric_value: format!("{}%", percent),
                                    threshold: Some(format!(
                                        "警告: {}%",
                                        self.thresholds.inode_warn_percent
                                    )),
                                    message: format!(
                                        "{} inode 使用率 {}%, 可能无法创建新文件",
                                        mount, percent
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(items)
    }

    /// 进程巡检
    async fn check_process(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 僵尸进程
        let zombie_cmd = "ps aux | awk '$8==\"Z\" {count++} END {print count+0}'";
        let result = executor
            .execute_command(&[host.to_string()], zombie_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
                if count >= self.thresholds.zombie_warn_count {
                    items.push(HealthCheckItem {
                        check_type: HealthCheckType::Process,
                        status: HealthStatus::Warning,
                        metric_name: "僵尸进程".to_string(),
                        metric_value: count.to_string(),
                        threshold: Some(format!("警告: >= {}", self.thresholds.zombie_warn_count)),
                        message: format!("发现 {} 个僵尸进程", count),
                    });
                } else {
                    items.push(HealthCheckItem {
                        check_type: HealthCheckType::Process,
                        status: HealthStatus::Healthy,
                        metric_name: "僵尸进程".to_string(),
                        metric_value: count.to_string(),
                        threshold: None,
                        message: "无僵尸进程".to_string(),
                    });
                }
            }
        }

        // 进程总数
        let total_cmd = "ps aux | wc -l";
        let result = executor
            .execute_command(&[host.to_string()], total_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Process,
                    status: HealthStatus::Healthy,
                    metric_name: "进程总数".to_string(),
                    metric_value: (count - 1).to_string(), // 减去 header 行
                    threshold: None,
                    message: format!("共 {} 个进程", count - 1),
                });
            }
        }

        Ok(items)
    }

    /// 日志巡检
    async fn check_logs(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 检查 systemd 失败单元
        let cmd = "systemctl --failed --no-legend 2>/dev/null | wc -l";
        let result = executor.execute_command(&[host.to_string()], cmd).await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
                let status = if count > 0 {
                    HealthStatus::Warning
                } else {
                    HealthStatus::Healthy
                };
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Log,
                    status,
                    metric_name: "systemd失败单元".to_string(),
                    metric_value: count.to_string(),
                    threshold: None,
                    message: if count > 0 {
                        format!("{} 个 systemd 单元处于失败状态", count)
                    } else {
                        "所有 systemd 单元正常".to_string()
                    },
                });
            }
        }

        // 检查 OOM killer
        let oom_cmd = "dmesg 2>/dev/null | grep -c 'Out of memory' || echo 0";
        let result = executor
            .execute_command(&[host.to_string()], oom_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
            if count > 0 {
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Log,
                    status: HealthStatus::Critical,
                    metric_name: "OOM Killer".to_string(),
                    metric_value: count.to_string(),
                    threshold: None,
                    message: format!("检测到 {} 次 OOM Kill, 内存严重不足", count),
                });
            }
        }

        // 检查磁盘错误
        let disk_err_cmd =
            "dmesg 2>/dev/null | grep -ci 'I/O error\\|disk error\\|ext4.*error' || echo 0";
        let result = executor
            .execute_command(&[host.to_string()], disk_err_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
            if count > 0 {
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Log,
                    status: HealthStatus::Critical,
                    metric_name: "磁盘I/O错误".to_string(),
                    metric_value: count.to_string(),
                    threshold: None,
                    message: format!("dmesg 中发现 {} 条磁盘错误日志", count),
                });
            }
        }

        Ok(items)
    }

    /// 网络巡检
    async fn check_network(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 活跃连接数
        let conn_cmd = "ss -s | awk '/^TCP:/ {print $2}'";
        let result = executor
            .execute_command(&[host.to_string()], conn_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            if hr.exit_code == 0 {
                let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Network,
                    status: HealthStatus::Healthy,
                    metric_name: "TCP连接数".to_string(),
                    metric_value: count.to_string(),
                    threshold: None,
                    message: format!("当前 {} 个 TCP 连接", count),
                });
            }
        }

        // DNS 解析
        let dns_cmd = "host google.com >/dev/null 2>&1 && echo OK || echo FAIL";
        let result = executor
            .execute_command(&[host.to_string()], dns_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            let status = if hr.stdout.trim() == "OK" {
                HealthStatus::Healthy
            } else {
                HealthStatus::Warning
            };
            items.push(HealthCheckItem {
                check_type: HealthCheckType::Network,
                status,
                metric_name: "DNS解析".to_string(),
                metric_value: hr.stdout.trim().to_string(),
                threshold: None,
                message: if hr.stdout.trim() == "OK" {
                    "DNS 解析正常".to_string()
                } else {
                    "DNS 解析失败, 网络可能异常".to_string()
                },
            });
        }

        Ok(items)
    }

    /// 安全巡检
    async fn check_security(
        &self,
        executor: &TaskExecutor,
        host: &str,
    ) -> Result<Vec<HealthCheckItem>> {
        let mut items = Vec::new();

        // 最近失败SSH登录
        let ssh_cmd = "journalctl -u sshd --since '24 hours ago' 2>/dev/null | grep -c 'Failed password' || echo 0";
        let result = executor
            .execute_command(&[host.to_string()], ssh_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            let count: u32 = hr.stdout.trim().parse().unwrap_or(0);
            let status = if count > 100 {
                HealthStatus::Critical
            } else if count > 10 {
                HealthStatus::Warning
            } else {
                HealthStatus::Healthy
            };
            items.push(HealthCheckItem {
                check_type: HealthCheckType::Security,
                status,
                metric_name: "SSH暴力破解".to_string(),
                metric_value: format!("{}次/24h", count),
                threshold: Some("警告: >10, 严重: >100".to_string()),
                message: if count > 10 {
                    format!("24小时内 {} 次SSH登录失败, 可能遭受暴力破解", count)
                } else {
                    "SSH登录正常".to_string()
                },
            });
        }

        // 可疑用户 (UID=0 但非 root)
        let uid_cmd = "awk -F: '$3==0 && $1!=\"root\" {print $1}' /etc/passwd";
        let result = executor
            .execute_command(&[host.to_string()], uid_cmd)
            .await?;
        if let Some(hr) = result.host_results.first() {
            let users: Vec<&str> = hr.stdout.trim().lines().filter(|l| !l.is_empty()).collect();
            if !users.is_empty() {
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Security,
                    status: HealthStatus::Critical,
                    metric_name: "可疑UID=0用户".to_string(),
                    metric_value: users.join(", "),
                    threshold: None,
                    message: format!("发现非root的UID=0用户: {}, 可能存在后门", users.join(", ")),
                });
            } else {
                items.push(HealthCheckItem {
                    check_type: HealthCheckType::Security,
                    status: HealthStatus::Healthy,
                    metric_name: "UID=0用户".to_string(),
                    metric_value: "仅root".to_string(),
                    threshold: None,
                    message: "UID=0 用户检查正常".to_string(),
                });
            }
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_thresholds() -> HealthCheckThresholds {
        HealthCheckThresholds::default()
    }

    #[test]
    fn test_thresholds_default() {
        let t = default_thresholds();
        assert_eq!(t.cpu_warn_percent, 80.0);
        assert_eq!(t.cpu_crit_percent, 95.0);
        assert_eq!(t.mem_warn_percent, 80.0);
        assert_eq!(t.disk_crit_percent, 95.0);
        assert_eq!(t.zombie_warn_count, 1);
    }

    #[test]
    fn test_thresholds_custom() {
        let t = HealthCheckThresholds {
            cpu_warn_percent: 90.0,
            cpu_crit_percent: 99.0,
            mem_warn_percent: 85.0,
            mem_crit_percent: 98.0,
            disk_warn_percent: 90.0,
            disk_crit_percent: 98.0,
            load_warn_multiplier: 3.0,
            load_crit_multiplier: 5.0,
            swap_warn_percent: 70.0,
            inode_warn_percent: 90.0,
            zombie_warn_count: 5,
        };
        assert_eq!(t.cpu_warn_percent, 90.0);
        assert_eq!(t.zombie_warn_count, 5);
    }

    #[test]
    fn test_health_check_type_variants() {
        let types = vec![
            HealthCheckType::Cpu,
            HealthCheckType::Memory,
            HealthCheckType::Disk,
            HealthCheckType::Process,
            HealthCheckType::Log,
            HealthCheckType::Network,
            HealthCheckType::Security,
            HealthCheckType::All,
        ];
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn test_health_status_ordering() {
        assert_ne!(HealthStatus::Healthy, HealthStatus::Warning);
        assert_ne!(HealthStatus::Warning, HealthStatus::Critical);
        assert_ne!(HealthStatus::Critical, HealthStatus::Unknown);
    }

    #[test]
    fn test_health_check_report_creation() {
        let report = HealthCheckReport {
            host: "server1".to_string(),
            check_time: Utc::now(),
            overall_status: HealthStatus::Healthy,
            checks: vec![],
            recommendations: vec![],
        };
        assert_eq!(report.host, "server1");
        assert_eq!(report.overall_status, HealthStatus::Healthy);
        assert!(report.checks.is_empty());
    }

    #[test]
    fn test_health_check_item_creation() {
        let item = HealthCheckItem {
            check_type: HealthCheckType::Cpu,
            status: HealthStatus::Warning,
            metric_name: "CPU使用率".to_string(),
            metric_value: "85.0%".to_string(),
            threshold: Some("80%".to_string()),
            message: "CPU使用率偏高".to_string(),
        };
        assert_eq!(item.check_type, HealthCheckType::Cpu);
        assert_eq!(item.status, HealthStatus::Warning);
        assert!(item.threshold.is_some());
    }

    #[test]
    fn test_health_check_serialization_roundtrip() {
        let report = HealthCheckReport {
            host: "test".to_string(),
            check_time: Utc::now(),
            overall_status: HealthStatus::Healthy,
            checks: vec![HealthCheckItem {
                check_type: HealthCheckType::Memory,
                status: HealthStatus::Healthy,
                metric_name: "内存".to_string(),
                metric_value: "50%".to_string(),
                threshold: None,
                message: "正常".to_string(),
            }],
            recommendations: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: HealthCheckReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "test");
        assert_eq!(deserialized.checks.len(), 1);
    }

    #[test]
    fn test_health_status_serialization() {
        let statuses = vec![
            HealthStatus::Healthy,
            HealthStatus::Warning,
            HealthStatus::Critical,
            HealthStatus::Unknown,
        ];
        for s in statuses {
            let json = serde_json::to_string(&s).unwrap();
            let d: HealthStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, d);
        }
    }

    #[test]
    fn test_health_checker_creation() {
        let checker = HealthChecker::new(default_thresholds());
        assert_eq!(checker.thresholds.cpu_warn_percent, 80.0);
    }
}
