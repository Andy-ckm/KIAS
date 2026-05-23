use crate::error::RegistryError;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The result of capability matching.
#[derive(Debug, Clone)]
pub struct CapabilityMatchResult {
    /// Overall match score (0.0 to 1.0).
    pub score: f64,
    /// Capabilities present in both parties.
    pub matched: Vec<String>,
    /// Capabilities only in party B.
    pub missing_in_a: Vec<String>,
    /// Capabilities only in party A.
    pub missing_in_b: Vec<String>,
}

/// A single capability specification.
#[derive(Debug, Clone)]
pub struct Capability {
    /// Unique name of the capability.
    pub name: String,
    /// Version of the capability.
    pub version: String,
    /// Weight used in scoring.
    pub weight: f64,
}

/// Set of capabilities with a unique identifier.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    /// Map from capability name to Capability.
    pub capabilities: HashMap<String, Capability>,
}

/// Protocol version with semantic versioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProtocolVersion {
    /// Major version number.
    pub major: u16,
    /// Minor version number.
    pub minor: u16,
    /// Patch version number.
    pub patch: u16,
}

impl ProtocolVersion {
    /// Create a new ProtocolVersion.
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        ProtocolVersion { major, minor, patch }
    }

    /// Returns true if this version is compatible with another (same major).
    pub fn is_compatible(&self, other: &ProtocolVersion) -> bool {
        self.major == other.major
    }
}

/// Unique identifier for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Represents an active session between a client and a server.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// Identifier of the client part of the session.
    pub client_id: String,
    /// Identifier of the server part of the session.
    pub server_id: String,
    /// Capabilities negotiated for this session.
    pub capabilities: CapabilitySet,
    /// Protocol version used by this session.
    pub protocol_version: ProtocolVersion,
    /// Timestamp when the session was created.
    pub created_at: Instant,
    /// Timestamp of the last activity.
    pub last_activity: Instant,
    /// Whether the session is active.
    pub active: bool,
}

/// A message to be routed.
#[derive(Debug, Clone)]
pub struct Message {
    /// Unique message identifier.
    pub id: u64,
    /// Originator of the message.
    pub from: String,
    /// Destination of the message.
    pub to: String,
    /// Message payload.
    pub payload: Vec<u8>,
    /// Message type identifier.
    pub message_type: String,
}

/// Result of routing a message.
#[derive(Debug, Clone)]
pub struct RoutingResult {
    /// The session the message was routed to.
    pub session: SessionId,
    /// Whether routing succeeded.
    pub success: bool,
    /// Optional error message.
    pub error: Option<String>,
}

/// Manager for session lifecycle.
pub struct SessionManager {
    sessions: Arc<Mutex<HashMap<SessionId, Session>>>,
    session_timeout: Duration,
}

