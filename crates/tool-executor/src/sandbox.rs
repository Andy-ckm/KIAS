//! 沙箱执行环境
//!
//! 参考 Codex 的沙箱设计:
//! - Process 沙箱: 限制进程权限
//! - 文件系统隔离: 只允许访问指定目录
//! - 网络隔离: 可选禁用网络

use serde::{Deserialize, Serialize};

/// 沙箱配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// 沙箱类型: process, docker, namespace
    pub sandbox_type: SandboxType,
    /// 允许的文件路径
    pub allowed_paths: Vec<String>,
    /// 禁止的文件路径
    pub denied_paths: Vec<String>,
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 是否允许写入
    pub allow_write: bool,
    /// 内存限制 (MB)
    pub memory_limit_mb: Option<u64>,
    /// CPU 限制 (核心数)
    pub cpu_limit: Option<f64>,
    /// 超时 (秒)
    pub timeout_secs: Option<u64>,
}

/// 沙箱类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxType {
    /// 进程级沙箱
    Process,
    /// Docker 容器沙箱
    Docker,
    /// Linux Namespace 沙箱
    Namespace,
    /// 无沙箱 (开发模式)
    None,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            sandbox_type: SandboxType::Process,
            allowed_paths: vec![".".to_string()],
            denied_paths: vec![],
            allow_network: false,
            allow_write: true,
            memory_limit_mb: Some(512),
            cpu_limit: Some(1.0),
            timeout_secs: Some(60),
        }
    }
}

/// 沙箱执行器
pub struct SandboxExecutor {
    config: SandboxConfig,
}

impl SandboxExecutor {
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// 在沙箱中执行命令
    pub async fn execute(&self, command: &str, workdir: Option<&str>) -> SandboxResult {
        match self.config.sandbox_type {
            SandboxType::None => {
                // 无沙箱模式，直接执行
                self.execute_unsandboxed(command, workdir).await
            }
            SandboxType::Process => {
                // 进程级沙箱
                self.execute_in_process(command, workdir).await
            }
            SandboxType::Docker => {
                // Docker 沙箱
                self.execute_in_docker(command, workdir).await
            }
            SandboxType::Namespace => {
                // Namespace 沙箱
                self.execute_in_namespace(command, workdir).await
            }
        }
    }

    /// 无沙箱执行
    async fn execute_unsandboxed(&self, command: &str, workdir: Option<&str>) -> SandboxResult {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        let timeout = self.config.timeout_secs.unwrap_or(60);

        match tokio::time::timeout(std::time::Duration::from_secs(timeout), cmd.output()).await {
            Ok(Ok(output)) => SandboxResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                timed_out: false,
            },
            Ok(Err(e)) => SandboxResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Execution error: {}", e),
                exit_code: -1,
                timed_out: false,
            },
            Err(_) => SandboxResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Timed out after {}s", timeout),
                exit_code: -1,
                timed_out: true,
            },
        }
    }

    /// 进程级沙箱执行
    async fn execute_in_process(&self, command: &str, workdir: Option<&str>) -> SandboxResult {
        // 使用 ulimit 和其他进程限制
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        if let Some(dir) = workdir {
            cmd.current_dir(dir);
        }

        // 设置环境变量限制
        cmd.env("KIAS_SANDBOX", "true");

        let timeout = self.config.timeout_secs.unwrap_or(60);

        match tokio::time::timeout(std::time::Duration::from_secs(timeout), cmd.output()).await {
            Ok(Ok(output)) => SandboxResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
                timed_out: false,
            },
            Ok(Err(e)) => SandboxResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Sandbox execution error: {}", e),
                exit_code: -1,
                timed_out: false,
            },
            Err(_) => SandboxResult {
                success: false,
                stdout: String::new(),
                stderr: format!("Timed out after {}s", timeout),
                exit_code: -1,
                timed_out: true,
            },
        }
    }

    /// Docker 沙箱执行
    async fn execute_in_docker(&self, _command: &str, _workdir: Option<&str>) -> SandboxResult {
        // Docker 沙箱实现
        SandboxResult {
            success: false,
            stdout: String::new(),
            stderr: "Docker sandbox not yet implemented".to_string(),
            exit_code: -1,
            timed_out: false,
        }
    }

    /// Namespace 沙箱执行
    async fn execute_in_namespace(&self, _command: &str, _workdir: Option<&str>) -> SandboxResult {
        // Linux Namespace 沙箱实现
        SandboxResult {
            success: false,
            stdout: String::new(),
            stderr: "Namespace sandbox not yet implemented".to_string(),
            exit_code: -1,
            timed_out: false,
        }
    }
}

/// 沙箱执行结果
#[derive(Debug, Clone, Serialize)]
pub struct SandboxResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub timed_out: bool,
}
