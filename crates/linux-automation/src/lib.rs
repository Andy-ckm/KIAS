//! # AgentGuard Linux 自动化维护模块
//!
//! 医药企业 Linux 服务器自动化维护，包括：
//! - SSH 远程执行引擎
//! - 合规扫描（CIS Benchmark、OpenSCAP）
//! - 补丁管理
//! - 配置管理
//! - 审计日志持久化
//! - RBAC 权限控制

pub mod audit;
pub mod config;
pub mod error;
pub mod executor;
pub mod models;
pub mod queue;
pub mod scanner;

pub use config::LinuxAutomationConfig;
pub use error::{AutomationError, Result};
pub use executor::TaskExecutor;
pub use models::*;
pub use queue::TaskQueue;
pub use scanner::ComplianceScanner;

/// Linux 自动化引擎
pub struct LinuxAutomation {
    config: LinuxAutomationConfig,
    executor: TaskExecutor,
    queue: TaskQueue,
    scanner: ComplianceScanner,
}

impl LinuxAutomation {
    /// 创建新的自动化引擎
    pub fn new(config: LinuxAutomationConfig) -> Result<Self> {
        let executor = TaskExecutor::new(&config)?;
        let queue = TaskQueue::new(&config.database_path)?;
        let scanner = ComplianceScanner::new(&config)?;

        Ok(Self {
            config,
            executor,
            queue,
            scanner,
        })
    }

    /// 执行自动化任务
    pub async fn execute_task(&self, task: AutomationTask) -> Result<AutomationResult> {
        // 1. 记录任务到队列
        let task_id = self.queue.enqueue(&task)?;

        // 2. 执行任务
        let result = match &task.task_type {
            TaskType::ComplianceScan { profile, hosts } => {
                self.scanner.scan(hosts, profile).await?
            }
            TaskType::PatchInstall { packages, hosts } => {
                self.executor.install_patches(hosts, packages).await?
            }
            TaskType::ConfigDeploy { playbook, hosts } => {
                self.executor.deploy_config(hosts, playbook).await?
            }
            TaskType::SecurityUpdate { hosts } => {
                self.executor.security_update(hosts).await?
            }
            TaskType::LogCollection { hosts, log_paths } => {
                self.executor.collect_logs(hosts, log_paths).await?
            }
            TaskType::DiskCleanup { hosts, targets } => {
                self.executor.cleanup_disk(hosts, targets).await?
            }
            TaskType::ServiceRestart { service, hosts } => {
                self.executor.restart_service(hosts, service).await?
            }
            TaskType::CustomCommand { command, hosts } => {
                self.executor.execute_command(hosts, command).await?
            }
        };

        // 3. 更新任务状态
        self.queue.update_status(task_id, &result.status)?;

        // 4. 记录审计日志
        audit::record_task_execution(&self.config.database_path, &task, &result)?;

        Ok(result)
    }

    /// 获取任务历史
    pub fn get_task_history(&self, limit: Option<usize>) -> Result<Vec<AutomationResult>> {
        self.queue.get_history(limit)
    }

    /// 获取合规报告
    pub fn get_compliance_report(&self, host: &str) -> Result<ComplianceReport> {
        self.scanner.get_report(host)
    }

    /// 获取审计日志
    pub fn get_audit_log(&self, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        audit::get_audit_log(&self.config.database_path, limit)
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> Result<AutomationStatistics> {
        let queue_stats = self.queue.get_statistics()?;
        let audit_stats = audit::get_statistics(&self.config.database_path)?;

        Ok(AutomationStatistics {
            total_tasks: queue_stats.total,
            successful_tasks: queue_stats.successful,
            failed_tasks: queue_stats.failed,
            pending_tasks: queue_stats.pending,
            compliance_score: self.scanner.get_average_score()?,
            audit_entries: audit_stats.total_entries,
            last_scan_time: self.scanner.get_last_scan_time()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_config() -> (LinuxAutomationConfig, TempDir) {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        (config, tmp)
    }

    #[test]
    fn test_create_automation() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config);
        assert!(automation.is_ok());
    }

    #[test]
    fn test_get_statistics_empty() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let stats = automation.get_statistics().unwrap();
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.successful_tasks, 0);
        assert_eq!(stats.failed_tasks, 0);
    }

    #[test]
    fn test_get_task_history_empty() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let history = automation.get_task_history(Some(10)).unwrap();
        assert!(history.is_empty());
    }
}
