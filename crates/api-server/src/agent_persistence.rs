use std::collections::HashMap;

use kias_common::{KiasError, KiasResult};
use kias_data_store::models::AgentRow;
use serde::{Deserialize, Serialize};

use crate::models::agent::{Agent, AgentSpec, AgentStatus, ResourceRequest};

const REDACTED_ENV_VALUE: &str = "[REDACTED_AFTER_RESTART]";

#[derive(Debug, Default, Serialize, Deserialize)]
struct PersistedAgentMetadata {
    #[serde(default)]
    command: Vec<String>,
    #[serde(default)]
    resource_request: Option<ResourceRequest>,
    #[serde(default)]
    env_keys: Vec<String>,
    #[serde(default)]
    start_time: Option<String>,
    #[serde(default)]
    restart_count: u32,
}

/// Convert the API Agent representation into the durable SQLite row.
///
/// Environment values are deliberately not persisted. They may contain API keys
/// or other credentials; only the variable names survive a restart. Production
/// deployments should resolve secret values from an external secret provider at
/// execution time rather than store them inside the control-plane database.
pub fn to_row(agent: &Agent) -> KiasResult<AgentRow> {
    let labels = serde_json::to_string(&agent.spec.labels)
        .map_err(|error| KiasError::Storage(format!("serialize agent labels: {error}")))?;
    let metadata = PersistedAgentMetadata {
        command: agent.spec.command.clone(),
        resource_request: agent.spec.resource_request.clone(),
        env_keys: agent.spec.env.keys().cloned().collect(),
        start_time: agent.start_time.clone(),
        restart_count: agent.restart_count,
    };
    let metadata = serde_json::to_string(&metadata)
        .map_err(|error| KiasError::Storage(format!("serialize agent metadata: {error}")))?;

    let mut row = AgentRow::new(&agent.spec.name);
    row.id.clone_from(&agent.id);
    row.status = status_to_storage(&agent.status).to_string();
    row.node_id.clone_from(&agent.node_id);
    row.image = Some(agent.spec.image.clone());
    row.priority = priority_to_storage(&agent.spec.priority);
    row.cpu = agent
        .spec
        .resource_request
        .as_ref()
        .and_then(|request| request.cpu.as_deref())
        .and_then(parse_cpu)
        .unwrap_or_default();
    row.memory_bytes = agent
        .spec
        .resource_request
        .as_ref()
        .and_then(|request| request.memory.as_deref())
        .and_then(parse_memory_bytes)
        .unwrap_or_default();
    row.gpu = agent
        .spec
        .resource_request
        .as_ref()
        .and_then(|request| request.gpu.as_deref())
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or_default();
    row.labels = labels;
    row.metadata = metadata;
    row.created_at.clone_from(&agent.created_at);
    row.updated_at.clone_from(&agent.updated_at);
    Ok(row)
}

/// Convert a durable SQLite row into the API Agent representation.
pub fn from_row(row: AgentRow) -> KiasResult<Agent> {
    let labels: HashMap<String, String> = serde_json::from_str(&row.labels)
        .map_err(|error| KiasError::Storage(format!("deserialize agent labels: {error}")))?;
    let metadata: PersistedAgentMetadata = if row.metadata.trim().is_empty()
        || row.metadata.trim() == "{}"
    {
        PersistedAgentMetadata::default()
    } else {
        serde_json::from_str(&row.metadata)
            .map_err(|error| KiasError::Storage(format!("deserialize agent metadata: {error}")))?
    };

    let env = metadata
        .env_keys
        .into_iter()
        .map(|key| (key, REDACTED_ENV_VALUE.to_string()))
        .collect();

    Ok(Agent {
        id: row.id,
        spec: AgentSpec {
            name: row.name,
            image: row.image.unwrap_or_else(|| "python:3.11".to_string()),
            command: if metadata.command.is_empty() {
                vec!["python".to_string(), "app.py".to_string()]
            } else {
                metadata.command
            },
            resource_request: metadata.resource_request,
            labels,
            priority: priority_from_storage(row.priority).to_string(),
            env,
        },
        status: status_from_storage(&row.status),
        node_id: row.node_id,
        resource_usage: ResourceRequest::default(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        start_time: metadata.start_time,
        restart_count: metadata.restart_count,
    })
}

fn status_to_storage(status: &AgentStatus) -> &'static str {
    match status {
        AgentStatus::Pending => "pending",
        AgentStatus::Scheduled => "scheduled",
        AgentStatus::Running => "running",
        AgentStatus::Succeeded => "succeeded",
        AgentStatus::Failed => "failed",
        AgentStatus::Unknown => "unknown",
    }
}

