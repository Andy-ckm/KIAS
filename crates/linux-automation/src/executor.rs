//! SSH 远程执行引擎

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::Utc;
use std::time::Instant;
use uuid::Uuid;

/// SSH会话
#[derive(Debug, Clone)]
pub struct SshSession {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub key_path: Option<std::path::PathBuf>,
    pub connected: bool,
    pub last_used: chrono::DateTime<chrono::Utc>,
}

#[allow(dead_code)]
/// 任务执行器
pub struct TaskExecutor {
    ssh_key_path: Option<std::path::PathBuf>,
    sessions: std::collections::HashMap<String, SshSession>,
    max_sessions: usize,
    session_timeout: std::time::Duration,
}

#[allow(dead_code)]
impl TaskExecutor {
    /// 创建新的执行器
    pub fn new(config: &LinuxAutomationConfig) -> Result<Self> {
        Ok(Self {
            ssh_key_path: config.ssh_key_path.clone(),
            sessions: std::collections::HashMap::new(),
            max_sessions: 10,
            session_timeout: std::time::Duration::from_secs(300),
        })
    }

    /// 获取或创建SSH会话
    fn get_session(&mut self, host: &str) -> &mut SshSession {
        if !self.sessions.contains_key(host) {
            let session = SshSession {
                host: host.to_string(),
                port: 22,
                username: "root".to_string(),
                key_path: self.ssh_key_path.clone(),
                connected: false,
                last_used: chrono::Utc::now(),
            };
            self.sessions.insert(host.to_string(), session);
        }
        // SAFETY: 上面 insert 保证了 key 存在
        self.sessions
            .get_mut(host)
            .expect("session key just inserted above")
    }

    /// 清理过期会话
    fn cleanup_sessions(&mut self) {
        let now = chrono::Utc::now();
        if let Ok(timeout) = chrono::Duration::from_std(self.session_timeout) {
            self.sessions
                .retain(|_, session| now.signed_duration_since(session.last_used) < timeout);
        }
    }

    /// 在远程主机上执行命令
    pub async fn execute_command(
        &self,
        hosts: &[String],
        command: &str,
    ) -> Result<AutomationResult> {
        let _start = Instant::now();
        let mut host_results = Vec::new();

        for host in hosts {
            let result = self.execute_on_host(host, command).await?;
            host_results.push(result);
        }

        let all_success = host_results.iter().all(|r| r.status == TaskStatus::Success);
        let any_success = host_results.iter().any(|r| r.status == TaskStatus::Success);

        let status = if all_success {
            TaskStatus::Success
        } else if any_success {
            TaskStatus::PartialSuccess
        } else {
            TaskStatus::Failed
        };

        Ok(AutomationResult {
            task_id: Uuid::new_v4(),
            task_type: "CustomCommand".to_string(),
            status,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results,
            summary: format!("执行命令: {} 在 {} 个主机上", command, hosts.len()),
            audit_trail: vec![],
        })
    }

    /// 在单个主机上执行命令
    async fn execute_on_host(&self, host: &str, command: &str) -> Result<HostResult> {
        let start = Instant::now();

        // 构建SSH命令
        let mut ssh_cmd = tokio::process::Command::new("ssh");
        ssh_cmd.arg("-o").arg("StrictHostKeyChecking=no");
        ssh_cmd.arg("-o").arg("ConnectTimeout=10");
        ssh_cmd.arg("-o").arg("ServerAliveInterval=15");
        ssh_cmd.arg("-o").arg("ServerAliveCountMax=3");

        // 添加SSH密钥（如果指定）
        if let Some(key_path) = &self.ssh_key_path {
            if key_path.exists() {
                ssh_cmd.arg("-i").arg(key_path);
            }
        }

        ssh_cmd.arg(host).arg(command);

        // 执行命令，带超时
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5分钟超时
            ssh_cmd.output(),
        )
        .await
        .map_err(|_| AutomationError::CommandExecution("SSH 执行超时 (300秒)".to_string()))?
        .map_err(|e| AutomationError::CommandExecution(format!("SSH 执行失败: {}", e)))?;

        let duration = start.elapsed().as_millis() as u64;

