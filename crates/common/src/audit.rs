//! # Audit Logging
//!
//! Structured audit trail for security-relevant events in the KIAS system.
//!
//! Provides:
//! - [`AuditEvent`] – the event record
//! - [`AuditAction`] – discriminated action enum
//! - [`AuditLogger`] – async trait for pluggable backends
//! - [`MemoryAuditLog`] – in-memory ring-buffer implementation for testing
//! - [`audit_filter`] – query helper

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fmt;
use tokio::sync::RwLock;
use uuid::Uuid;

// ── AuditAction ───────────────────────────────────────────────────────

/// The type of action that was performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditAction {
    Create,
    Read,
    Update,
    Delete,
    Login,
    Logout,
    Schedule,
    Execute,
    ConfigChange,
}

impl fmt::Display for AuditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create => write!(f, "Create"),
            Self::Read => write!(f, "Read"),
            Self::Update => write!(f, "Update"),
            Self::Delete => write!(f, "Delete"),
            Self::Login => write!(f, "Login"),
            Self::Logout => write!(f, "Logout"),
            Self::Schedule => write!(f, "Schedule"),
            Self::Execute => write!(f, "Execute"),
            Self::ConfigChange => write!(f, "ConfigChange"),
        }
    }
}

// ── AuditOutcome ──────────────────────────────────────────────────────

/// Whether the audited action succeeded or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuditOutcome {
    Success,
    Failure,
}

impl fmt::Display for AuditOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Failure => write!(f, "Failure"),
        }
    }
}

// ── AuditEvent ────────────────────────────────────────────────────────

/// A single audit trail entry.
#[derive(Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// Unique event identifier.
    pub id: String,
    /// When the event occurred (UTC).
    pub timestamp: DateTime<Utc>,
    /// Who performed the action (user id, service account, etc.).
    pub actor: String,
    /// What was done.
    pub action: AuditAction,
    /// The kind of resource affected (e.g. "agent", "node", "config").
    pub resource_type: String,
    /// The identifier of the specific resource affected.
    pub resource_id: String,
    /// Free-form human-readable details.
    pub details: String,
    /// The client IP address, if known.
    pub ip_address: Option<String>,
    /// The client User-Agent string, if known.
    pub user_agent: Option<String>,
    /// Whether the action succeeded or failed.
    pub outcome: AuditOutcome,
}

/// Convert a potentially identifying subject into a stable pseudonym.
///
/// Deployments that require cross-system correlation should replace this with
/// an HMAC keyed by a deployment-specific secret. Raw identities must not be
/// written to application logs.
pub fn pseudonymize_identifier(identifier: &str) -> String {
    match identifier {
        "system" | "unknown" | "api-key-user" => identifier.to_string(),
        _ => {
            let mut hasher = Sha256::new();
            hasher.update(identifier.as_bytes());
            let digest = format!("{:x}", hasher.finalize());
            format!("subject:{}", &digest[..16])
        }
    }
}

impl fmt::Debug for AuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditEvent")
            .field("id", &self.id)
            .field("timestamp", &self.timestamp)
            .field("actor", &"[PSEUDONYMOUS]")
            .field("action", &self.action)
            .field("resource_type", &self.resource_type)
            .field("resource_id", &"[REDACTED]")
            .field("details", &"[REDACTED]")
            .field(
                "ip_address",
                &self.ip_address.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "user_agent",
                &self.user_agent.as_ref().map(|_| "[REDACTED]"),
            )
            .field("outcome", &self.outcome)
            .finish()
    }
}

impl AuditEvent {
    /// Create a new event with a generated id and the current timestamp.
    pub fn new(
        actor: impl Into<String>,
        action: AuditAction,
        resource_type: impl Into<String>,
        resource_id: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            actor: actor.into(),
            action,
            resource_type: resource_type.into(),
            resource_id: resource_id.into(),
            details: String::new(),
            ip_address: None,
            user_agent: None,
            outcome,
        }
    }

    /// Builder-style setter for `details`.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = details.into();
        self
    }

    /// Builder-style setter for `ip_address`.
    pub fn with_ip(mut self, ip: impl Into<String>) -> Self {
        self.ip_address = Some(ip.into());
        self
    }

    /// Builder-style setter for `user_agent`.
    pub fn with_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.user_agent = Some(ua.into());
        self
    }
}

