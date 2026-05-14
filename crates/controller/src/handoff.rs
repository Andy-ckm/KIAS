//! # Agent Handoff System
//!
//! Enables seamless task transfer between agents, inspired by:
//! - Google A2A protocol's task delegation
//! - Claude Code's subagent patterns
//! - MiniMax Agent Team's worker reassignment
//!
//! Agents can hand off tasks to other agents based on:
//! - Capability gaps (target agent has better skills)
//! - Load balancing (distribute work evenly)
//! - Specialization (route to domain experts)
//! - Error recovery (failover to healthy agents)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use kias_common::a2a::{AgentHealth, AgentSkill, HandoffReason};

/// Handoff policy - controls when and how handoffs happen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffPolicy {
    /// Maximum number of handoffs for a single task
    pub max_handoffs: u32,
    /// Enable automatic handoff on capability mismatch
    pub auto_handoff_on_capability_gap: bool,
    /// Enable load-based handoff
    pub auto_handoff_on_overload: bool,
    /// Load threshold for triggering handoff (0.0 - 1.0)
    pub overload_threshold: f64,
    /// Required skill match score (0.0 - 1.0)
    pub min_skill_match_score: f64,
    /// Timeout for handoff acknowledgment (seconds)
    pub ack_timeout_secs: u64,
}

impl Default for HandoffPolicy {
    fn default() -> Self {
        Self {
            max_handoffs: 3,
            auto_handoff_on_capability_gap: true,
            auto_handoff_on_overload: true,
            overload_threshold: 0.85,
            min_skill_match_score: 0.7,
            ack_timeout_secs: 30,
        }
    }
}

/// A handoff record tracking task transfers between agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRecord {
    /// Unique handoff ID
    pub id: String,
    /// Task being handed off
    pub task_id: String,
    /// Source agent
    pub from_agent: String,
    /// Target agent
    pub to_agent: String,
    /// Reason for handoff
    pub reason: HandoffReason,
    /// Context transferred with the task
    pub context: serde_json::Value,
    /// Required skills for the target
    pub required_skills: Vec<String>,
    /// Current status
    pub status: HandoffStatus,
    /// When handoff was initiated
    pub initiated_at: DateTime<Utc>,
    /// When handoff was completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Number of previous handoffs for this task
    pub handoff_chain_length: u32,
}

/// Handoff status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HandoffStatus {
    /// Handoff requested, waiting for target agent
    Pending,
    /// Target agent accepted the handoff
    Accepted,
    /// Target agent rejected the handoff
    Rejected,
    /// Handoff completed successfully
    Completed,
    /// Handoff failed
    Failed,
    /// Handoff timed out
    TimedOut,
}

/// Agent candidate for handoff
#[derive(Debug, Clone)]
pub struct HandoffCandidate {
    pub agent_id: String,
    pub skill_match_score: f64,
    pub current_load: f64,
    pub health: AgentHealth,
    pub available_skills: Vec<AgentSkill>,
}

/// Handoff manager - orchestrates task transfers
pub struct HandoffManager {
    /// Active handoff records
    records: Arc<RwLock<HashMap<String, HandoffRecord>>>,
    /// Handoff history per task
    task_history: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Handoff policy
    policy: HandoffPolicy,
}

