use super::session::{Session, SessionStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 代理视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentView {
    pub agent_id: String,
    pub sessions: Vec<Session>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AgentView {
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            sessions: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn add_session(&mut self, session: Session) {
        self.sessions.push(session);
        self.updated_at = Utc::now();
    }

    pub fn get_active_sessions(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Active)
            .collect()
    }

    pub fn get_completed_sessions(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Completed)
            .collect()
    }

    pub fn display_summary(&self) {
        tracing::info!(agent_id = %self.agent_id, "Agent view summary");
        tracing::info!(total = self.sessions.len(), "Total sessions");
        tracing::info!(active = self.get_active_sessions().len(), "Active sessions");
        tracing::info!(
            completed = self.get_completed_sessions().len(),
            "Completed sessions"
        );
        tracing::info!("Recent sessions:");
        for session in self.sessions.iter().take(5) {
            tracing::info!(session_id = %session.id, status = ?session.status, "Session");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{Session, SessionStatus};

    #[test]
    fn test_agent_view_creation() {
        let view = AgentView::new("agent-1");
        assert_eq!(view.agent_id, "agent-1");
        assert!(view.sessions.is_empty());
    }

    #[test]
    fn test_add_session() {
        let mut view = AgentView::new("agent-1");
        let session = Session::new("sess-1", "agent-1");
        view.add_session(session);
        assert_eq!(view.sessions.len(), 1);
    }

    #[test]
    fn test_active_sessions() {
        let mut view = AgentView::new("agent-1");
        view.add_session(Session::new("s1", "agent-1"));
        view.add_session(Session::new("s2", "agent-1"));

        // All should be active initially
        assert_eq!(view.get_active_sessions().len(), 2);
        assert_eq!(view.get_completed_sessions().len(), 0);
    }

    #[test]
    fn test_completed_sessions() {
        let mut view = AgentView::new("agent-1");
        let mut session = Session::new("s1", "agent-1");
        session.update_status(SessionStatus::Completed);
        view.add_session(session);
        view.add_session(Session::new("s2", "agent-1"));

        assert_eq!(view.get_active_sessions().len(), 1);
        assert_eq!(view.get_completed_sessions().len(), 1);
    }

    #[test]
    fn test_session_creation() {
        let session = Session::new("sess-1", "agent-1");
        assert_eq!(session.id, "sess-1");
        assert_eq!(session.agent_id, "agent-1");
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.completed_at.is_none());
    }

    #[test]
    fn test_session_status_transitions() {
        let mut session = Session::new("s1", "a1");
        assert_eq!(session.status, SessionStatus::Active);

        session.update_status(SessionStatus::Paused);
        assert_eq!(session.status, SessionStatus::Paused);
        assert!(session.completed_at.is_none());

        session.update_status(SessionStatus::Completed);
        assert_eq!(session.status, SessionStatus::Completed);
        assert!(session.completed_at.is_some());
    }

    #[test]
    fn test_session_status_variants() {
        assert_ne!(SessionStatus::Active, SessionStatus::Completed);
        assert_ne!(SessionStatus::Paused, SessionStatus::Failed);
    }
}