// ── AuditLogger trait ─────────────────────────────────────────────────

/// Async trait for pluggable audit log backends.
#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    /// Persist (or buffer) an audit event.
    async fn log_event(&self, event: AuditEvent);
}

// ── MemoryAuditLog ────────────────────────────────────────────────────

/// In-memory ring-buffer audit log (max 10 000 entries).  Useful for tests
/// and development.
pub struct MemoryAuditLog {
    events: RwLock<VecDeque<AuditEvent>>,
    capacity: usize,
}

impl MemoryAuditLog {
    /// Create a new log with the default capacity of 10 000.
    pub fn new() -> Self {
        Self {
            events: RwLock::new(VecDeque::new()),
            capacity: 10_000,
        }
    }

    /// Create a new log with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: RwLock::new(VecDeque::new()),
            capacity,
        }
    }

    /// Return a snapshot of all stored events (oldest first).
    pub async fn all_events(&self) -> Vec<AuditEvent> {
        self.events.read().await.iter().cloned().collect()
    }

    /// Return the number of stored events.
    pub async fn count(&self) -> usize {
        self.events.read().await.len()
    }
}

impl Default for MemoryAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AuditLogger for MemoryAuditLog {
    async fn log_event(&self, event: AuditEvent) {
        let mut events = self.events.write().await;
        if events.len() >= self.capacity {
            events.pop_front();
        }
        events.push_back(event);
    }
}

// ── audit_filter helper ───────────────────────────────────────────────