fn status_from_storage(status: &str) -> AgentStatus {
    match status.trim().to_ascii_lowercase().as_str() {
        "pending" => AgentStatus::Pending,
        "scheduled" => AgentStatus::Scheduled,
        "running" => AgentStatus::Running,
        "succeeded" | "success" => AgentStatus::Succeeded,
        "failed" | "failure" => AgentStatus::Failed,
        _ => AgentStatus::Unknown,
    }
}

fn priority_to_storage(priority: &str) -> i32 {
    match priority.trim().to_ascii_lowercase().as_str() {
        "low" => 25,
        "high" => 75,
        "critical" => 100,
        _ => 50,
    }
}

fn priority_from_storage(priority: i32) -> &'static str {
    match priority {
        value if value >= 90 => "critical",
        value if value >= 65 => "high",
        value if value <= 35 => "low",
        _ => "medium",
    }
}

fn parse_cpu(value: &str) -> Option<f64> {
    let value = value.trim();
    if let Some(millicores) = value.strip_suffix('m') {
        return millicores.parse::<f64>().ok().map(|number| number / 1000.0);
    }
    value.parse::<f64>().ok()
}

fn parse_memory_bytes(value: &str) -> Option<i64> {
    let value = value.trim();
    let units = [
        ("Ti", 1024_i64.pow(4)),
        ("Gi", 1024_i64.pow(3)),
        ("Mi", 1024_i64.pow(2)),
        ("Ki", 1024_i64),
        ("TB", 1000_i64.pow(4)),
        ("GB", 1000_i64.pow(3)),
        ("MB", 1000_i64.pow(2)),
        ("KB", 1000_i64),
        ("B", 1_i64),
    ];
    for (suffix, multiplier) in units {
        if let Some(number) = value.strip_suffix(suffix) {
            return number
                .trim()
                .parse::<f64>()
                .ok()
                .map(|parsed| (parsed * multiplier as f64) as i64);
        }
    }
    value.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_agent_round_trip_redacts_environment_values() {
        let mut env = HashMap::new();
        env.insert("PROVIDER_TOKEN".to_string(), "must-not-persist".to_string());
        let agent = Agent::from_spec(AgentSpec {
            name: "durable-agent".to_string(),
            image: "python:3.11".to_string(),
            command: vec!["python".to_string(), "worker.py".to_string()],
            resource_request: Some(ResourceRequest {
                cpu: Some("500m".to_string()),
                memory: Some("256Mi".to_string()),
                gpu: Some("0".to_string()),
            }),
            labels: HashMap::from([("environment".to_string(), "test".to_string())]),
            priority: "high".to_string(),
            env,
        });

        let row = to_row(&agent).unwrap();
        assert!(!row.metadata.contains("must-not-persist"));
        assert_eq!(row.cpu, 0.5);
        assert_eq!(row.memory_bytes, 256 * 1024 * 1024);

        let restored = from_row(row).unwrap();
        assert_eq!(restored.id, agent.id);
        assert_eq!(restored.spec.command, agent.spec.command);
        assert_eq!(restored.spec.priority, "high");
        assert_eq!(
            restored.spec.env.get("PROVIDER_TOKEN").map(String::as_str),
            Some(REDACTED_ENV_VALUE)
        );
    }

    #[test]
    fn unknown_persisted_status_fails_closed_to_unknown() {
        let mut row = AgentRow::new("unknown-status");
        row.status = "future-state".to_string();
        let restored = from_row(row).unwrap();
        assert_eq!(restored.status, AgentStatus::Unknown);
    }
}
