//! Process management for KIAS server daemon mode.
//!
//! Handles PID file management, daemonization, signal sending,
//! and process lifecycle (start/stop/restart/status).

use std::fs;
use std::path::{Path, PathBuf};

/// Default PID file location
const DEFAULT_PID_FILE: &str = "/tmp/kias-server.pid";

/// Process manager for KIAS server
pub struct ProcessManager {
    pid_file: PathBuf,
}

impl ProcessManager {
    /// Create a new process manager with default PID file location
    pub fn new() -> Self {
        Self {
            pid_file: PathBuf::from(DEFAULT_PID_FILE),
        }
    }

    /// Create with custom PID file path
    pub fn with_pid_file(path: impl Into<PathBuf>) -> Self {
        Self {
            pid_file: path.into(),
        }
    }

    /// Get the PID file path
    pub fn pid_file(&self) -> &Path {
        &self.pid_file
    }

    /// Write the current process PID to the PID file
    pub fn write_pid(&self) -> Result<(), String> {
        let pid = std::process::id();
        fs::write(&self.pid_file, pid.to_string()).map_err(|e| {
            format!(
                "无法写入 PID 文件 {}: {}",
                self.pid_file.display(),
                e
            )
        })
    }

    /// Read the PID from the PID file
    pub fn read_pid(&self) -> Result<u32, String> {
        let content = fs::read_to_string(&self.pid_file).map_err(|_| {
            format!(
                "PID 文件不存在: {} (服务可能未启动)",
                self.pid_file.display()
            )
        })?;
        content
            .trim()
            .parse::<u32>()
            .map_err(|e| format!("PID 文件内容无效: {}", e))
    }

    /// Remove the PID file
    pub fn remove_pid_file(&self) {
        let _ = fs::remove_file(&self.pid_file);
    }

    /// Check if a process with the given PID is running
    pub fn is_process_running(pid: u32) -> bool {
        // Use kill -0 to check process existence without sending a signal
        #[cfg(unix)]
        {
            use std::process::Command;
            Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            // On non-Unix, assume running if PID file exists
            true
        }
    }