impl SessionManager {
    /// Create a new SessionManager with the given session timeout.
    pub fn new(session_timeout: Duration) -> Self {
        SessionManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            session_timeout,
        }
    }

    /// Creates a new session and returns its ID.
    pub fn create_session(
        &self,
        client_id: String,
        server_id: String,
        capabilities: CapabilitySet,
        protocol_version: ProtocolVersion,
    ) -> Result<SessionId, RegistryError> {
        let id = {
            let mut sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
            let now = Instant::now();
            // Generate a new unique ID
            let id = SessionId(sessions.len() as u64 + 1);
            let session = Session {
                id,
                client_id,
                server_id,
                capabilities,
                protocol_version,
                created_at: now,
                last_activity: now,
                active: true,
            };
            sessions.insert(id, session);
            id
        };
        tracing::info!("Session created with ID: {:?}", id);
        Ok(id)
    }

    /// Terminates a session.
    pub fn terminate_session(&self, id: SessionId) -> Result<(), RegistryError> {
        let mut sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
        if let Some(session) = sessions.get_mut(&id) {
            session.active = false;
            tracing::info!("Session {:?} terminated", id);
            Ok(())
        } else {
            tracing::warn!("Attempted to terminate unknown session: {:?}", id);
            Err(RegistryError::NotFound { agent_id: "session not found".to_string() })
        }
    }

    /// Returns a clone of the session if it exists and is active.
    pub fn get_session(&self, id: SessionId) -> Result<Session, RegistryError> {
        let sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
        sessions.get(&id).cloned().ok_or(RegistryError::NotFound { agent_id: format!("{:?}", id) })
    }

    /// Get session by client or server ID.
    pub fn get_session_by_client_or_server(&self, id: &str) -> Result<Session, RegistryError> {
        let sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
        sessions
            .values()
            .find(|s| s.client_id == id || s.server_id == id)
            .cloned()
            .ok_or(RegistryError::NotFound { agent_id: id.to_string() })
    }

    /// Refreshes the last activity timestamp of a session.
    pub fn touch_session(&self, id: SessionId) -> Result<(), RegistryError> {
        let mut sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
        if let Some(session) = sessions.get_mut(&id) {
            session.last_activity = Instant::now();
            Ok(())
        } else {
            Err(RegistryError::NotFound { agent_id: "session not found".to_string() })
        }
    }

    /// Removes all sessions that have been idle longer than the configured timeout.
    pub fn cleanup_expired(&self) -> Result<usize, RegistryError> {
        let mut sessions = self.sessions.lock().map_err(|_| RegistryError::NotFound { agent_id: "lock error".to_string() })?;
        let now = Instant::now();
        let mut expired = 0;
        sessions.retain(|_, session| {
            let keep = !session.active || now.duration_since(session.last_activity) < self.session_timeout;
            if !keep {
                expired += 1;
                tracing::info!("Session {:?} expired and removed", session.id);
            }
            keep
        });
        Ok(expired)
    }
}

/// Matcher for capability sets.
pub struct CapabilityMatcher;

impl CapabilityMatcher {
    /// Matches two capability sets and returns a detailed result.
    pub fn match_capabilities(
        &self,
        a: &CapabilitySet,
        b: &CapabilitySet,
    ) -> Result<CapabilityMatchResult, RegistryError> {
        let mut matched = Vec::new();
        let mut missing_in_a = Vec::new();
        let mut missing_in_b = Vec::new();
        let mut total_weight = 0.0;
        let mut matched_weight = 0.0;

        // Iterate over all capability names in set A
        for (name, cap_a) in &a.capabilities {
            total_weight += cap_a.weight;
            if let Some(cap_b) = b.capabilities.get(name) {
                // Capability present in both; check version compatibility (simple exact match)
                if cap_a.version == cap_b.version {
                    matched.push(name.clone());
                    matched_weight += cap_a.weight;
                } else {
                    // version mismatch, treat as missing
                    missing_in_a.push(name.clone());
                }
            } else {
                missing_in_a.push(name.clone());
            }
        }

        // Identify capabilities in B that are not in A
        for (name, cap_b) in &b.capabilities {
            if !a.capabilities.contains_key(name) {
                missing_in_b.push(name.clone());
                total_weight += cap_b.weight;
            }
        }

        let score = if total_weight > 0.0 {
            matched_weight / total_weight
        } else {
            0.0
        };

        tracing::info!(
            "Capability match score: {:.2} between sets (matched: {:?})",
            score,
            matched
        );

        Ok(CapabilityMatchResult {
            score,
            matched,
            missing_in_a,
            missing_in_b,
        })
    }
}

/// Negotiator for protocol versions.
pub struct ProtocolNegotiator;

impl ProtocolNegotiator {
    /// Negotiates the highest common protocol version between two lists.
    pub fn negotiate(
        client_versions: &[ProtocolVersion],
        server_versions: &[ProtocolVersion],
    ) -> Result<ProtocolVersion, RegistryError> {
        // Ensure both lists are non‑empty
        if client_versions.is_empty() {
            return Err(RegistryError::NotFound { agent_id: "client version list is empty".to_string() });
        }
        if server_versions.is_empty() {
            return Err(RegistryError::NotFound { agent_id: "server version list is empty".to_string() });
        }

        // Find the highest common version that is compatible
        let mut best: Option<ProtocolVersion> = None;
        for cv in client_versions {
            for sv in server_versions {
                if cv.is_compatible(sv) {
                    let candidate = *cv.max(sv);
                    if let Some(cur) = best {
                        if candidate > cur {
                            best = Some(candidate);
                        }
                    } else {
                        best = Some(candidate);
                    }
                }
            }
        }

        best.ok_or_else(|| RegistryError::NotFound { agent_id: "no compatible protocol version found".to_string() })
    }
}