/// Query helper: filter audit events by optional actor, action, resource type,
/// and time range.
pub async fn audit_filter(
    logger: &MemoryAuditLog,
    actor: Option<&str>,
    action: Option<AuditAction>,
    resource_type: Option<&str>,
    time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
) -> Vec<AuditEvent> {
    let events = logger.all_events().await;
    events
        .into_iter()
        .filter(|e| {
            if let Some(a) = actor {
                if e.actor != a {
                    return false;
                }
            }
            if let Some(act) = action {
                if e.action != act {
                    return false;
                }
            }
            if let Some(rt) = resource_type {
                if e.resource_type != rt {
                    return false;
                }
            }
            if let Some((start, end)) = time_range {
                if e.timestamp < start || e.timestamp > end {
                    return false;
                }
            }
            true
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_audit_log_store_and_retrieve() {
        let log = MemoryAuditLog::new();
        let event = AuditEvent::new(
            "user1",
            AuditAction::Create,
            "agent",
            "agent-1",
            AuditOutcome::Success,
        );
        log.log_event(event.clone()).await;
        let events = log.all_events().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].actor, "user1");
    }

    #[tokio::test]
    async fn test_memory_audit_log_capacity_eviction() {
        let log = MemoryAuditLog::with_capacity(3);
        for i in 0..5 {
            let event = AuditEvent::new(
                format!("user{i}"),
                AuditAction::Create,
                "agent",
                format!("agent-{i}"),
                AuditOutcome::Success,
            );
            log.log_event(event).await;
        }
        let events = log.all_events().await;
        assert_eq!(events.len(), 3);
        // Oldest two should have been evicted
        assert_eq!(events[0].actor, "user2");
    }

    #[tokio::test]
    async fn test_audit_filter_by_actor() {
        let log = MemoryAuditLog::new();
        log.log_event(AuditEvent::new(
            "alice",
            AuditAction::Read,
            "agent",
            "a1",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "bob",
            AuditAction::Read,
            "agent",
            "a2",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "alice",
            AuditAction::Delete,
            "agent",
            "a3",
            AuditOutcome::Success,
        ))
        .await;

        let filtered = audit_filter(&log, Some("alice"), None, None, None).await;
        assert_eq!(filtered.len(), 2);
    }

    #[tokio::test]
    async fn test_audit_filter_by_action() {
        let log = MemoryAuditLog::new();
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Login,
            "session",
            "s1",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Logout,
            "session",
            "s1",
            AuditOutcome::Success,
        ))
        .await;

        let filtered = audit_filter(&log, None, Some(AuditAction::Login), None, None).await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].action, AuditAction::Login);
    }

    #[tokio::test]
    async fn test_audit_filter_by_resource_type() {
        let log = MemoryAuditLog::new();
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Create,
            "agent",
            "a1",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Create,
            "node",
            "n1",
            AuditOutcome::Success,
        ))
        .await;

        let filtered = audit_filter(&log, None, None, Some("node"), None).await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].resource_type, "node");
    }

    #[tokio::test]
    async fn test_audit_filter_by_time_range() {
        let log = MemoryAuditLog::new();
        let old_event = AuditEvent {
            id: "1".into(),
            timestamp: Utc::now() - chrono::Duration::hours(2),
            actor: "u".into(),
            action: AuditAction::Read,
            resource_type: "agent".into(),
            resource_id: "a1".into(),
            details: String::new(),
            ip_address: None,
            user_agent: None,
            outcome: AuditOutcome::Success,
        };
        log.log_event(old_event).await;
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Read,
            "agent",
            "a2",
            AuditOutcome::Success,
        ))
        .await;

        let since = Utc::now() - chrono::Duration::minutes(5);
        let filtered = audit_filter(&log, None, None, None, Some((since, Utc::now()))).await;
        assert_eq!(filtered.len(), 1);
    }

    #[tokio::test]
    async fn test_audit_filter_combined() {
        let log = MemoryAuditLog::new();
        log.log_event(AuditEvent::new(
            "alice",
            AuditAction::Update,
            "config",
            "c1",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "alice",
            AuditAction::Update,
            "agent",
            "a1",
            AuditOutcome::Success,
        ))
        .await;
        log.log_event(AuditEvent::new(
            "bob",
            AuditAction::Update,
            "config",
            "c2",
            AuditOutcome::Success,
        ))
        .await;

        let filtered = audit_filter(
            &log,
            Some("alice"),
            Some(AuditAction::Update),
            Some("config"),
            None,
        )
        .await;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].resource_id, "c1");
    }

    #[tokio::test]
    async fn test_audit_event_builder() {
        let event = AuditEvent::new(
            "admin",
            AuditAction::ConfigChange,
            "system",
            "cfg",
            AuditOutcome::Success,
        )
        .with_details("changed log level")
        .with_ip("10.0.0.1")
        .with_user_agent("curl/7.0");

        assert_eq!(event.details, "changed log level");
        assert_eq!(event.ip_address.as_deref(), Some("10.0.0.1"));
        assert_eq!(event.user_agent.as_deref(), Some("curl/7.0"));
    }

    #[tokio::test]
    async fn test_audit_action_display() {
        assert_eq!(AuditAction::Create.to_string(), "Create");
        assert_eq!(AuditAction::Login.to_string(), "Login");
        assert_eq!(AuditAction::ConfigChange.to_string(), "ConfigChange");
    }

    #[tokio::test]
    async fn test_audit_outcome_display() {
        assert_eq!(AuditOutcome::Success.to_string(), "Success");
        assert_eq!(AuditOutcome::Failure.to_string(), "Failure");
    }

    #[tokio::test]
    async fn test_memory_audit_log_default_capacity() {
        let log = MemoryAuditLog::new();
        assert_eq!(log.capacity, 10_000);
    }

    #[tokio::test]
    async fn test_memory_audit_log_count() {
        let log = MemoryAuditLog::new();
        assert_eq!(log.count().await, 0);
        log.log_event(AuditEvent::new(
            "u",
            AuditAction::Read,
            "r",
            "id",
            AuditOutcome::Success,
        ))
        .await;
        assert_eq!(log.count().await, 1);
    }

    #[tokio::test]
    async fn test_audit_filter_no_results() {
        let log = MemoryAuditLog::new();
        log.log_event(AuditEvent::new(
            "alice",
            AuditAction::Read,
            "agent",
            "a1",
            AuditOutcome::Success,
        ))
        .await;
        let filtered = audit_filter(&log, Some("bob"), None, None, None).await;
        assert!(filtered.is_empty());
    }
}
