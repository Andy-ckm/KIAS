use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use kias_common::{KiasError, KiasResult};
use kias_data_store::{Repository, TaskRepository, TaskRow};
use kias_executor::{
    DockerSandboxExecutor, DockerSandboxPolicy, Task, TaskExecutor, TaskStatus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::models::agent::{Agent, AgentSpec};
use crate::models::run::{
    ReplayRunRequest, RunCheckpoint, RunConstraints, RunEvidence, RunLineage, RunLogs,
    RunPolicyDecision, RunRecord, RunStatus, StartRunRequest,
};

const RUN_TASK_TYPE: &str = "agent_run";
const POLICY_VERSION: &str = "core-run-policy-v1";
const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_TIMEOUT_SECONDS: u64 = 300;
const MAX_RETRIES: u32 = 3;
const MAX_CPU: f64 = 1.0;
const MAX_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_CPU: f64 = 0.5;
const DEFAULT_MEMORY_BYTES: u64 = 128 * 1024 * 1024;
const PIDS_LIMIT: u32 = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAgentSnapshot {
    name: String,
    image: String,
    command: Vec<String>,
    spec_sha256: String,
}

/// Durable Run metadata deliberately excludes the caller input. Only a digest
/// and byte count are persisted so prompts, messages, and PII do not become a
/// second unmanaged copy in the control-plane database.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRunInput {
    input_sha256: String,
    input_bytes: usize,
    agent: StoredAgentSnapshot,
    policy: RunPolicyDecision,
    lineage: RunLineage,
}

pub struct RunService {
    repository: Arc<TaskRepository>,
    allowed_images: BTreeSet<String>,
    runtime_available: bool,
}

impl RunService {
    pub fn new(repository: Arc<TaskRepository>) -> Self {
        let allowed_images = std::env::var("KIAS_RUN_ALLOWED_IMAGES")
            .unwrap_or_else(|_| "busybox:1.36".to_string())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        let runtime_available = std::process::Command::new("docker")
            .arg("info")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        Self {
            repository,
            allowed_images,
            runtime_available,
        }
    }

    pub fn runtime_available(&self) -> bool {
        self.runtime_available
    }

    pub fn allowed_images(&self) -> Vec<String> {
        self.allowed_images.iter().cloned().collect()
    }

    pub async fn mark_interrupted_runs(&self) -> KiasResult<usize> {
        let mut interrupted = 0;
        for status in ["queued", "running"] {
            for mut row in self.repository.get_by_status(status).await? {
                if row.task_type != RUN_TASK_TYPE {
                    continue;
                }
                let _ = DockerSandboxExecutor::cancel(&row.id).await;
                row.status = RunStatus::Interrupted.as_storage().to_string();
                row.error_message = Some(
                    "Control plane restarted before the Agent Run reached a terminal state"
                        .to_string(),
                );
                row.completed_at = Some(Utc::now().to_rfc3339());
                self.repository.update(&row).await?;
                interrupted += 1;
            }
        }
        Ok(interrupted)
    }

    pub async fn create_run(
        self: &Arc<Self>,
        agent: &Agent,
        request: StartRunRequest,
    ) -> KiasResult<RunRecord> {
        let (policy, executor_policy) = self.evaluate(agent, &request);
        let input_sha256 = sha256_hex(request.input.as_bytes());
        let input_bytes = request.input.len();
        let stored = StoredRunInput {
            input_sha256,
            input_bytes,
            agent: snapshot_agent(&agent.spec)?,
            policy,
            lineage: RunLineage::default(),
        };

        self.persist_and_maybe_spawn(agent.id.clone(), stored, executor_policy, request.input)
            .await
    }

    pub async fn list_runs(&self) -> KiasResult<Vec<RunRecord>> {
        let rows = self.repository.list(None, None).await?;
        rows.into_iter()
            .filter(|row| row.task_type == RUN_TASK_TYPE)
            .map(run_record_from_row)
            .collect()
    }

