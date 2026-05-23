use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_equality() {
        assert_eq!(SessionStatus::Active, SessionStatus::Active);
        assert_eq!(SessionStatus::Completed, SessionStatus::Completed);
        assert_ne!(SessionStatus::Active, SessionStatus::Paused);
        assert_ne!(SessionStatus::Failed, SessionStatus::Completed);
    }

    #[test]
    fn session_status_debug() {
        assert_eq!(format!("{:?}", SessionStatus::Active), "Active");
        assert_eq!(format!("{:?}", SessionStatus::Failed), "Failed");
    }

    #[test]
    fn session_status_serde_roundtrip() {
        for status in [
            SessionStatus::Active,
            SessionStatus::Paused,
            SessionStatus::Completed,
            SessionStatus::Failed,
        ] {
            let encoded = serde_json::to_string(&status).unwrap();
            let decoded: SessionStatus = serde_json::from_str(&encoded).unwrap();
            assert_eq!(status, decoded);
        }
    }

    #[test]
    fn session_new_creates_active_session() {
        let before = Utc::now();
        let session = Session::new("s1", "a1");
        let after = Utc::now();

        assert_eq!(session.id, "s1");
        assert_eq!(session.agent_id, "a1");
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.metadata.is_object());
        assert!(session.metadata.as_object().unwrap().is_empty());
        assert!(session.completed_at.is_none());

        // timestamps within 2 seconds
        assert!(session.started_at - before < Duration::from_secs(2));
        assert!(after - session.started_at < Duration::from_secs(2));
        assert!(session.updated_at - before < Duration::from_secs(2));
        assert!(after - session.updated_at < Duration::from_secs(2));
    }

    #[test]
    fn session_update_status_to_completed_sets_completed_at() {
        let mut session = Session::new("s1", "a1");
        let before = Utc::now();

        session.update_status(SessionStatus::Completed);

        let after = Utc::now();
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(session.completed_at.is_some());
        let completed_at = session.completed_at.unwrap();
        assert!(completed_at - before < Duration::from_secs(1));
        assert!(after - completed_at < Duration::from_secs(1));
        assert!(session.updated_at >= session.started_at);
    }

    #[test]
    fn session_update_status_does_not_set_completed_at_for_non_completed() {
        let mut session = Session::new("s1", "a1");
        session.update_status(SessionStatus::Paused);
        assert_eq!(session.status, SessionStatus::Paused);
        assert!(session.completed_at.is_none());

        session.update_status(SessionStatus::Failed);
        assert_eq!(session.status, SessionStatus::Failed);
        assert!(session.completed_at.is_none());
    }

    #[test]
    fn session_update_status_updates_timestamp() {
        let mut session = Session::new("s1", "a1");
        let original_updated_at = session.updated_at;

        std::thread::sleep(Duration::from_millis(10));
        session.update_status(SessionStatus::Paused);

        assert!(session.updated_at > original_updated_at);
    }

    #[test]
    fn session_serde_roundtrip() {
        let mut session = Session::new("s1", "a1");
        session.update_status(SessionStatus::Completed);

        let encoded = serde_json::to_string(&session).unwrap();
        let decoded: Session = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded.id, session.id);
        assert_eq!(decoded.agent_id, session.agent_id);
        assert_eq!(decoded.status, session.status);
        assert_eq!(decoded.completed_at, session.completed_at);
    }
}

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