/// Router for messages based on sessions and capabilities.
pub struct Router;

impl Router {
    /// Routes a message to the appropriate session based on the destination.
    pub fn route(
        &self,
        msg: &Message,
        sessions: &SessionManager,
    ) -> Result<RoutingResult, RegistryError> {
        // Try to find an active session where the 'to' field matches either client_id or server_id
        let all_sessions = sessions
            .get_session_by_client_or_server(&msg.to)
            .map_err(|_| RegistryError::NotFound { agent_id: "failed to retrieve sessions".to_string() })?;

        // For simplicity, select the first active session that matches
        for session in all_sessions {
            if session.active {
                // Update last activity
                let _ = sessions.touch_session(session.id);
                tracing::info!("Routed message {:?} to session {:?}", msg.id, session.id);
                return Ok(RoutingResult {
                    session: session.id,
                    success: true,
                    error: None,
                });
            }
        }

        tracing::warn!("No active session found for destination: {}", msg.to);
        Ok(RoutingResult {
            session: SessionId(0),
            success: false,
            error: Some("No active session".to_string()),
        })
    }
}

/// Enhanced A2A system that combines capability matching, protocol negotiation,
/// session management, and message routing.
pub struct EnhancedA2A {
    session_manager: SessionManager,
    capability_matcher: CapabilityMatcher,
    protocol_negotiator: ProtocolNegotiator,
    router: Router,
}

impl EnhancedA2A {
    /// Creates a new EnhancedA2A instance.
    pub fn new(session_timeout: Duration) -> Self {
        EnhancedA2A {
            session_manager: SessionManager::new(session_timeout),
            capability_matcher: CapabilityMatcher,
            protocol_negotiator: ProtocolNegotiator,
            router: Router,
        }
    }

    /// Establishes a new session after matching capabilities and negotiating a protocol version.
    pub fn establish_session(
        &self,
        client_id: String,
        server_id: String,
        client_capabilities: CapabilitySet,
        client_versions: Vec<ProtocolVersion>,
        server_capabilities: CapabilitySet,
        server_versions: Vec<ProtocolVersion>,
    ) -> Result<SessionId, RegistryError> {
        // Step 1: Match capabilities
        let match_result = self.capability_matcher.match_capabilities(&client_capabilities, &server_capabilities)?;
        tracing::info!("Capability match result: {:?}", match_result);

        // Step 2: Negotiate protocol version
        let version = self.protocol_negotiator.negotiate(&client_versions, &server_versions)?;
        tracing::info!("Negotiated protocol version: {:?}", version);

        // Step 3: Create session with negotiated capabilities
        let session_capabilities = client_capabilities; // Could be filtered by matched set
        let session_id = self.session_manager.create_session(
            client_id,
            server_id,
            session_capabilities,
            version,
        )?;
        Ok(session_id)
    }

    /// Sends a message by routing it to the appropriate session.
    pub fn send_message(&self, msg: Message) -> Result<RoutingResult, RegistryError> {
        self.router.route(&msg, &self.session_manager)
    }

    /// Terminates a session.
    pub fn close_session(&self, session_id: SessionId) -> Result<(), RegistryError> {
        self.session_manager.terminate_session(session_id)
    }

    /// Retrieves the current session information.
    pub fn get_session(&self, session_id: SessionId) -> Result<Session, RegistryError> {
        self.session_manager.get_session(session_id)
    }

