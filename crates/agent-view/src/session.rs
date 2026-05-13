use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Paused,
    Completed,
    Failed,
}

/// 会话信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub status: SessionStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
}

impl Session {
    pub fn new(id: &str, agent_id: &str) -> Self {
        Self {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            status: SessionStatus::Active,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            metadata: serde_json::json!({}),
        }
    }

    pub fn update_status(&mut self, status: SessionStatus) {
        if status == SessionStatus::Completed {
            self.completed_at = Some(Utc::now());
        }
        self.status = status;
        self.updated_at = Utc::now();
    }
}
