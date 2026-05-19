//! SSH 远程执行引擎

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::Utc;
use std::time::Instant;
use uuid::Uuid;

/// 任务执行器
pub struct TaskExecutor {
    ssh_key_path: Option<std::path::PathBuf>,
}

impl TaskExecutor {
    /// 创建新的执行器
    pub fn new(config: &LinuxAutomationConfig) -> Result<Self> {
        Ok(Self {
            ssh_key_path: config.ssh_key_path.clone(),
        })
    }

    /// 在远程主机上执行命令
    pub async fn execute_command(
        &self,
        hosts: &[String],
        command: &str,
    ) -> Result<AutomationResult> {
        let start = Instant::now();
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

        // 使用 tokio::process 执行 SSH 命令
        let output = tokio::process::Command::new("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("ConnectTimeout=10")
            .arg(host)
            .arg(command)
            .output()
            .await
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
        self.execute_command(hosts, "yum update --security -y").await
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
}
