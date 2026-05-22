use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    /// Returns true if the session is in a terminal state (Completed or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, SessionStatus::Completed | SessionStatus::Failed)
    }

    /// Returns the session duration in milliseconds, or None if still active.
    pub fn duration_ms(&self) -> Option<i64> {
        let end = self.completed_at.unwrap_or_else(Utc::now);
        Some((end - self.started_at).num_milliseconds())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new_defaults() {
        let s = Session::new("s1", "a1");
        assert_eq!(s.id, "s1");
        assert_eq!(s.agent_id, "a1");
        assert_eq!(s.status, SessionStatus::Active);
        assert!(s.completed_at.is_none());
        assert_eq!(s.metadata, serde_json::json!({}));
    }

    #[test]
    fn test_session_update_status_non_terminal() {
        let mut s = Session::new("s1", "a1");
        let before = s.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.update_status(SessionStatus::Paused);
        assert_eq!(s.status, SessionStatus::Paused);
        assert!(s.completed_at.is_none());
        assert!(s.updated_at > before);
    }

    #[test]
    fn test_session_update_status_completed_sets_timestamp() {
        let mut s = Session::new("s1", "a1");
        assert!(s.completed_at.is_none());
        s.update_status(SessionStatus::Completed);
        assert_eq!(s.status, SessionStatus::Completed);
        assert!(s.completed_at.is_some());
    }

    #[test]
    fn test_session_update_status_failed_no_completed_at() {
        let mut s = Session::new("s1", "a1");
        s.update_status(SessionStatus::Failed);
        assert_eq!(s.status, SessionStatus::Failed);
        assert!(s.completed_at.is_none());
    }

    #[test]
    fn test_session_is_terminal() {
        let mut s = Session::new("s1", "a1");
        assert!(!s.is_terminal());

        s.update_status(SessionStatus::Paused);
        assert!(!s.is_terminal());

        s.update_status(SessionStatus::Completed);
        assert!(s.is_terminal());

        let mut s2 = Session::new("s2", "a1");
        s2.update_status(SessionStatus::Failed);
        assert!(s2.is_terminal());
    }

    #[test]
    fn test_session_duration_ms() {
        let mut s = Session::new("s1", "a1");
        // Active session returns Some (duration up to now)
        let d = s.duration_ms();
        assert!(d.is_some());
        assert!(d.unwrap() >= 0);

        s.update_status(SessionStatus::Completed);
        let d = s.duration_ms();
        assert!(d.is_some());
        assert!(d.unwrap() >= 0);
    }

    #[test]
    fn test_session_status_serde_roundtrip() {
        let statuses = vec![
            SessionStatus::Active,
            SessionStatus::Paused,
            SessionStatus::Completed,
            SessionStatus::Failed,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: SessionStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_session_serde_roundtrip() {
        let mut s = Session::new("s1", "a1");
        s.update_status(SessionStatus::Completed);
        s.metadata = serde_json::json!({"key": "value"});

        let json = serde_json::to_string(&s).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "s1");
        assert_eq!(deserialized.agent_id, "a1");
        assert_eq!(deserialized.status, SessionStatus::Completed);
        assert!(deserialized.completed_at.is_some());
        assert_eq!(deserialized.metadata, serde_json::json!({"key": "value"}));
    }
}
