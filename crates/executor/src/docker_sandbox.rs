//! Hardened Docker-backed execution for bounded Agent Runs.
//!
//! This executor intentionally uses the Docker CLI instead of mounting arbitrary
//! host paths or forwarding the caller environment. Images must already exist on
//! the runner (`--pull=never`), networking is disabled, the root filesystem is
//! read-only, Linux capabilities are removed, and resource limits are enforced.

use async_trait::async_trait;
use chrono::Utc;
use kias_common::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::runtime::TaskExecutor;
use super::task::{Task, TaskResult, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerSandboxPolicy {
    pub timeout: Duration,
    pub memory_bytes: u64,
    pub cpus: f64,
    pub pids_limit: u32,
    pub tmpfs_bytes: u64,
    pub max_output_bytes: usize,
}

impl Default for DockerSandboxPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            memory_bytes: 128 * 1024 * 1024,
            cpus: 0.5,
            pids_limit: 64,
            tmpfs_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerResourceUsage {
    pub peak_memory_bytes: u64,
    pub peak_cpu_percent: f64,
    pub configured_memory_bytes: u64,
    pub configured_cpus: f64,
    pub configured_pids_limit: u32,
}

pub struct DockerSandboxExecutor {
    policy: DockerSandboxPolicy,
}

impl DockerSandboxExecutor {
    pub fn new(policy: DockerSandboxPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &DockerSandboxPolicy {
        &self.policy
    }

    pub fn container_name(task_id: &str) -> String {
        let suffix: String = task_id
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .take(48)
            .collect();
        format!("kias-run-{suffix}")
    }

    pub async fn cancel(task_id: &str) -> KiasResult<bool> {
        let name = Self::container_name(task_id);
        let output = Command::new("docker")
            .args(["stop", "--time", "1", &name])
            .output()
            .await
            .map_err(|error| KiasError::ExternalService(format!("docker stop failed: {error}")))?;

        if output.status.success() {
            return Ok(true);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such container") {
            Ok(false)
        } else {
            Err(KiasError::ExternalService(format!(
                "docker stop failed: {}",
                stderr.trim()
            )))
        }
    }

    async fn remove_container(name: &str) {
        let _ = Command::new("docker")
            .args(["rm", "--force", name])
            .output()
            .await;
    }

    async fn sample_usage(name: &str) -> Option<(u64, f64)> {
        let output = Command::new("docker")
            .args(["stats", "--no-stream", "--format", "{{json .}}", name])
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let memory = value
            .get("MemUsage")
            .and_then(serde_json::Value::as_str)
            .map(parse_memory_usage)
            .unwrap_or_default();
        let cpu = value
            .get("CPUPerc")
            .and_then(serde_json::Value::as_str)
            .and_then(|raw| raw.trim_end_matches('%').parse::<f64>().ok())
            .unwrap_or_default();
        Some((memory, cpu))
    }

    async fn inspect_oom_killed(name: &str) -> bool {
        let output = match Command::new("docker")
            .args(["inspect", "--format", "{{json .State}}", name])
            .output()
            .await
        {
            Ok(output) if output.status.success() => output,
            _ => return false,
        };
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .ok()
            .and_then(|value| value.get("OOMKilled").and_then(serde_json::Value::as_bool))
            .unwrap_or(false)
    }
}

impl Default for DockerSandboxExecutor {
    fn default() -> Self {
        Self::new(DockerSandboxPolicy::default())
    }
}

#[async_trait]
impl TaskExecutor for DockerSandboxExecutor {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
        let started_at = Utc::now();
        let started = Instant::now();
        let name = Self::container_name(&task.id);

        let image = task
            .payload
            .get("image")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| KiasError::BadRequest("Agent Run requires an image".to_string()))?;
        let command = task
            .payload
            .get("command")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .ok_or_else(|| KiasError::BadRequest("Agent Run requires a command".to_string()))?;
        let input = task
            .payload
            .get("input")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let timeout = task.timeout.unwrap_or(self.policy.timeout);

        Self::remove_container(&name).await;

        let memory = self.policy.memory_bytes.to_string();
        let cpus = self.policy.cpus.to_string();
        let pids = self.policy.pids_limit.to_string();
        let tmpfs = format!(
            "/tmp:rw,noexec,nosuid,nodev,size={}",
            self.policy.tmpfs_bytes
        );
        let label = format!("kias.run_id={}", task.id);

        let create_output = Command::new("docker")
            .args([
                "create",
                "--pull=never",
                "--interactive",
                "--name",
                &name,
                "--hostname",
                "kias-sandbox",
                "--network",
                "none",
                "--read-only",
                "--cap-drop",
                "ALL",
                "--security-opt",
                "no-new-privileges:true",
                "--pids-limit",
                &pids,
                "--memory",
                &memory,
                "--memory-swap",
                &memory,
                "--cpus",
                &cpus,
                "--tmpfs",
                &tmpfs,
                "--user",
                "65534:65534",
                "--label",
                &label,
                "--env",
                "KIAS_INPUT_MODE=stdin",
                image,
            ])
            .args(&command)
            .output()
            .await
            .map_err(|error| {
                KiasError::ExternalService(format!("failed to create sandbox container: {error}"))
            })?;

        if !create_output.status.success() {
            let error = truncate_utf8(
                &String::from_utf8_lossy(&create_output.stderr),
                self.policy.max_output_bytes,
            );
            Self::remove_container(&name).await;
            return Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Failed,
                output: Some(serde_json::json!({
                    "sandbox": sandbox_evidence(&name, &self.policy),
                    "resource_usage": DockerResourceUsage {
                        configured_memory_bytes: self.policy.memory_bytes,
                        configured_cpus: self.policy.cpus,
                        configured_pids_limit: self.policy.pids_limit,
                        ..Default::default()
                    },
                })),
                error: Some(format!(
                    "sandbox admission passed but container creation failed: {error}"
                )),
                started_at,
                completed_at: Utc::now(),
            });
        }

        let mut process = Command::new("docker");
        process.args(["start", "--attach", "--interactive", &name]);
        process.stdin(std::process::Stdio::piped());
        process.stdout(std::process::Stdio::piped());
        process.stderr(std::process::Stdio::piped());
        process.kill_on_drop(true);

        let mut child = match process.spawn() {
            Ok(child) => child,
            Err(error) => {
                Self::remove_container(&name).await;
                return Err(KiasError::ExternalService(format!(
                    "failed to start sandbox container: {error}"
                )));
            }
        };
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes()).await?;
            if !input.ends_with('\n') {
                stdin.write_all(b"\n").await?;
            }
            stdin.shutdown().await?;
        }

        let mut wait = Box::pin(child.wait_with_output());
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        let mut peak_memory_bytes = 0_u64;
        let mut peak_cpu_percent = 0_f64;
        let mut timed_out = false;

        let process_output = loop {
            tokio::select! {
                result = &mut wait => {
                    break result.map_err(|error| KiasError::ExternalService(format!("sandbox attach failed: {error}")))?;
                }
                _ = &mut deadline => {
                    timed_out = true;
                    let _ = Command::new("docker").args(["stop", "--time", "1", &name]).output().await;
                    break tokio::time::timeout(Duration::from_secs(10), &mut wait)
                        .await
                        .map_err(|_| KiasError::ExternalService("sandbox did not stop after timeout".to_string()))?
                        .map_err(|error| KiasError::ExternalService(format!("sandbox attach failed: {error}")))?;
                }
                _ = interval.tick() => {
                    if let Some((memory_bytes, cpu_percent)) = Self::sample_usage(&name).await {
                        peak_memory_bytes = peak_memory_bytes.max(memory_bytes);
                        peak_cpu_percent = peak_cpu_percent.max(cpu_percent);
                    }
                }
            }
        };

        let oom_killed = Self::inspect_oom_killed(&name).await;
        let exit_code = process_output.status.code().unwrap_or(-1);
        let stdout = truncate_utf8(
            &String::from_utf8_lossy(&process_output.stdout),
            self.policy.max_output_bytes,
        );
        let stderr = truncate_utf8(
            &String::from_utf8_lossy(&process_output.stderr),
            self.policy.max_output_bytes,
        );
        Self::remove_container(&name).await;

        let status = if exit_code == 0 && !timed_out && !oom_killed {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        let error = match status {
            TaskStatus::Completed => None,
            _ if timed_out => Some(format!(
                "Agent Run timed out after {}ms",
                timeout.as_millis()
            )),
            _ if oom_killed => Some("Agent Run exceeded its memory limit".to_string()),
            _ => Some(format!("Agent Run exited with code {exit_code}")),
        };

        Ok(TaskResult {
            task_id: task.id.clone(),
            status,
            output: Some(serde_json::json!({
                "stdout": stdout,
                "stderr": stderr,
                "exit_code": exit_code,
                "duration_ms": started.elapsed().as_millis() as u64,
                "timed_out": timed_out,
                "oom_killed": oom_killed,
                "resource_usage": DockerResourceUsage {
                    peak_memory_bytes,
                    peak_cpu_percent,
                    configured_memory_bytes: self.policy.memory_bytes,
                    configured_cpus: self.policy.cpus,
                    configured_pids_limit: self.policy.pids_limit,
                },
                "sandbox": sandbox_evidence(&name, &self.policy),
            })),
            error,
            started_at,
            completed_at: Utc::now(),
        })
    }
}