    pub async fn get_run(&self, id: &str) -> KiasResult<RunRecord> {
        run_record_from_row(self.get_run_row(id).await?)
    }

    pub async fn get_logs(&self, id: &str) -> KiasResult<RunLogs> {
        let row = self.get_run_row(id).await?;
        let output = parse_output(&row);
        let final_execution = output.get("final").cloned().unwrap_or_default();
        let stdout = final_execution
            .get("stdout")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let stderr = final_execution
            .get("stderr")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok(RunLogs {
            truncated: stdout.contains("[TRUNCATED]") || stderr.contains("[TRUNCATED]"),
            stdout,
            stderr,
        })
    }

    pub async fn get_evidence(&self, id: &str) -> KiasResult<RunEvidence> {
        let row = self.get_run_row(id).await?;
        let run = run_record_from_row(row.clone())?;
        let output = parse_output(&row);
        let attempts = output
            .get("attempts")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let final_execution = output.get("final").cloned().filter(|value| !value.is_null());
        let unsigned = serde_json::json!({
            "run": &run,
            "attempts": &attempts,
            "final_execution": &final_execution,
        });
        let evidence_sha256 = sha256_hex(&serde_json::to_vec(&unsigned)?);

        Ok(RunEvidence {
            run,
            attempts,
            final_execution,
            evidence_sha256,
        })
    }

    pub async fn checkpoint(&self, id: &str) -> KiasResult<RunCheckpoint> {
        let row = self.get_run_row(id).await?;
        let stored = parse_stored_input(&row)?;
        Ok(RunCheckpoint {
            run_id: row.id,
            agent_id: row.agent_id,
            replayable: RunStatus::from_storage(&row.status).is_terminal(),
            status: RunStatus::from_storage(&row.status),
            input_sha256: stored.input_sha256,
            agent_spec_sha256: stored.agent.spec_sha256,
            created_at: row.created_at,
            note: "Replay checkpoint: KIAS persists the admitted AgentSpec, input digest, policy decision, and lineage. Retry or recovery must resupply the identical input; process memory is never snapshotted."
                .to_string(),
        })
    }

    pub async fn cancel(&self, id: &str) -> KiasResult<RunRecord> {
        let mut row = self.get_run_row(id).await?;
        if RunStatus::from_storage(&row.status).is_terminal() {
            return run_record_from_row(row);
        }

        let _ = DockerSandboxExecutor::cancel(id).await?;
        row.status = RunStatus::Cancelled.as_storage().to_string();
        row.error_message = Some("Agent Run cancelled by an operator".to_string());
        row.completed_at = Some(Utc::now().to_rfc3339());
        self.repository.update(&row).await?;
        run_record_from_row(row)
    }

    pub async fn retry(
        self: &Arc<Self>,
        id: &str,
        request: ReplayRunRequest,
    ) -> KiasResult<RunRecord> {
        let source = self.get_run_row(id).await?;
        let status = RunStatus::from_storage(&source.status);
        if !matches!(status, RunStatus::Failed | RunStatus::Cancelled) {
            return Err(KiasError::Conflict(
                "Only failed or cancelled Agent Runs can be retried".to_string(),
            ));
        }
        self.replay(source, request.input, true).await
    }

    pub async fn recover(
        self: &Arc<Self>,
        id: &str,
        request: ReplayRunRequest,
    ) -> KiasResult<RunRecord> {
        let source = self.get_run_row(id).await?;
        if RunStatus::from_storage(&source.status) != RunStatus::Interrupted {
            return Err(KiasError::Conflict(
                "Only interrupted Agent Runs can be recovered".to_string(),
            ));
        }
        self.replay(source, request.input, false).await
    }

    async fn replay(
        self: &Arc<Self>,
        source: TaskRow,
        input: String,
        is_retry: bool,
    ) -> KiasResult<RunRecord> {
        let mut stored = parse_stored_input(&source)?;
        verify_replay_input(&stored, &input)?;
        stored.lineage = if is_retry {
            RunLineage {
                retry_of: Some(source.id.clone()),
                recovery_of: None,
            }
        } else {
            RunLineage {
                retry_of: None,
                recovery_of: Some(source.id.clone()),
            }
        };
        let executor_policy = executor_policy_from_decision(&stored.policy);
        self.persist_and_maybe_spawn(source.agent_id, stored, executor_policy, input)
            .await
    }

