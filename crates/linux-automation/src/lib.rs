//! # AgentGuard Linux 自动化维护模块
//!
//! 医药企业 Linux 服务器自动化维护，包括：
//! - SSH 远程执行引擎
//! - 合规扫描（CIS Benchmark、OpenSCAP）
//! - 补丁管理
//! - 配置管理
//! - 审计日志持久化
//! - RBAC 权限控制
//! - R023: 日常巡检（CPU/内存/磁盘/进程/日志/网络/安全）
//! - R024: 服务器初始化（软件安装/用户配置/安全加固）
//! - R025: Docker 容器运维管理
//! - R026: Kubernetes 集群运维

pub mod audit;
pub mod config;
pub mod config_mgmt;
pub mod docker_ops;
pub mod error;
pub mod executor;
pub mod health_check;
pub mod k8s_ops;
pub mod models;
pub mod operation_hub;
pub mod patch;
pub mod provisioning;
pub mod queue;
pub mod rbac;
pub mod scanner;

pub use audit::AuditLog;
pub use error::{AutomationError, Result};
pub use executor::TaskExecutor;
pub use models::*;
pub use patch::PatchManager;
pub use queue::TaskQueue;
pub use scanner::ComplianceScanner;

use chrono::Utc;

/// Linux 自动化引擎
pub struct LinuxAutomation {
    #[allow(dead_code)]
    config: LinuxAutomationConfig,
    executor: TaskExecutor,
    queue: TaskQueue,
    scanner: ComplianceScanner,
    audit: AuditLog,
}

impl LinuxAutomation {
    /// 创建新的自动化引擎
    pub fn new(config: LinuxAutomationConfig) -> Result<Self> {
        let executor = TaskExecutor::new(&config)?;
        let queue = TaskQueue::new(&config.database_path)?;
        let scanner = ComplianceScanner::new(&config)?;
        let audit = AuditLog::new(&config.database_path)?;

        Ok(Self {
            config,
            executor,
            queue,
            scanner,
            audit,
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
            TaskType::SecurityUpdate { hosts } => self.executor.security_update(hosts).await?,
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
            TaskType::HealthCheck { hosts, checks } => {
                let hosts_count = hosts.len();
                let checker = health_check::HealthChecker::new(
                    health_check::HealthCheckThresholds::default(),
                );
                let mut all_results = Vec::new();
                for host in hosts {
                    let report = checker
                        .check_all(&self.executor, host, checks, &self.audit)
                        .await?;
                    all_results.push(report);
                }
                let checks_count: usize = all_results.iter().map(|r| r.checks.len()).sum();
                let summary = format!("巡检 {} 台主机, 共 {} 项检查", hosts_count, checks_count);
                AutomationResult {
                    task_id,
                    task_type: "HealthCheck".to_string(),
                    status: if all_results
                        .iter()
                        .any(|r| r.overall_status == HealthStatus::Critical)
                    {
                        TaskStatus::Failed
                    } else {
                        TaskStatus::Success
                    },
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    host_results: vec![],
                    summary,
                    audit_trail: vec![],
                }
            }
            TaskType::ServerProvision { hosts, template } => {
                let hosts_count = hosts.len();
                let mut all_results = Vec::new();
                for host in hosts {
                    let report = provisioning::Provisioner::provision(
                        &self.executor,
                        host,
                        template,
                        &self.audit,
                    )
                    .await?;
                    all_results.push(report);
                }
                let summary = format!("初始化 {} 台主机, 模板: {}", hosts_count, template.name);
                AutomationResult {
                    task_id,
                    task_type: "ServerProvision".to_string(),
                    status: if all_results
                        .iter()
                        .all(|r| r.overall_status == TaskStatus::Success)
                    {
                        TaskStatus::Success
                    } else {
                        TaskStatus::PartialSuccess
                    },
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    host_results: vec![],
                    summary,
                    audit_trail: vec![],
                }
            }
            TaskType::DockerOps { hosts, action } => {
                let hosts_count = hosts.len();
                let mut all_results = Vec::new();
                for host in hosts {
                    let result =
                        docker_ops::DockerOps::execute(&self.executor, host, action, &self.audit)
                            .await?;
                    all_results.push(result);
                }
                let summary = format!("Docker操作 {} 台主机", hosts_count);
                AutomationResult {
                    task_id,
                    task_type: "DockerOps".to_string(),
                    status: if all_results.iter().all(|r| r.status == TaskStatus::Success) {
                        TaskStatus::Success
                    } else {
                        TaskStatus::PartialSuccess
                    },
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    host_results: vec![],
                    summary,
                    audit_trail: vec![],
                }
            }
            TaskType::K8sOps { context, action } => {
                let result = k8s_ops::K8sOps::execute(
                    &self.executor,
                    "localhost",
                    context,
                    action,
                    &self.audit,
                )
                .await?;
                AutomationResult {
                    task_id,
                    task_type: "K8sOps".to_string(),
                    status: result.status.clone(),
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    host_results: vec![],
                    summary: format!("K8s操作: {:?}", action),
                    audit_trail: vec![],
                }
            }
            TaskType::BackupOps { hosts, action: _ } => {
                // TODO: 实现备份操作
                let summary = format!("备份操作 {} 台主机 (待实现)", hosts.len());
                AutomationResult {
                    task_id,
                    task_type: "BackupOps".to_string(),
                    status: TaskStatus::Failed,
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                    host_results: vec![],
                    summary,
                    audit_trail: vec![],
                }
            }
        };

        // 3. 更新任务状态
        self.queue.update_status(task_id, &result.status)?;

        // 4. 记录审计日志
        self.audit.record_task_execution(&task, &result)?;

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
        self.audit.get_audit_log(limit)
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> Result<AutomationStatistics> {
        let queue_stats = self.queue.get_statistics()?;
        let audit_stats = self.audit.get_statistics()?;

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
    }

    #[test]
    fn test_get_task_history_empty() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let history = automation.get_task_history(Some(10)).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_get_audit_log_empty() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let log = automation.get_audit_log(Some(10)).unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn test_get_statistics_all_zero() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let stats = automation.get_statistics().unwrap();
        assert_eq!(stats.total_tasks, 0);
        assert_eq!(stats.successful_tasks, 0);
        assert_eq!(stats.failed_tasks, 0);
        assert_eq!(stats.pending_tasks, 0);
        assert_eq!(stats.audit_entries, 0);
        assert!(stats.last_scan_time.is_none());
    }

    #[test]
    fn test_create_automation_with_ssh_key() {
        let tmp = TempDir::new().unwrap();
        let key_path = tmp.path().join("id_rsa");
        std::fs::write(&key_path, "fake-key").unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: Some(key_path),
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()],
            compliance_tool: ComplianceTool::Lynis,
        };
        let automation = LinuxAutomation::new(config);
        assert!(automation.is_ok());
    }

    #[test]
    fn test_get_task_history_limit_none() {
        let (config, _tmp) = create_test_config();
        let automation = LinuxAutomation::new(config).unwrap();
        let history = automation.get_task_history(None).unwrap();
        assert!(history.is_empty());
    }
}