    /// Performs periodic cleanup of expired sessions.
    pub fn cleanup(&self) -> Result<usize, RegistryError> {
        self.session_manager.cleanup_expired()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ProtocolVersion tests =====

    #[test]
    fn test_protocol_version_new() {
        let v = ProtocolVersion::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_protocol_version_is_compatible_same_major() {
        let v1 = ProtocolVersion::new(1, 0, 0);
        let v2 = ProtocolVersion::new(1, 5, 10);
        assert!(v1.is_compatible(&v2));
        assert!(v2.is_compatible(&v1));
    }

    #[test]
    fn test_protocol_version_is_not_compatible_different_major() {
        let v1 = ProtocolVersion::new(1, 0, 0);
        let v2 = ProtocolVersion::new(2, 0, 0);
        assert!(!v1.is_compatible(&v2));
        assert!(!v2.is_compatible(&v1));
    }

    #[test]
    fn test_protocol_version_ordering() {
        let v1 = ProtocolVersion::new(1, 0, 0);
        let v2 = ProtocolVersion::new(1, 1, 0);
        let v3 = ProtocolVersion::new(2, 0, 0);
        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v1 < v3);
    }

    #[test]
    fn test_protocol_version_partial_eq() {
        let v1 = ProtocolVersion::new(1, 2, 3);
        let v2 = ProtocolVersion::new(1, 2, 3);
        let v3 = ProtocolVersion::new(1, 2, 4);
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    // ===== CapabilitySet tests =====

    #[test]
    fn test_capability_set_empty() {
        let cs = CapabilitySet::default();
        assert!(cs.capabilities.is_empty());
    }

    #[test]
    fn test_capability_set_with_capabilities() {
        let mut cs = CapabilitySet::default();
        cs.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 0.5,
        });
        cs.capabilities.insert("streaming".to_string(), Capability {
            name: "streaming".to_string(),
            version: "1.0".to_string(),
            weight: 0.5,
        });
        assert_eq!(cs.capabilities.len(), 2);
        assert!(cs.capabilities.contains_key("tool_call"));
        assert!(cs.capabilities.contains_key("streaming"));
    }

    // ===== CapabilityMatcher tests =====

    #[test]
    fn test_match_capabilities_perfect_match() {
        let mut set_a = CapabilitySet::default();
        set_a.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let mut set_b = CapabilitySet::default();
        set_b.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let matcher = CapabilityMatcher;
        let result = matcher.match_capabilities(&set_a, &set_b).unwrap();
        assert_eq!(result.score, 1.0);
        assert_eq!(result.matched, vec!["tool_call"]);
        assert!(result.missing_in_a.is_empty());
        assert!(result.missing_in_b.is_empty());
    }