    async fn persist_and_maybe_spawn(
        self: &Arc<Self>,
        agent_id: String,
        stored: StoredRunInput,
        executor_policy: DockerSandboxPolicy,
        execution_input: String,
    ) -> KiasResult<RunRecord> {
        let now = Utc::now().to_rfc3339();
        let mut row = TaskRow::new(agent_id, format!("Agent Run: {}", stored.agent.name));
        row.task_type = RUN_TASK_TYPE.to_string();
        row.input = serde_json::to_string(&stored)?;
        row.timeout_seconds = Some(stored.policy.constraints.timeout_seconds as i32);
        row.max_retries = stored.policy.constraints.max_retries as i32;
        row.status = if stored.policy.allowed {
            RunStatus::Queued.as_storage().to_string()
        } else {
            RunStatus::Failed.as_storage().to_string()
        };
        if !stored.policy.allowed {
            row.error_message = Some(format!(
                "Policy admission denied: {}",
                stored.policy.reasons.join("; ")
            ));
            row.completed_at = Some(now);
        }

        self.repository.create(&row).await?;
        let record = run_record_from_row(row.clone())?;
        if stored.policy.allowed {
            let service = Arc::clone(self);
            let id = row.id.clone();
            tokio::spawn(async move {
                if let Err(error) = service
                    .execute_run(&id, executor_policy, execution_input)
                    .await
                {
                    tracing::error!(run_id = %id, error = %error, "Agent Run execution failed");
                    let _ = service.fail_run(&id, error.to_string()).await;
                }
            });
        }
        Ok(record)
    }

    async fn execute_run(
        &self,
        id: &str,
        executor_policy: DockerSandboxPolicy,
        execution_input: String,
    ) -> KiasResult<()> {
        let mut row = self.get_run_row(id).await?;
        let stored = parse_stored_input(&row)?;
        let executor = DockerSandboxExecutor::new(executor_policy);
        let mut attempts = Vec::new();
        let max_retries = row.max_retries.max(0) as u32;

        for attempt in 0..=max_retries {
            let latest = self.get_run_row(id).await?;
            if RunStatus::from_storage(&latest.status) == RunStatus::Cancelled {
                return Ok(());
            }

            row = latest;
            row.status = RunStatus::Running.as_storage().to_string();
            row.retry_count = attempt as i32;
            if row.started_at.is_none() {
                row.started_at = Some(Utc::now().to_rfc3339());
            }
            self.repository.update(&row).await?;

            let task = Task {
                id: row.id.clone(),
                name: row.name.clone(),
                agent_id: row.agent_id.clone(),
                payload: serde_json::json!({
                    "image": stored.agent.image.clone(),
                    "command": stored.agent.command.clone(),
                    "input": execution_input,
                }),
                created_at: Utc::now(),
                timeout: row
                    .timeout_seconds
                    .map(|seconds| Duration::from_secs(seconds.max(1) as u64)),
            };

            let result = executor.execute(&task).await?;
            let result_status = result.status.clone();
            attempts.push(serde_json::json!({
                "attempt": attempt,
                "status": result_status.clone(),
                "started_at": result.started_at,
                "completed_at": result.completed_at,
                "error": result.error.clone(),
                "execution": result.output.clone(),
            }));

            let latest = self.get_run_row(id).await?;
            if RunStatus::from_storage(&latest.status) == RunStatus::Cancelled {
                return Ok(());
            }
            row = latest;

            if result_status == TaskStatus::Completed {
                row.status = RunStatus::Succeeded.as_storage().to_string();
                row.error_message = None;
                row.output = Some(serde_json::to_string(&serde_json::json!({
                    "attempts": attempts,
                    "final": result.output,
                }))?);
                row.completed_at = Some(Utc::now().to_rfc3339());
                self.repository.update(&row).await?;
                return Ok(());
            }

            if attempt == max_retries {
                row.status = RunStatus::Failed.as_storage().to_string();
                row.error_message = result.error.clone();
                row.output = Some(serde_json::to_string(&serde_json::json!({
                    "attempts": attempts,
                    "final": result.output,
                }))?);
                row.completed_at = Some(Utc::now().to_rfc3339());
                self.repository.update(&row).await?;
                return Ok(());
            }
        }
        Ok(())
    }