fn sandbox_evidence(name: &str, policy: &DockerSandboxPolicy) -> serde_json::Value {
    serde_json::json!({
        "runtime": "docker-cli",
        "container_name": name,
        "network": "none",
        "root_filesystem": "read-only",
        "capabilities": "dropped-all",
        "no_new_privileges": true,
        "user": "65534:65534",
        "host_mounts": false,
        "image_pull": "never",
        "tmpfs_bytes": policy.tmpfs_bytes,
    })
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}\n[TRUNCATED]", &value[..boundary])
}

fn parse_memory_usage(raw: &str) -> u64 {
    let value = raw.split('/').next().unwrap_or(raw).trim();
    let split_at = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let number = value[..split_at]
        .trim()
        .parse::<f64>()
        .unwrap_or_default();
    let unit = value[split_at..].trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "b" | "" => 1_f64,
        "kb" => 1_000_f64,
        "kib" => 1_024_f64,
        "mb" => 1_000_000_f64,
        "mib" => 1_048_576_f64,
        "gb" => 1_000_000_000_f64,
        "gib" => 1_073_741_824_f64,
        _ => 1_f64,
    };
    (number * multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_names_are_bounded_and_safe() {
        let name = DockerSandboxExecutor::container_name("ABC/123:unsafe");
        assert_eq!(name, "kias-run-abc-123-unsafe");
        assert!(name.len() <= 57);
    }

    #[test]
    fn parses_docker_memory_units() {
        assert_eq!(parse_memory_usage("12.5MiB / 128MiB"), 13_107_200);
        assert_eq!(parse_memory_usage("2GiB / 4GiB"), 2_147_483_648);
    }

    #[test]
    fn truncation_preserves_utf8_boundaries() {
        assert_eq!(truncate_utf8("你好世界", 4), "你\n[TRUNCATED]");
    }
}