        Ok(HostResult {
            host: host.to_string(),
            status: if output.status.success() {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            },
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: duration,
        })
    }

    /// 安装补丁
    pub async fn install_patches(
        &self,
        hosts: &[String],
        packages: &[String],
    ) -> Result<AutomationResult> {
        let command = format!("yum install -y {}", packages.join(" "));
        self.execute_command(hosts, &command).await
    }

    /// 部署配置
    pub async fn deploy_config(
        &self,
        hosts: &[String],
        playbook: &str,
    ) -> Result<AutomationResult> {
        let command = format!("ansible-playbook {}", playbook);
        self.execute_command(hosts, &command).await
    }

    /// 安全更新
    pub async fn security_update(&self, hosts: &[String]) -> Result<AutomationResult> {
        self.execute_command(hosts, "yum update --security -y")
            .await
    }

    /// 收集日志
    pub async fn collect_logs(
        &self,
        hosts: &[String],
        log_paths: &[String],
    ) -> Result<AutomationResult> {
        let command = format!("cat {}", log_paths.join(" "));
        self.execute_command(hosts, &command).await
    }

    /// 清理磁盘
    pub async fn cleanup_disk(
        &self,
        hosts: &[String],
        targets: &[String],
    ) -> Result<AutomationResult> {
        let command = format!("rm -rf {}", targets.join(" "));
        self.execute_command(hosts, &command).await
    }

    /// 重启服务
    pub async fn restart_service(
        &self,
        hosts: &[String],
        service: &str,
    ) -> Result<AutomationResult> {
        let command = format!("systemctl restart {}", service);
        self.execute_command(hosts, &command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_executor() -> (TaskExecutor, TempDir) {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let executor = TaskExecutor::new(&config).unwrap();
        (executor, tmp)
    }

    #[test]
    fn test_create_executor() {
        let (executor, _tmp) = create_test_executor();
        assert!(executor.ssh_key_path.is_none());
    }

    #[test]
    fn test_executor_with_ssh_key() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: Some(tmp.path().join("id_rsa")),
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let executor = TaskExecutor::new(&config).unwrap();
        assert!(executor.ssh_key_path.is_some());
    }

    #[test]
    fn test_task_type_variants() {
        let types = [
            TaskType::ComplianceScan {
                profile: "cis".to_string(),
                hosts: vec![],
            },
            TaskType::PatchInstall {
                packages: vec![],
                hosts: vec![],
            },
            TaskType::ConfigDeploy {
                playbook: "test.yml".to_string(),
                hosts: vec![],
            },
            TaskType::SecurityUpdate { hosts: vec![] },
            TaskType::LogCollection {
                hosts: vec![],
                log_paths: vec![],
            },
            TaskType::DiskCleanup {
                hosts: vec![],
                targets: vec![],
            },
            TaskType::ServiceRestart {
                service: "nginx".to_string(),
                hosts: vec![],
            },
            TaskType::CustomCommand {
                command: "ls".to_string(),
                hosts: vec![],
            },
        ];
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn test_automation_statistics_default() {
        let stats = AutomationStatistics {
            total_tasks: 0,
            successful_tasks: 0,
            failed_tasks: 0,
            pending_tasks: 0,
            compliance_score: 0.0,
            audit_entries: 0,
            last_scan_time: None,
        };
        assert_eq!(stats.total_tasks, 0);
        assert!(stats.last_scan_time.is_none());
    }

    #[test]
    fn test_ssh_session_fields() {
        let session = SshSession {
            host: "10.0.0.1".to_string(),
            port: 22,
            username: "admin".to_string(),
            key_path: None,
            connected: false,
            last_used: chrono::Utc::now(),
        };
        assert_eq!(session.host, "10.0.0.1");
        assert_eq!(session.port, 22);
        assert_eq!(session.username, "admin");
        assert!(!session.connected);
        assert!(session.key_path.is_none());
    }

    #[test]
    fn test_ssh_session_clone() {
        let session = SshSession {
            host: "server1".to_string(),
            port: 2222,
            username: "root".to_string(),
            key_path: Some(std::path::PathBuf::from("/root/.ssh/id_rsa")),
            connected: true,
            last_used: chrono::Utc::now(),
        };
        let cloned = session.clone();
        assert_eq!(cloned.host, session.host);
        assert_eq!(cloned.port, session.port);
        assert_eq!(cloned.connected, session.connected);
    }

    #[test]
    fn test_executor_sessions_start_empty() {
        let (executor, _tmp) = create_test_executor();
        assert!(executor.sessions.is_empty());
    }

    #[test]
    fn test_executor_max_sessions_default() {
        let (executor, _tmp) = create_test_executor();
        assert_eq!(executor.max_sessions, 10);
    }

    #[test]
    fn test_executor_session_timeout_default() {
        let (executor, _tmp) = create_test_executor();
        assert_eq!(
            executor.session_timeout,
            std::time::Duration::from_secs(300)
        );
    }

    // ============================================================
    // SshSession tests
    // ============================================================

    #[test]
    fn test_ssh_session_debug() {
        let session = SshSession {
            host: "server1".to_string(),
            port: 22,
            username: "root".to_string(),
            key_path: None,
            connected: false,
            last_used: chrono::Utc::now(),
        };
        let debug = format!("{:?}", session);
        assert!(debug.contains("SshSession"));
        assert!(debug.contains("server1"));
    }

    #[test]
    fn test_ssh_session_with_key_path() {
        let session = SshSession {
            host: "server1".to_string(),
            port: 22,
            username: "deploy".to_string(),
            key_path: Some(std::path::PathBuf::from("/home/deploy/.ssh/id_ed25519")),
            connected: true,
            last_used: chrono::Utc::now(),
        };
        assert!(session.key_path.is_some());
        assert!(session.connected);
        assert_eq!(session.username, "deploy");
    }

    // ============================================================
    // TaskExecutor creation tests
    // ============================================================

    #[test]
    fn test_executor_with_ssh_key_path() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: Some(tmp.path().join("id_rsa")),
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["server1".to_string()],
            compliance_tool: ComplianceTool::Lynis,
        };
        let executor = TaskExecutor::new(&config).unwrap();
        assert!(executor.ssh_key_path.is_some());
        assert!(executor.sessions.is_empty());
        assert_eq!(executor.max_sessions, 10);
    }

    // ============================================================
    // AutomationStatistics tests
    // ============================================================

    #[test]
    fn test_automation_statistics_with_values() {
        let stats = AutomationStatistics {
            total_tasks: 100,
            successful_tasks: 85,
            failed_tasks: 10,
            pending_tasks: 5,
            compliance_score: 92.5,
            audit_entries: 500,
            last_scan_time: Some(Utc::now()),
        };
        assert_eq!(stats.total_tasks, 100);
        assert_eq!(stats.successful_tasks, 85);
        assert_eq!(stats.failed_tasks, 10);
        assert!(stats.last_scan_time.is_some());
    }

    #[test]
    fn test_automation_statistics_clone() {
        let stats = AutomationStatistics {
            total_tasks: 50,
            successful_tasks: 45,
            failed_tasks: 3,
            pending_tasks: 2,
            compliance_score: 88.0,
            audit_entries: 200,
            last_scan_time: None,
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total_tasks, stats.total_tasks);
        assert_eq!(cloned.compliance_score, stats.compliance_score);
    }

    #[test]
    fn test_automation_statistics_debug() {
        let stats = AutomationStatistics {
            total_tasks: 10,
            successful_tasks: 8,
            failed_tasks: 1,
            pending_tasks: 1,
            compliance_score: 95.0,
            audit_entries: 50,
            last_scan_time: None,
        };
        let debug = format!("{:?}", stats);
        assert!(debug.contains("AutomationStatistics"));
        assert!(debug.contains("10"));
    }

    // ============================================================
    // HostResult tests
    // ============================================================

    #[test]
    fn test_host_result_creation() {
        let hr = HostResult {
            host: "server1".to_string(),
            status: TaskStatus::Success,
            stdout: "ok".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 150,
        };
        assert_eq!(hr.host, "server1");
        assert_eq!(hr.exit_code, 0);
        assert_eq!(hr.duration_ms, 150);
    }

    #[test]
    fn test_host_result_clone() {
        let hr = HostResult {
            host: "server1".to_string(),
            status: TaskStatus::Failed,
            stdout: String::new(),
            stderr: "error".to_string(),
            exit_code: 1,
            duration_ms: 500,
        };
        let cloned = hr.clone();
        assert_eq!(cloned.host, hr.host);
        assert_eq!(cloned.exit_code, hr.exit_code);
    }

    #[test]
    fn test_host_result_debug() {
        let hr = HostResult {
            host: "server1".to_string(),
            status: TaskStatus::Success,
            stdout: "output".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 100,
        };
        let debug = format!("{:?}", hr);
        assert!(debug.contains("HostResult"));
        assert!(debug.contains("server1"));
    }

    // ============================================================
    // AutomationResult tests
    // ============================================================

    #[test]
    fn test_automation_result_creation() {
        let result = AutomationResult {
            task_id: Uuid::new_v4(),
            task_type: "test".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "done".to_string(),
            audit_trail: vec![],
        };
        assert_eq!(result.task_type, "test");
        assert_eq!(result.status, TaskStatus::Success);
    }

    #[test]
    fn test_automation_result_clone() {
        let result = AutomationResult {
            task_id: Uuid::new_v4(),
            task_type: "test".to_string(),
            status: TaskStatus::Failed,
            started_at: Utc::now(),
            completed_at: None,
            host_results: vec![],
            summary: "failed".to_string(),
            audit_trail: vec![],
        };
        let cloned = result.clone();
        assert_eq!(cloned.task_type, result.task_type);
        assert_eq!(cloned.status, result.status);
    }

    #[test]
    fn test_automation_result_debug() {
        let result = AutomationResult {
            task_id: Uuid::new_v4(),
            task_type: "test".to_string(),
            status: TaskStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            host_results: vec![],
            summary: "running".to_string(),
            audit_trail: vec![],
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("AutomationResult"));
    }

    // ============================================================
    // TaskType serialization
    // ============================================================

    #[test]
    fn test_task_type_serialization_roundtrip() {
        let types = vec![
            TaskType::ComplianceScan {
                profile: "cis".to_string(),
                hosts: vec!["h1".to_string()],
            },
            TaskType::PatchInstall {
                packages: vec!["vim".to_string()],
                hosts: vec!["h1".to_string()],
            },
            TaskType::ConfigDeploy {
                playbook: "test.yml".to_string(),
                hosts: vec!["h1".to_string()],
            },
            TaskType::SecurityUpdate {
                hosts: vec!["h1".to_string()],
            },
            TaskType::CustomCommand {
                command: "ls".to_string(),
                hosts: vec!["h1".to_string()],
            },
        ];
        for tt in types {
            let json = serde_json::to_string(&tt).unwrap();
            let deserialized: TaskType = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, json2);
        }
    }

    // ============================================================
    // TaskPriority & TaskStatus tests
    // ============================================================

    #[test]
    fn test_task_priority_variants() {
        let priorities = [
            TaskPriority::Low,
            TaskPriority::Normal,
            TaskPriority::High,
            TaskPriority::Critical,
        ];
        assert_eq!(priorities.len(), 4);
    }

    #[test]
    fn test_task_priority_partial_eq() {
        assert_eq!(TaskPriority::Low, TaskPriority::Low);
        assert_ne!(TaskPriority::Low, TaskPriority::High);
        assert_ne!(TaskPriority::Normal, TaskPriority::Critical);
    }

    #[test]
    fn test_task_priority_serialization() {
        let priorities = vec![
            TaskPriority::Low,
            TaskPriority::Normal,
            TaskPriority::High,
            TaskPriority::Critical,
        ];
        for p in priorities {
            let json = serde_json::to_string(&p).unwrap();
            let deserialized: TaskPriority = serde_json::from_str(&json).unwrap();
            assert_eq!(p, deserialized);
        }
    }

    #[test]
    fn test_task_status_variants() {
        let statuses = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Success,
            TaskStatus::Failed,
            TaskStatus::PartialSuccess,
            TaskStatus::Cancelled,
        ];
        assert_eq!(statuses.len(), 6);
    }

    #[test]
    fn test_task_status_partial_eq() {
        assert_eq!(TaskStatus::Success, TaskStatus::Success);
        assert_ne!(TaskStatus::Success, TaskStatus::Failed);
        assert_ne!(TaskStatus::Pending, TaskStatus::Running);
    }

    #[test]
    fn test_task_status_serialization() {
        let statuses = vec![
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Success,
            TaskStatus::Failed,
            TaskStatus::PartialSuccess,
            TaskStatus::Cancelled,
        ];
        for s in statuses {
            let json = serde_json::to_string(&s).unwrap();
            let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, deserialized);
        }
    }
}