    #[test]
    fn test_match_capabilities_partial_match() {
        let mut set_a = CapabilitySet::default();
        set_a.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 0.5,
        });
        set_a.capabilities.insert("streaming".to_string(), Capability {
            name: "streaming".to_string(),
            version: "1.0".to_string(),
            weight: 0.5,
        });

        let mut set_b = CapabilitySet::default();
        set_b.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let matcher = CapabilityMatcher;
        let result = matcher.match_capabilities(&set_a, &set_b).unwrap();
        assert_eq!(result.score, 0.5);
        assert_eq!(result.matched, vec!["tool_call"]);
        assert!(result.missing_in_a.is_empty());
        assert_eq!(result.missing_in_b, vec!["streaming"]);
    }

    #[test]
    fn test_match_capabilities_version_mismatch() {
        let mut set_a = CapabilitySet::default();
        set_a.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let mut set_b = CapabilitySet::default();
        set_b.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "2.0".to_string(),
            weight: 1.0,
        });

        let matcher = CapabilityMatcher;
        let result = matcher.match_capabilities(&set_a, &set_b).unwrap();
        assert_eq!(result.score, 0.0);
        assert!(result.matched.is_empty());
        assert_eq!(result.missing_in_a, vec!["tool_call"]);
    }

    #[test]
    fn test_match_capabilities_empty_sets() {
        let set_a = CapabilitySet::default();
        let set_b = CapabilitySet::default();

        let matcher = CapabilityMatcher;
        let result = matcher.match_capabilities(&set_a, &set_b).unwrap();
        assert_eq!(result.score, 0.0);
        assert!(result.matched.is_empty());
    }

    // ===== ProtocolNegotiator tests =====

    #[test]
    fn test_negotiate_basic() {
        let client = vec![ProtocolVersion::new(1, 0, 0), ProtocolVersion::new(1, 1, 0)];
        let server = vec![ProtocolVersion::new(1, 0, 0), ProtocolVersion::new(1, 1, 0)];
        let result = ProtocolNegotiator::negotiate(&client, &server).unwrap();
        assert_eq!(result, ProtocolVersion::new(1, 1, 0));
    }

    #[test]
    fn test_negotiate_client_empty() {
        let client: Vec<ProtocolVersion> = vec![];
        let server = vec![ProtocolVersion::new(1, 0, 0)];
        let result = ProtocolNegotiator::negotiate(&client, &server);
        assert!(result.is_err());
    }

    #[test]
    fn test_negotiate_server_empty() {
        let client = vec![ProtocolVersion::new(1, 0, 0)];
        let server: Vec<ProtocolVersion> = vec![];
        let result = ProtocolNegotiator::negotiate(&client, &server);
        assert!(result.is_err());
    }

    #[test]
    fn test_negotiate_no_compatible_version() {
        let client = vec![ProtocolVersion::new(1, 0, 0)];
        let server = vec![ProtocolVersion::new(2, 0, 0)];
        let result = ProtocolNegotiator::negotiate(&client, &server);
        assert!(result.is_err());
    }

    #[test]
    fn test_negotiate_skips_incompatible() {
        let client = vec![ProtocolVersion::new(1, 0, 0), ProtocolVersion::new(2, 0, 0)];
        let server = vec![ProtocolVersion::new(3, 0, 0)];
        let result = ProtocolNegotiator::negotiate(&client, &server);
        assert!(result.is_err());
    }

    // ===== SessionManager tests =====

    fn make_cap_set() -> CapabilitySet {
        let mut cs = CapabilitySet::default();
        cs.capabilities.insert("test".to_string(), Capability {
            name: "test".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });
        cs
    }

    #[test]
    fn test_create_session() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        assert_eq!(id.0, 1);
    }

    #[test]
    fn test_get_session() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        let session = mgr.get_session(id).unwrap();
        assert_eq!(session.client_id, "client1");
        assert_eq!(session.server_id, "server1");
        assert!(session.active);
    }

    #[test]
    fn test_get_session_not_found() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let result = mgr.get_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_terminate_session() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        mgr.terminate_session(id).unwrap();
        let session = mgr.get_session(id).unwrap();
        assert!(!session.active);
    }

    #[test]
    fn test_terminate_session_not_found() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let result = mgr.terminate_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_session_by_client_or_server_by_client() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        let session = mgr.get_session_by_client_or_server("client1").unwrap();
        assert_eq!(session.id, id);
    }

    #[test]
    fn test_get_session_by_client_or_server_by_server() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        let session = mgr.get_session_by_client_or_server("server1").unwrap();
        assert_eq!(session.id, id);
    }

    #[test]
    fn test_get_session_by_client_or_server_not_found() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let result = mgr.get_session_by_client_or_server("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_touch_session() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        let before = mgr.get_session(id).unwrap().last_activity;
        std::thread::sleep(Duration::from_millis(10));
        mgr.touch_session(id).unwrap();
        let after = mgr.get_session(id).unwrap().last_activity;
        assert!(after > before);
    }

    #[test]
    fn test_touch_session_not_found() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let result = mgr.touch_session(SessionId(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_expired() {
        let mgr = SessionManager::new(Duration::from_millis(50));
        let id1 = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        // Inactive session should be cleaned up
        mgr.terminate_session(id1).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let cleaned = mgr.cleanup_expired().unwrap();
        assert_eq!(cleaned, 1);
    }

    #[test]
    fn test_cleanup_active_session_preserved() {
        let mgr = SessionManager::new(Duration::from_millis(50));
        let _id = mgr.create_session(
            "client1".to_string(),
            "server1".to_string(),
            make_cap_set(),
            ProtocolVersion::new(1, 0, 0),
        ).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let cleaned = mgr.cleanup_expired().unwrap();
        assert_eq!(cleaned, 0); // active sessions preserved
    }

    // ===== Router tests =====

    #[test]
    fn test_router_no_session() {
        let mgr = SessionManager::new(Duration::from_secs(300));
        let router = Router;
        let msg = Message {
            id: 1,
            from: "a".to_string(),
            to: "unknown".to_string(),
            payload: vec![],
            message_type: "test".to_string(),
        };
        // get_session_by_client_or_server returns Err for unknown, route maps to RegistryError::NotFound
        let result = router.route(&msg, &mgr);
        assert!(result.is_err());
    }

    // ===== EnhancedA2A tests =====

    #[test]
    fn test_enhanced_a2a_new() {
        let a2a = EnhancedA2A::new(Duration::from_secs(300));
        // Just verify it doesn't panic
    }

    #[test]
    fn test_establish_session() {
        let a2a = EnhancedA2A::new(Duration::from_secs(300));

        let mut client_caps = CapabilitySet::default();
        client_caps.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let mut server_caps = CapabilitySet::default();
        server_caps.capabilities.insert("tool_call".to_string(), Capability {
            name: "tool_call".to_string(),
            version: "1.0".to_string(),
            weight: 1.0,
        });

        let session_id = a2a.establish_session(
            "client1".to_string(),
            "server1".to_string(),
            client_caps,
            vec![ProtocolVersion::new(1, 0, 0)],
            server_caps,
            vec![ProtocolVersion::new(1, 0, 0)],
        ).unwrap();

        assert_eq!(session_id.0, 1);
        let session = a2a.get_session(session_id).unwrap();
        assert_eq!(session.client_id, "client1");
        assert_eq!(session.server_id, "server1");
    }

    #[test]
    fn test_establish_session_no_capability_match() {
        let a2a = EnhancedA2A::new(Duration::from_secs(300));

        let client_caps = CapabilitySet::default();
        let server_caps = CapabilitySet::default();

        // Empty caps = score 0.0 but still Ok
        let session_id = a2a.establish_session(
            "client1".to_string(),
            "server1".to_string(),
            client_caps,
            vec![ProtocolVersion::new(1, 0, 0)],
            server_caps,
            vec![ProtocolVersion::new(1, 0, 0)],
        ).unwrap();
        assert_eq!(session_id.0, 1);
    }

    #[test]
    fn test_close_session() {
        let a2a = EnhancedA2A::new(Duration::from_secs(300));

        let caps = make_cap_set();
        let sid = a2a.establish_session(
            "client1".to_string(),
            "server1".to_string(),
            caps,
            vec![ProtocolVersion::new(1, 0, 0)],
            CapabilitySet::default(),
            vec![ProtocolVersion::new(1, 0, 0)],
        ).unwrap();

        a2a.close_session(sid).unwrap();
        let session = a2a.get_session(sid).unwrap();
        assert!(!session.active);
    }

    #[test]
    fn test_cleanup() {
        let a2a = EnhancedA2A::new(Duration::from_millis(50));
        let caps = make_cap_set();
        let sid = a2a.establish_session(
            "client1".to_string(),
            "server1".to_string(),
            caps,
            vec![ProtocolVersion::new(1, 0, 0)],
            CapabilitySet::default(),
            vec![ProtocolVersion::new(1, 0, 0)],
        ).unwrap();
        a2a.close_session(sid).unwrap();
        std::thread::sleep(Duration::from_millis(60));
        let cleaned = a2a.cleanup().unwrap();
        assert_eq!(cleaned, 1);
    }

    // ===== Message and RoutingResult tests =====

    #[test]
    fn test_message_debug() {
        let msg = Message {
            id: 42,
            from: "alice".to_string(),
            to: "bob".to_string(),
            payload: vec![1, 2, 3],
            message_type: "chat".to_string(),
        };
        let debug = format!("{:?}", msg);
        assert!(debug.contains("42"));
        assert!(debug.contains("alice"));
        assert!(debug.contains("bob"));
    }

    #[test]
    fn test_routing_result_success() {
        let result = RoutingResult {
            session: SessionId(1),
            success: true,
            error: None,
        };
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_routing_result_failure() {
        let result = RoutingResult {
            session: SessionId(0),
            success: false,
            error: Some("No active session".to_string()),
        };
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    // ===== SessionId tests =====

    #[test]
    fn test_session_id_copy() {
        let id = SessionId(42);
        let id2 = id; // Copy
        assert_eq!(id, id2);
    }

    #[test]
    fn test_session_id_hash() {
        use std::collections::HashSet;
        let id1 = SessionId(1);
        let id2 = SessionId(2);
        let id3 = SessionId(1);
        let mut set = HashSet::new();
        set.insert(id1);
        set.insert(id2);
        assert!(set.contains(&id3));
        assert_eq!(set.len(), 2);
    }
}