impl HandoffManager {
    pub fn new(policy: HandoffPolicy) -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            task_history: Arc::new(RwLock::new(HashMap::new())),
            policy,
        }
    }

    /// Initiate a handoff
    pub async fn initiate_handoff(
        &self,
        task_id: &str,
        from_agent: &str,
        to_agent: &str,
        reason: HandoffReason,
        context: serde_json::Value,
        required_skills: Vec<String>,
    ) -> Result<String, String> {
        // Check handoff chain length
        let history = self.task_history.read().await;
        let chain_length = history.get(task_id).map(|h| h.len() as u32).unwrap_or(0);

        if chain_length >= self.policy.max_handoffs {
            return Err(format!(
                "Task {} has exceeded maximum handoff count ({})",
                task_id, self.policy.max_handoffs
            ));
        }
        drop(history);

        let handoff_id = uuid::Uuid::new_v4().to_string();
        let record = HandoffRecord {
            id: handoff_id.clone(),
            task_id: task_id.to_string(),
            from_agent: from_agent.to_string(),
            to_agent: to_agent.to_string(),
            reason,
            context,
            required_skills,
            status: HandoffStatus::Pending,
            initiated_at: Utc::now(),
            completed_at: None,
            handoff_chain_length: chain_length + 1,
        };

        // Store the record
        {
            let mut records = self.records.write().await;
            records.insert(handoff_id.clone(), record);
        }

        // Update task history
        {
            let mut history = self.task_history.write().await;
            history
                .entry(task_id.to_string())
                .or_default()
                .push(handoff_id.clone());
        }

        tracing::info!(
            handoff_id = %handoff_id,
            task_id = %task_id,
            from = %from_agent,
            to = %to_agent,
            "Handoff initiated"
        );

        Ok(handoff_id)
    }

    /// Accept a handoff
    pub async fn accept_handoff(&self, handoff_id: &str) -> Result<(), String> {
        let mut records = self.records.write().await;
        let record = records
            .get_mut(handoff_id)
            .ok_or_else(|| format!("Handoff {} not found", handoff_id))?;

        if record.status != HandoffStatus::Pending {
            return Err(format!(
                "Handoff {} is not in Pending status (current: {:?})",
                handoff_id, record.status
            ));
        }

        record.status = HandoffStatus::Accepted;
        record.completed_at = Some(Utc::now());

        tracing::info!(handoff_id = %handoff_id, "Handoff accepted");
        Ok(())
    }

    /// Reject a handoff
    pub async fn reject_handoff(&self, handoff_id: &str, reason: &str) -> Result<(), String> {
        let mut records = self.records.write().await;
        let record = records
            .get_mut(handoff_id)
            .ok_or_else(|| format!("Handoff {} not found", handoff_id))?;

        record.status = HandoffStatus::Rejected;
        record.completed_at = Some(Utc::now());

        tracing::warn!(
            handoff_id = %handoff_id,
            reason = %reason,
            "Handoff rejected"
        );
        Ok(())
    }

    /// Complete a handoff
    pub async fn complete_handoff(&self, handoff_id: &str) -> Result<(), String> {
        let mut records = self.records.write().await;
        let record = records
            .get_mut(handoff_id)
            .ok_or_else(|| format!("Handoff {} not found", handoff_id))?;

        record.status = HandoffStatus::Completed;
        record.completed_at = Some(Utc::now());

        tracing::info!(handoff_id = %handoff_id, "Handoff completed");
        Ok(())
    }

    /// Get a handoff record
    pub async fn get_handoff(&self, handoff_id: &str) -> Option<HandoffRecord> {
        let records = self.records.read().await;
        records.get(handoff_id).cloned()
    }

    /// Get all handoffs for a task
    pub async fn get_task_handoffs(&self, task_id: &str) -> Vec<HandoffRecord> {
        let history = self.task_history.read().await;
        let records = self.records.read().await;

        history
            .get(task_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| records.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Select the best candidate for handoff
    pub fn select_candidate<'a>(
        &self,
        candidates: &'a [HandoffCandidate],
        required_skills: &[String],
    ) -> Option<&'a HandoffCandidate> {
        candidates
            .iter()
            .filter(|c| c.health == AgentHealth::Healthy)
            .filter(|c| c.current_load < self.policy.overload_threshold)
            .filter(|c| {
                let match_score = self.calculate_skill_match(&c.available_skills, required_skills);
                match_score >= self.policy.min_skill_match_score
            })
            .min_by(move |a, b| {
                // Prefer lower load, then higher skill match
                let load_cmp = a
                    .current_load
                    .partial_cmp(&b.current_load)
                    .unwrap_or(std::cmp::Ordering::Equal);
                if load_cmp != std::cmp::Ordering::Equal {
                    return load_cmp;
                }
                b.skill_match_score
                    .partial_cmp(&a.skill_match_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Calculate skill match score between agent skills and required skills
    fn calculate_skill_match(
        &self,
        agent_skills: &[AgentSkill],
        required_skills: &[String],
    ) -> f64 {
        if required_skills.is_empty() {
            return 1.0;
        }

        let matched = required_skills
            .iter()
            .filter(|req| {
                agent_skills.iter().any(|s| {
                    s.id == **req
                        || s.tags.iter().any(|t| t == *req)
                        || s.name.to_lowercase().contains(&req.to_lowercase())
                })
            })
            .count();

        matched as f64 / required_skills.len() as f64
    }

    /// Get handoff statistics
    pub async fn stats(&self) -> HandoffStats {
        let records = self.records.read().await;
        let total = records.len();
        let pending = records
            .values()
            .filter(|r| r.status == HandoffStatus::Pending)
            .count();
        let completed = records
            .values()
            .filter(|r| r.status == HandoffStatus::Completed)
            .count();
        let failed = records
            .values()
            .filter(|r| r.status == HandoffStatus::Failed || r.status == HandoffStatus::TimedOut)
            .count();

        HandoffStats {
            total_handoffs: total,
            pending,
            completed,
            failed,
        }
    }
}

/// Handoff statistics
#[derive(Debug, Clone, Default)]
pub struct HandoffStats {
    pub total_handoffs: usize,
    pub pending: usize,
    pub completed: usize,
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::a2a::AgentSkill;

    fn make_manager() -> HandoffManager {
        HandoffManager::new(HandoffPolicy::default())
    }

    fn make_candidate(id: &str, load: f64, skills: Vec<&str>) -> HandoffCandidate {
        HandoffCandidate {
            agent_id: id.to_string(),
            skill_match_score: 0.0,
            current_load: load,
            health: AgentHealth::Healthy,
            available_skills: skills
                .into_iter()
                .map(|s| AgentSkill {
                    id: s.to_string(),
                    name: s.to_string(),
                    description: String::new(),
                    examples: vec![],
                    tags: vec![s.to_string()],
                    location_bound: false,
                })
                .collect(),
        }
    }

    #[tokio::test]
    async fn test_handoff_lifecycle() {
        let mgr = make_manager();

        let id = mgr
            .initiate_handoff(
                "task-1",
                "agent-a",
                "agent-b",
                HandoffReason::Specialization,
                serde_json::json!({"progress": 50}),
                vec!["code-review".to_string()],
            )
            .await
            .unwrap();

        let record = mgr.get_handoff(&id).await.unwrap();
        assert_eq!(record.status, HandoffStatus::Pending);

        mgr.accept_handoff(&id).await.unwrap();
        let record = mgr.get_handoff(&id).await.unwrap();
        assert_eq!(record.status, HandoffStatus::Accepted);

        mgr.complete_handoff(&id).await.unwrap();
        let record = mgr.get_handoff(&id).await.unwrap();
        assert_eq!(record.status, HandoffStatus::Completed);
    }

    #[tokio::test]
    async fn test_handoff_rejection() {
        let mgr = make_manager();

        let id = mgr
            .initiate_handoff(
                "task-1",
                "agent-a",
                "agent-b",
                HandoffReason::LoadBalancing,
                serde_json::json!(null),
                vec![],
            )
            .await
            .unwrap();

        mgr.reject_handoff(&id, "too busy").await.unwrap();
        let record = mgr.get_handoff(&id).await.unwrap();
        assert_eq!(record.status, HandoffStatus::Rejected);
    }

    #[tokio::test]
    async fn test_max_handoff_chain() {
        let policy = HandoffPolicy {
            max_handoffs: 2,
            ..Default::default()
        };
        let mgr = HandoffManager::new(policy);

        // First handoff OK
        let _ = mgr
            .initiate_handoff(
                "task-1",
                "a",
                "b",
                HandoffReason::Specialization,
                serde_json::json!(null),
                vec![],
            )
            .await
            .unwrap();

        // Second handoff OK
        let _ = mgr
            .initiate_handoff(
                "task-1",
                "b",
                "c",
                HandoffReason::Specialization,
                serde_json::json!(null),
                vec![],
            )
            .await
            .unwrap();

        // Third should fail (exceeds max)
        let result = mgr
            .initiate_handoff(
                "task-1",
                "c",
                "d",
                HandoffReason::Specialization,
                serde_json::json!(null),
                vec![],
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_task_handoffs() {
        let mgr = make_manager();

        let _ = mgr
            .initiate_handoff(
                "task-1",
                "a",
                "b",
                HandoffReason::LoadBalancing,
                serde_json::json!(null),
                vec![],
            )
            .await
            .unwrap();

        let handoffs = mgr.get_task_handoffs("task-1").await;
        assert_eq!(handoffs.len(), 1);

        let empty = mgr.get_task_handoffs("task-999").await;
        assert!(empty.is_empty());
    }

    #[test]
    fn test_candidate_selection() {
        let mgr = make_manager();
        let candidates = vec![
            make_candidate("fast", 0.2, vec!["code-review"]),
            make_candidate("slow", 0.9, vec!["code-review"]),
            make_candidate("expert", 0.5, vec!["code-review", "security"]),
        ];

        let selected = mgr.select_candidate(&candidates, &["code-review".to_string()]);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().agent_id, "fast");
    }

    #[test]
    fn test_skill_match_score() {
        let mgr = make_manager();
        let skills = vec![
            AgentSkill {
                id: "review".to_string(),
                name: "Code Review".to_string(),
                description: String::new(),
                examples: vec![],
                tags: vec!["coding".to_string()],
                location_bound: false,
            },
            AgentSkill {
                id: "security".to_string(),
                name: "Security Audit".to_string(),
                description: String::new(),
                examples: vec![],
                tags: vec!["security".to_string()],
                location_bound: false,
            },
        ];

        let score = mgr.calculate_skill_match(&skills, &["review".to_string()]);
        assert!((score - 1.0).abs() < f64::EPSILON);

        let score =
            mgr.calculate_skill_match(&skills, &["review".to_string(), "deploy".to_string()]);
        assert!((score - 0.5).abs() < f64::EPSILON);

        let score = mgr.calculate_skill_match(&skills, &[]);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_handoff_stats() {
        let mgr = make_manager();

        let _ = mgr
            .initiate_handoff(
                "t1",
                "a",
                "b",
                HandoffReason::LoadBalancing,
                serde_json::json!(null),
                vec![],
            )
            .await
            .unwrap();

        let stats = mgr.stats().await;
        assert_eq!(stats.total_handoffs, 1);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.completed, 0);
    }

    #[test]
    fn test_handoff_policy_default() {
        let policy = HandoffPolicy::default();
        assert_eq!(policy.max_handoffs, 3);
        assert!(policy.auto_handoff_on_capability_gap);
        assert!((policy.overload_threshold - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_unhealthy_candidate_filtered() {
        let mgr = make_manager();
        let mut candidate = make_candidate("sick", 0.1, vec!["code-review"]);
        candidate.health = AgentHealth::Unhealthy;

        let candidates = [candidate];
        let selected = mgr.select_candidate(&candidates, &["code-review".to_string()]);
        assert!(selected.is_none());
    }
}