    async fn fail_run(&self, id: &str, error: String) -> KiasResult<()> {
        let mut row = self.get_run_row(id).await?;
        if RunStatus::from_storage(&row.status) == RunStatus::Cancelled {
            return Ok(());
        }
        row.status = RunStatus::Failed.as_storage().to_string();
        row.error_message = Some(error);
        row.completed_at = Some(Utc::now().to_rfc3339());
        self.repository.update(&row).await
    }

    async fn get_run_row(&self, id: &str) -> KiasResult<TaskRow> {
        let row = self
            .repository
            .get_by_id(id)
            .await?
            .ok_or_else(|| KiasError::NotFound(format!("Agent Run {id} not found")))?;
        if row.task_type != RUN_TASK_TYPE {
            return Err(KiasError::NotFound(format!("Agent Run {id} not found")));
        }
        Ok(row)
    }

    fn evaluate(
        &self,
        agent: &Agent,
        request: &StartRunRequest,
    ) -> (RunPolicyDecision, DockerSandboxPolicy) {
        let timeout_seconds = request.timeout_seconds.unwrap_or(30);
        let max_retries = request.max_retries.unwrap_or(0);
        let cpu = agent
            .spec
            .resource_request
            .as_ref()
            .and_then(|request| request.cpu.as_deref())
            .and_then(parse_cpu)
            .unwrap_or(DEFAULT_CPU);
        let memory_bytes = agent
            .spec
            .resource_request
            .as_ref()
            .and_then(|request| request.memory.as_deref())
            .and_then(parse_memory)
            .unwrap_or(DEFAULT_MEMORY_BYTES);

        let mut reasons = Vec::new();
        if !self.runtime_available {
            reasons.push("Docker runner is unavailable on this KIAS instance".to_string());
        }
        if agent.spec.labels.get("kias.io/execution").map(String::as_str) != Some("enabled") {
            reasons.push("AgentSpec must opt in with label kias.io/execution=enabled".to_string());
        }
        if !self.allowed_images.contains(&agent.spec.image) {
            reasons.push(format!(
                "image is not in KIAS_RUN_ALLOWED_IMAGES ({})",
                self.allowed_images.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        if agent.spec.image.ends_with(":latest") {
            reasons.push("mutable :latest images are not admitted".to_string());
        }
        if agent.spec.command.is_empty() || agent.spec.command.len() > 32 {
            reasons.push("command must contain between 1 and 32 arguments".to_string());
        }
        if agent
            .spec
            .command
            .iter()
            .any(|argument| argument.is_empty() || argument.len() > 1024)
        {
            reasons.push("command arguments must be non-empty and at most 1024 bytes".to_string());
        }
        if !agent.spec.env.is_empty() {
            reasons.push(
                "AgentSpec env values are not admitted; pass bounded input through stdin"
                    .to_string(),
            );
        }
        if request.input.len() > MAX_INPUT_BYTES {
            reasons.push(format!("input exceeds {MAX_INPUT_BYTES} bytes"));
        }
        if !(1..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
            reasons.push(format!(
                "timeout_seconds must be between 1 and {MAX_TIMEOUT_SECONDS}"
            ));
        }
        if max_retries > MAX_RETRIES {
            reasons.push(format!("max_retries must be at most {MAX_RETRIES}"));
        }
        if cpu <= 0.0 || cpu > MAX_CPU {
            reasons.push(format!("CPU request must be greater than 0 and at most {MAX_CPU}"));
        }
        if memory_bytes == 0 || memory_bytes > MAX_MEMORY_BYTES {
            reasons.push(format!(
                "memory request must be greater than 0 and at most {MAX_MEMORY_BYTES} bytes"
            ));
        }
        if agent
            .spec
            .resource_request
            .as_ref()
            .and_then(|request| request.gpu.as_deref())
            .is_some_and(|gpu| gpu != "0")
        {
            reasons.push("GPU access is not available in the Core sandbox".to_string());
        }

        let executor_policy = DockerSandboxPolicy {
            timeout: Duration::from_secs(timeout_seconds.clamp(1, MAX_TIMEOUT_SECONDS)),
            memory_bytes: memory_bytes.clamp(1, MAX_MEMORY_BYTES),
            cpus: cpu.clamp(0.1, MAX_CPU),
            pids_limit: PIDS_LIMIT,
            tmpfs_bytes: 64 * 1024 * 1024,
            max_output_bytes: 64 * 1024,
        };
        let constraints = RunConstraints {
            image: agent.spec.image.clone(),
            network: "none".to_string(),
            root_filesystem: "read-only".to_string(),
            host_mounts: false,
            no_new_privileges: true,
            cpu_limit: executor_policy.cpus,
            memory_limit_bytes: executor_policy.memory_bytes,
            pids_limit: executor_policy.pids_limit,
            timeout_seconds,
            max_retries,
        };
        (
            RunPolicyDecision {
                allowed: reasons.is_empty(),
                policy_version: POLICY_VERSION.to_string(),
                reasons,
                constraints,
            },
            executor_policy,
        )
    }
}

fn executor_policy_from_decision(decision: &RunPolicyDecision) -> DockerSandboxPolicy {
    DockerSandboxPolicy {
        timeout: Duration::from_secs(decision.constraints.timeout_seconds.max(1)),
        memory_bytes: decision.constraints.memory_limit_bytes,
        cpus: decision.constraints.cpu_limit,
        pids_limit: decision.constraints.pids_limit,
        tmpfs_bytes: 64 * 1024 * 1024,
        max_output_bytes: 64 * 1024,
    }
}

fn snapshot_agent(spec: &AgentSpec) -> KiasResult<StoredAgentSnapshot> {
    let safe_spec = serde_json::json!({
        "name": spec.name,
        "image": spec.image,
        "command": spec.command,
        "resource_request": spec.resource_request,
        "labels": spec.labels,
    });
    Ok(StoredAgentSnapshot {
        name: spec.name.clone(),
        image: spec.image.clone(),
        command: spec.command.clone(),
        spec_sha256: sha256_hex(&serde_json::to_vec(&safe_spec)?),
    })
}

fn parse_stored_input(row: &TaskRow) -> KiasResult<StoredRunInput> {
    serde_json::from_str(&row.input).map_err(Into::into)
}

fn parse_output(row: &TaskRow) -> serde_json::Value {
    row.output
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default()
}

fn run_record_from_row(row: TaskRow) -> KiasResult<RunRecord> {
    let stored = parse_stored_input(&row)?;
    Ok(RunRecord {
        id: row.id,
        agent_id: row.agent_id,
        status: RunStatus::from_storage(&row.status),
        retry_count: row.retry_count.max(0) as u32,
        max_retries: row.max_retries.max(0) as u32,
        timeout_seconds: row.timeout_seconds.unwrap_or(30).max(1) as u64,
        input_sha256: stored.input_sha256,
        input_bytes: stored.input_bytes,
        policy: stored.policy,
        lineage: stored.lineage,
        error: row.error_message,
        created_at: row.created_at,
        started_at: row.started_at,
        completed_at: row.completed_at,
        updated_at: row.updated_at,
    })
}

fn verify_replay_input(stored: &StoredRunInput, input: &str) -> KiasResult<()> {
    let digest = sha256_hex(input.as_bytes());
    if input.len() != stored.input_bytes || digest != stored.input_sha256 {
        return Err(KiasError::Validation(
            "retry/recovery input must exactly match the original input digest".to_string(),
        ));
    }
    Ok(())
}

fn sha256_hex(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

fn parse_cpu(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if let Some(millicores) = value.strip_suffix('m') {
        return millicores.parse::<f64>().ok().map(|number| number / 1000.0);
    }
    value.parse::<f64>().ok()
}

fn parse_memory(raw: &str) -> Option<u64> {
    let value = raw.trim();
    let split_at = value
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(value.len());
    let number = value[..split_at].parse::<f64>().ok()?;
    let unit = value[split_at..].trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1_f64,
        "k" | "kb" => 1_000_f64,
        "ki" | "kib" => 1_024_f64,
        "m" | "mb" => 1_000_000_f64,
        "mi" | "mib" => 1_048_576_f64,
        "g" | "gb" => 1_000_000_000_f64,
        "gi" | "gib" => 1_073_741_824_f64,
        _ => return None,
    };
    Some((number * multiplier) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::ResourceRequest;
    use std::collections::HashMap;

    fn agent() -> Agent {
        Agent::from_spec(AgentSpec {
            name: "runner".to_string(),
            image: "busybox:1.36".to_string(),
            command: vec!["sh".to_string(), "-c".to_string(), "cat".to_string()],
            resource_request: Some(ResourceRequest {
                cpu: Some("500m".to_string()),
                memory: Some("128Mi".to_string()),
                gpu: Some("0".to_string()),
            }),
            labels: HashMap::from([(
                "kias.io/execution".to_string(),
                "enabled".to_string(),
            )]),
            priority: "medium".to_string(),
            env: HashMap::new(),
        })
    }

    #[tokio::test]
    async fn policy_admits_bounded_run() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        kias_data_store::MigrationRunner::new(pool.clone())
            .run_all()
            .await
            .unwrap();
        let mut service = RunService::new(Arc::new(TaskRepository::new(pool)));
        service.runtime_available = true;
        let (decision, _) = service.evaluate(&agent(), &StartRunRequest::default());
        assert!(decision.allowed, "{:?}", decision.reasons);
    }

    #[tokio::test]
    async fn policy_denies_environment_values() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        kias_data_store::MigrationRunner::new(pool.clone())
            .run_all()
            .await
            .unwrap();
        let mut service = RunService::new(Arc::new(TaskRepository::new(pool)));
        service.runtime_available = true;
        let mut agent = agent();
        agent
            .spec
            .env
            .insert("TOKEN".to_string(), "secret".to_string());
        let (decision, _) = service.evaluate(&agent, &StartRunRequest::default());
        assert!(!decision.allowed);
    }

    #[test]
    fn durable_metadata_excludes_raw_input() {
        let stored = StoredRunInput {
            input_sha256: sha256_hex(b"sensitive"),
            input_bytes: 9,
            agent: snapshot_agent(&agent().spec).unwrap(),
            policy: RunPolicyDecision {
                allowed: true,
                policy_version: POLICY_VERSION.to_string(),
                reasons: Vec::new(),
                constraints: RunConstraints {
                    image: "busybox:1.36".to_string(),
                    network: "none".to_string(),
                    root_filesystem: "read-only".to_string(),
                    host_mounts: false,
                    no_new_privileges: true,
                    cpu_limit: 0.5,
                    memory_limit_bytes: DEFAULT_MEMORY_BYTES,
                    pids_limit: PIDS_LIMIT,
                    timeout_seconds: 30,
                    max_retries: 0,
                },
            },
            lineage: RunLineage::default(),
        };
        let json = serde_json::to_string(&stored).unwrap();
        assert!(!json.contains("sensitive"));
        assert!(verify_replay_input(&stored, "sensitive").is_ok());
        assert!(verify_replay_input(&stored, "different").is_err());
    }

    #[test]
    fn parses_resource_requests() {
        assert_eq!(parse_cpu("500m"), Some(0.5));
        assert_eq!(parse_memory("128Mi"), Some(134_217_728));
    }
}