    /// Send SIGTERM to a process
    pub fn send_sigterm(pid: u32) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::process::Command;
            let output = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .output()
                .map_err(|e| format!("无法发送 SIGTERM: {}", e))?;
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("SIGTERM 发送失败: {}", stderr.trim()))
            }
        }
        #[cfg(not(unix))]
        {
            Err("SIGTERM 仅支持 Unix 系统".to_string())
        }
    }

    /// Wait for a process to exit (poll with timeout)
    pub fn wait_for_exit(pid: u32, timeout_secs: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        while start.elapsed() < timeout {
            if !Self::is_process_running(pid) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        false
    }

    /// Start the server as a daemon process.
    ///
    /// Re-launches the current binary with the same arguments but without --daemon,
    /// redirecting stdout/stderr to a log file.
    pub fn start_daemon(
        &self,
        binary: &str,
        args: &[String],
        log_file: Option<&Path>,
    ) -> Result<(), String> {
        #[cfg(unix)]
        {
            use std::process::Command;

            // Build the command: same binary, same args, minus --daemon/-d
            let filtered_args: Vec<&str> = args
                .iter()
                .filter(|a| *a != "--daemon" && *a != "-d" && !a.starts_with("--daemon="))
                .map(|s| s.as_str())
                .collect();

            let mut cmd = Command::new(binary);
            cmd.args(&filtered_args);

            // Redirect output to log file or /dev/null
            if let Some(log_path) = log_file {
                let log_file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .map_err(|e| format!("无法打开日志文件 {}: {}", log_path.display(), e))?;
                use std::os::unix::io::AsRawFd;
                let fd = log_file.as_raw_fd();
                unsafe {
                    use std::os::unix::io::FromRawFd;
                    cmd.stdout(std::process::Stdio::from_raw_fd(fd));
                    cmd.stderr(std::process::Stdio::from_raw_fd(
                        fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(log_path)
                            .map_err(|e| format!("无法打开日志文件: {}", e))?
                            .as_raw_fd(),
                    ));
                }
            } else {
                cmd.stdout(std::process::Stdio::null());
                cmd.stderr(std::process::Stdio::null());
            }

            // Detach from terminal
            cmd.stdin(std::process::Stdio::null());

            let child = cmd
                .spawn()
                .map_err(|e| format!("无法启动守护进程: {}", e))?;

            // Write the child PID
            fs::write(&self.pid_file, child.id().to_string())
                .map_err(|e| format!("无法写入 PID 文件: {}", e))?;

            Ok(())
        }
        #[cfg(not(unix))]
        {
            Err("守护进程模式仅支持 Unix 系统".to_string())
        }
    }

    /// Stop a running server process
    pub fn stop(&self) -> Result<StopResult, String> {
        let pid = self.read_pid()?;

        if !Self::is_process_running(pid) {
            self.remove_pid_file();
            return Ok(StopResult::AlreadyStopped);
        }

        Self::send_sigterm(pid)?;

        // Wait for graceful shutdown
        if Self::wait_for_exit(pid, 10) {
            self.remove_pid_file();
            Ok(StopResult::Stopped)
        } else {
            // Force kill
            #[cfg(unix)]
            {
                use std::process::Command;
                let _ = Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .status();
            }
            self.remove_pid_file();
            Ok(StopResult::ForceKilled)
        }
    }

    /// Get server status
    pub fn status(&self, server_url: &str) -> ServerStatus {
        let pid_info = match self.read_pid() {
            Ok(pid) => {
                if Self::is_process_running(pid) {
                    Some(pid)
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        ServerStatus {
            pid: pid_info,
            pid_file_exists: self.pid_file.exists(),
            health: None, // Caller should check health endpoint separately
            server_url: server_url.to_string(),
        }
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a stop operation
#[derive(Debug)]
pub enum StopResult {
    /// Process was stopped gracefully (SIGTERM)
    Stopped,
    /// Process was force-killed (SIGKILL)
    ForceKilled,
    /// Process was not running
    AlreadyStopped,
}

/// Server status information
#[derive(Debug)]
pub struct ServerStatus {
    /// PID of the running server (if running)
    pub pid: Option<u32>,
    /// Whether the PID file exists
    pub pid_file_exists: bool,
    /// Health check result (if available)
    pub health: Option<bool>,
    /// Server URL
    pub server_url: String,
}

impl ServerStatus {
    /// Check health endpoint
    pub async fn check_health(&mut self) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build();
        if let Ok(client) = client {
            if let Ok(resp) = client.get(format!("{}/health", self.server_url)).send().await {
                self.health = Some(resp.status().is_success());
            }
        }
    }

    /// Format status for display
    pub fn display(&self) -> String {
        let mut lines = Vec::new();

        if let Some(pid) = self.pid {
            lines.push(format!("进程: 运行中 (PID {})", pid));
        } else {
            lines.push("进程: 未运行".to_string());
        }

        lines.push(format!("PID 文件: {}", if self.pid_file_exists {
            "存在"
        } else {
            "不存在"
        }));

        match self.health {
            Some(true) => lines.push("健康检查: 正常".to_string()),
            Some(false) => lines.push("健康检查: 异常".to_string()),
            None => lines.push("健康检查: 无法连接".to_string()),
        }

        lines.push(format!("服务地址: {}", self.server_url));

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_write_and_read_pid() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("test.pid");
        let pm = ProcessManager::with_pid_file(&pid_file);

        pm.write_pid().unwrap();
        let pid = pm.read_pid().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn test_read_pid_missing_file() {
        let pm = ProcessManager::with_pid_file("/tmp/nonexistent_kias_test.pid");
        assert!(pm.read_pid().is_err());
    }

    #[test]
    fn test_read_pid_invalid_content() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("bad.pid");
        fs::write(&pid_file, "not_a_number").unwrap();
        let pm = ProcessManager::with_pid_file(&pid_file);
        assert!(pm.read_pid().is_err());
    }

    #[test]
    fn test_is_process_running_self() {
        let pid = std::process::id();
        assert!(ProcessManager::is_process_running(pid));
    }

    #[test]
    fn test_is_process_running_nonexistent() {
        // PID 999999 is very unlikely to exist
        assert!(!ProcessManager::is_process_running(999999));
    }

    #[test]
    fn test_remove_pid_file() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("remove_test.pid");
        fs::write(&pid_file, "12345").unwrap();
        let pm = ProcessManager::with_pid_file(&pid_file);
        assert!(pid_file.exists());
        pm.remove_pid_file();
        assert!(!pid_file.exists());
    }

    #[test]
    fn test_stop_already_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("stop_test.pid");
        // No PID file = already stopped
        let pm = ProcessManager::with_pid_file(&pid_file);
        // read_pid will fail, which means not running
        assert!(pm.read_pid().is_err());
    }

    #[test]
    fn test_status_no_process() {
        let pm = ProcessManager::with_pid_file("/tmp/nonexistent_kias_status_test.pid");
        let status = pm.status("http://localhost:8080");
        assert!(status.pid.is_none());
        assert!(!status.pid_file_exists);
        assert!(status.health.is_none());
    }

    #[test]
    fn test_default_pid_file() {
        let pm = ProcessManager::new();
        assert_eq!(pm.pid_file(), Path::new(DEFAULT_PID_FILE));
    }
}
