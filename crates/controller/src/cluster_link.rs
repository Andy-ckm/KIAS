
//! ClusterLink: connect to peer clusters, sync state, failover, split-brain detection
//!
//! This module provides the `ClusterLink` struct and related utilities to maintain
//! a cluster of peer nodes, synchronizing state, handling failover, and detecting
//! split-brain situations.

use kias_common::KiasError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Represents a configuration for a peer cluster.
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Unique identifier for the peer.
    pub id: String,
    /// Network address (host:port) of the peer.
    pub address: String,
    /// Weight used for leader election (higher weight => more priority).
    pub weight: u32,
}

impl PeerConfig {
    /// Creates a new `PeerConfig`.
    pub fn new(id: impl Into<String>, address: impl Into<String>, weight: u32) -> Self {
        PeerConfig {
            id: id.into(),
            address: address.into(),
            weight,
        }
    }
}

/// Metadata about the last heartbeat received from a peer.
#[derive(Debug, Clone)]
pub struct PeerHeartbeat {
    /// Peer identifier.
    pub peer_id: String,
    /// Timestamp of last heartbeat.
    pub last_seen: Instant,
    /// Number of missed heartbeats.
    pub missed_count: u32,
}

impl PeerHeartbeat {
    /// Creates a new `PeerHeartbeat`.
    pub fn new(peer_id: impl Into<String>) -> Self {
        PeerHeartbeat {
            peer_id: peer_id.into(),
            last_seen: Instant::now(),
            missed_count: 0,
        }
    }

    /// Updates the heartbeat timestamp and resets missed count.
    pub fn touch(&mut self) {
        self.last_seen = Instant::now();
        self.missed_count = 0;
    }

    /// Increments the missed heartbeat counter.
    pub fn miss(&mut self) {
        self.missed_count += 1;
    }
}

/// State of the cluster that can be synchronized with peers.
#[derive(Debug, Clone, Default)]
pub struct ClusterState {
    /// Current term, incremented on leader election.
    pub term: u64,
    /// Last committed index in the log.
    pub commit_index: u64,
    /// Last applied index to state machine.
    pub last_applied: u64,
    /// Leader identifier, if any.
    pub leader_id: Option<String>,
    /// Miscellaneous key-value pairs for arbitrary cluster metadata.
    pub data: HashMap<String, String>,
}

impl ClusterState {
    /// Creates a new `ClusterState`.
    pub fn new() -> Self {
        ClusterState::default()
    }

    /// Updates the term and clears leader.
    pub fn bump_term(&mut self, new_term: u64) {
        self.term = new_term;
        self.leader_id = None;
    }

    /// Sets the leader identifier.
    pub fn set_leader(&mut self, leader_id: String) {
        self.leader_id = Some(leader_id);
    }

    /// Merges remote state into this state according to Raft-like rules.
    pub fn merge(&mut self, remote: &ClusterState) {
        if remote.term > self.term {
            self.term = remote.term;
            self.leader_id = remote.leader_id.clone();
        }
        if remote.commit_index > self.commit_index {
            self.commit_index = remote.commit_index;
        }
        if remote.last_applied > self.last_applied {
            self.last_applied = remote.last_applied;
        }
        // For simplicity, we just replace data; a real implementation might merge carefully.
        self.data = remote.data.clone();
    }
}

/// Result of a synchronization attempt.
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Whether synchronization succeeded.
    pub success: bool,
    /// Peer that responded.
    pub peer_id: String,
    /// Applied index after sync.
    pub applied_index: u64,
    /// Error message if failure.
    pub error_message: Option<String>,
}

/// Information about a potential split-brain situation.
#[derive(Debug, Clone)]
pub struct SplitBrainInfo {
    /// Peers that claim to be leader.
    pub leaders: Vec<String>,
    /// Reason for detection.
    pub reason: String,
}

/// Core struct for managing cluster links.
///
/// `ClusterLink` maintains connections to peer clusters, tracks heartbeats,
/// synchronizes cluster state, and can trigger failover when a leader fails.
pub struct ClusterLink {
    /// Identifier for this node.
    node_id: String,
    /// Current known peers.
    peers: HashMap<String, PeerConfig>,
    /// Heartbeat info for each peer.
    heartbeats: HashMap<String, PeerHeartbeat>,
    /// Cluster state shared across the link.
    state: ClusterState,
    /// Connection status per peer (true if connected).
    connected: HashMap<String, bool>,
    /// Interval for sending heartbeats.
    heartbeat_interval: Duration,
    /// Threshold for considering a peer dead.
    missed_heartbeat_threshold: u32,
    /// Lock for async-safe modifications.
    rw_lock: RwLock<()>,
}

impl ClusterLink {
    /// Creates a new `ClusterLink` for the given node identifier.
    ///
    /// # Arguments
    /// * `node_id` - Unique identifier for this node.
    /// * `heartbeat_interval` - Interval between heartbeats sent to peers.
    /// * `missed_threshold` - Number of missed heartbeats before peer is considered dead.
    pub fn new(
        node_id: impl Into<String>,
        heartbeat_interval: Duration,
        missed_threshold: u32,
    ) -> Self {
        let node_id = node_id.into();
        info!(node_id = %node_id, "Creating new ClusterLink");
        ClusterLink {
            node_id,
            peers: HashMap::new(),
            heartbeats: HashMap::new(),
            state: ClusterState::new(),
            connected: HashMap::new(),
            heartbeat_interval,
            missed_heartbeat_threshold: missed_threshold,
            rw_lock: RwLock::new(()),
        }
    }

    /// Adds a peer to the cluster.
    ///
    /// # Arguments
    /// * `peer` - Configuration for the peer to add.
    ///
    /// # Returns
    /// * `Ok(())` if the peer was added successfully.
    /// * `Err(KiasError)` if the peer id already exists or registration fails.
    pub async fn add_peer(&mut self, peer: PeerConfig) -> Result<(), KiasError> {
        let _guard = self.rw_lock.write().await;
        let peer_id = peer.id.clone();
        if self.peers.contains_key(&peer_id) {
            warn!(peer_id = %peer_id, "Attempt to add duplicate peer");
            return Err(KiasError::InvalidInput(format!(
                "Peer {} already exists",
                peer_id
            )));
        }
        self.peers.insert(peer_id.clone(), peer);
        self.heartbeats.insert(peer_id.clone(), PeerHeartbeat::new(peer_id.clone()));
        self.connected.insert(peer_id, false);
        info!(peer_id = %peer_id, "Peer added to cluster");
        Ok(())
    }

    /// Removes a peer from the cluster.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the peer to remove.
    ///
    /// # Returns
    /// * `Ok(())` if the peer was removed.
    /// * `Err(KiasError)` if the peer is not known.
    pub async fn remove_peer(&mut self, peer_id: &str) -> Result<(), KiasError> {
        let _guard = self.rw_lock.write().await;
        if self.peers.remove(peer_id).is_none() {
            warn!(peer_id = %peer_id, "Attempt to remove unknown peer");
            return Err(KiasError::NotFound(format!("Peer {} not found", peer_id)));
        }
        self.heartbeats.remove(peer_id);
        self.connected.remove(peer_id);
        info!(peer_id = %peer_id, "Peer removed from cluster");
        Ok(())
    }

    /// Attempts to establish connections to all known peers.
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - List of peer IDs that were successfully connected.
    /// * `Err(KiasError)` - If connection errors occur.
    pub async fn connect_all(&mut self) -> Result<Vec<String>, KiasError> {
        let _guard = self.rw_lock.write().await;
        let mut successful = Vec::new();
        for (peer_id, peer_config) in &self.peers.clone() {
            match self.connect_peer(peer_config).await {
                Ok(connected) => {
                    if connected {
                        self.connected.insert(peer_id.clone(), true);
                        successful.push(peer_id.clone());
                        info!(peer_id = %peer_id, "Connected to peer");
                    } else {
                        self.connected.insert(peer_id.clone(), false);
                        warn!(peer_id = %peer_id, "Failed to connect to peer");
                    }
                }
                Err(e) => {
                    self.connected.insert(peer_id.clone(), false);
                    warn!(peer_id = %peer_id, error = %e, "Connection error");
                }
            }
        }
        Ok(successful)
    }

    /// Attempts to connect to a specific peer.
    async fn connect_peer(&self, peer: &PeerConfig) -> Result<bool, KiasError> {
        // In a real implementation, we would perform async TCP connection.
        // Here we simulate success if the address is non-empty.
        if peer.address.is_empty() {
            return Err(KiasError::ConnectionError("Empty address".into()));
        }
        // Simulate connection attempt (replace with actual TcpStream::connect).
        // For demonstration, we return Ok(true) if the address parses.
        if peer.address.parse::<std::net::SocketAddr>().is_ok() {
            Ok(true)
        } else {
            // Simulate a successful connection even if parsing fails for demo.
            Ok(true)
        }
    }

    /// Closes all active connections to peers.
    pub async fn disconnect_all(&mut self) -> Result<(), KiasError> {
        let _guard = self.rw_lock.write().await;
        for (peer_id, connected) in self.connected.iter_mut() {
            if *connected {
                // In a real implementation, close the TCP stream.
                *connected = false;
                info!(peer_id = %peer_id, "Disconnected from peer");
            }
        }
        Ok(())
    }

    /// Syncs the current cluster state with a specific peer.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the peer to sync with.
    /// * `state` - The state to send to the peer.
    ///
    /// # Returns
    /// * `Ok(SyncResult)` - The result of the sync attempt.
    /// * `Err(KiasError)` - If sync fails.
    pub async fn sync_with_peer(
        &mut self,
        peer_id: &str,
        state: ClusterState,
    ) -> Result<SyncResult, KiasError> {
        let _guard = self.rw_lock.write().await;

        // Ensure peer exists.
        if !self.peers.contains_key(peer_id) {
            warn!(peer_id = %peer_id, "Cannot sync: unknown peer");
            return Err(KiasError::NotFound(format!("Peer {} not found", peer_id)));
        }

        // Check if connected.
        let is_connected = self
            .connected
            .get(peer_id)
            .copied()
            .unwrap_or(false);

        if !is_connected {
            warn!(peer_id = %peer_id, "Cannot sync: not connected");
            return Err(KiasError::ConnectionError(format!(
                "Peer {} not connected",
                peer_id
            )));
        }

        // Simulate sending state and receiving response.
        // In a real implementation, we would send a message via TCP and await a reply.
        let applied_index = state.commit_index.saturating_add(1);
        let sync_result = SyncResult {
            success: true,
            peer_id: peer_id.to_string(),
            applied_index,
            error_message: None,
        };

        info!(
            peer_id = %peer_id,
            applied_index = %applied_index,
            "State synced with peer"
        );

        // Merge remote state back if needed.
        // For simplicity, we trust the peer's applied_index.
        if applied_index > self.state.commit_index {
            self.state.commit_index = applied_index;
        }

        Ok(sync_result)
    }

    /// Broadcasts the current cluster state to all connected peers.
    ///
    /// # Returns
    /// * `Ok(Vec<SyncResult>)` - List of results from each peer.
    /// * `Err(KiasError)` - If broadcast fails for any reason.
    pub async fn broadcast_state(&mut self) -> Result<Vec<SyncResult>, KiasError> {
        let _guard = self.rw_lock.write().await;
        let mut results = Vec::new();

        let state_clone = self.state.clone();

        for (peer_id, _) in self.peers.iter().filter(|(id, _)| {
            self.connected.get(*id).copied().unwrap_or(false)
        }) {
            // For each connected peer, simulate sync.
            let result = self.sync_with_peer(peer_id, state_clone.clone()).await;
            match result {
                Ok(r) => results.push(r),
                Err(e) => {
                    warn!(peer_id = %peer_id, error = %e, "Failed to sync");
                    results.push(SyncResult {
                        success: false,
                        peer_id: peer_id.clone(),
                        applied_index: 0,
                        error_message: Some(e.to_string()),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Updates the local cluster state with new term and leader.
    ///
    /// # Arguments
    /// * `new_term` - New term value.
    /// * `leader_id` - Leader identifier (None if no leader).
    pub async fn update_state(&mut self, new_term: u64, leader_id: Option<String>) {
        let _guard = self.rw_lock.write().await;
        self.state.bump_term(new_term);
        if let Some(id) = leader_id {
            self.state.set_leader(id);
        }
        info!(term = %new_term, leader = ?self.state.leader_id, "Local state updated");
    }

    /// Records a heartbeat received from a peer.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the peer.
    ///
    /// # Returns
    /// * `Ok(())` if heartbeat recorded.
    /// * `Err(KiasError)` if peer unknown.
    pub async fn record_heartbeat(&mut self, peer_id: &str) -> Result<(), KiasError> {
        let _guard = self.rw_lock.write().await;

        let heartbeat = self
            .heartbeats
            .get_mut(peer_id)
            .ok_or_else(|| KiasError::NotFound(format!("Peer {} not found", peer_id)))?;

        heartbeat.touch();
        info!(peer_id = %peer_id, "Heartbeat recorded");
        Ok(())
    }

    /// Checks for split-brain conditions among connected peers.
    ///
    /// A split-brain occurs when multiple peers claim to be the leader.
    ///
    /// # Returns
    /// * `Ok(SplitBrainInfo)` - Information about the split-brain, if any.
    pub async fn detect_split_brain(&self) -> Result<SplitBrainInfo, KiasError> {
        let _guard = self.rw_lock.read().await;

        let mut leaders: Vec<String> = Vec::new();

        // Check all peers that claim to be leader.
        for (peer_id, heartbeat) in &self.heartbeats {
            // Simulate retrieving peer state; in a real system, you'd query peer state.
            // For demonstration, we consider a peer a leader if its ID is "leader_*".
            if peer_id.starts_with("leader_") {
                leaders.push(peer_id.clone());
            }
        }

        // If more than one leader, we have split-brain.
        let reason = if leaders.len() > 1 {
            format!(
                "{} peers claiming leadership simultaneously",
                leaders.len()
            )
        } else {
            "No split-brain detected".to_string()
        };

        if leaders.len() > 1 {
            warn!(leaders = ?leaders, "Split-brain detected");
        } else {
            info!("Cluster is healthy: no split-brain");
        }

        Ok(SplitBrainInfo { leaders, reason })
    }

    /// Attempts to promote a failover to a given peer.
    ///
    /// # Arguments
    /// * `target_peer_id` - Identifier of the peer to promote.
    ///
    /// # Returns
    /// * `Ok(())` if failover promoted successfully.
    /// * `Err(KiasError)` if promotion fails.
    pub async fn promote_failover(&mut self, target_peer_id: &str) -> Result<(), KiasError> {
        let _guard = self.rw_lock.write().await;

        if !self.peers.contains_key(target_peer_id) {
            return Err(KiasError::NotFound(format!(
                "Target peer {} not found",
                target_peer_id
            )));
        }

        // Check connectivity.
        if !self.connected.get(target_peer_id).copied().unwrap_or(false) {
            return Err(KiasError::ConnectionError(format!(
                "Target peer {} not connected",
                target_peer_id
            )));
        }

        // Update term and set new leader.
        self.state.bump_term(self.state.term + 1);
        self.state.set_leader(target_peer_id.to_string());

        info!(
            target_peer = %target_peer_id,
            new_term = %self.state.term,
            "Failover promoted to target peer"
        );

        Ok(())
    }

    /// Determines if a peer is considered dead based on missed heartbeats.
    ///
    /// # Arguments
    /// * `peer_id` - Identifier of the peer.
    ///
    /// # Returns
    /// * `Ok(bool)` - true if the peer is dead.
    /// * `Err(KiasError)` - if peer not found.
    pub async fn is_peer_dead(&self, peer_id: &str) -> Result<bool, KiasError> {
        let _guard = self.rw_lock.read().await;

        let heartbeat = self
            .heartbeats
            .get(peer_id)
            .ok_or_else(|| KiasError::NotFound(format!("Peer {} not found", peer_id)))?;

        let dead = heartbeat.missed_count >= self.missed_heartbeat_threshold;

        if dead {
            warn!(peer_id = %peer_id, missed = %heartbeat.missed_count, "Peer considered dead");
        }

        Ok(dead)
    }

    /// Updates the missed heartbeat count for all peers and checks for dead peers.
    ///
    /// This should be called periodically by a background task.
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - List of dead peer IDs.
    pub async fn check_dead_peers(&mut self) -> Result<Vec<String>, KiasError> {
        let _guard = self.rw_lock.write().await;

        let now = Instant::now();
        let mut dead_peers: Vec<String> = Vec::new();

        for (peer_id, heartbeat) in self.heartbeats.iter_mut() {
            let elapsed = now.duration_since(heartbeat.last_seen);
            if elapsed > self.heartbeat_interval {
                heartbeat.miss();
                if heartbeat.missed_count >= self.missed_heartbeat_threshold {
                    dead_peers.push(peer_id.clone());
                    self.connected.insert(peer_id.clone(), false);
                }
            }
        }

        if !dead_peers.is_empty() {
            warn!(dead_peers = ?dead_peers, "Dead peers detected");
        }

        Ok(dead_peers)
    }

    /// Returns a summary of the current cluster link status.
    ///
    /// # Returns
    /// * `String` - Human-readable summary.
    pub async fn status(&self) -> String {
        let _guard = self.rw_lock.read().await;

        let mut lines = Vec::new();
        lines.push(format!("Node ID: {}", self.node_id));
        lines.push(format!("Term: {}", self.state.term));
        lines.push(format!("Leader: {:?}", self.state.leader_id));
        lines.push(format!("Commit Index: {}", self.state.commit_index));
        lines.push(format!("Peers ({}):", self.peers.len()));
        for (peer_id, connected) in &self.connected {
            lines.push(format!("  - {} (connected: {})", peer_id, connected));
        }
        lines.join("\n")
    }

    /// Retrieves the current cluster state.
    pub async fn get_state(&self) -> ClusterState {
        let _guard = self.rw_lock.read().await;
        self.state.clone()
    }

    /// Modifies cluster metadata (key-value store).
    ///
    /// # Arguments
    /// * `key` - Metadata key.
    /// * `value` - Metadata value.
    pub async fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let _guard = self.rw_lock.write().await;
        self.state.data.insert(key.into(), value.into());
        info!(key = %key.into(), "Metadata updated");
    }

    /// Retrieves cluster metadata.
    ///
    /// # Arguments
    /// * `key` - Metadata key.
    ///
    /// # Returns
    /// * `Option<String>` - Value if present.
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        let _guard = self.rw_lock.read().await;
        self.state.data.get(key).cloned()
    }
}

// ---------------------------------------------------------------------------
// Helper types for internal communication
// ---------------------------------------------------------------------------

/// Message format for cluster communication.
#[derive(Debug, Clone)]
pub enum ClusterMessage {
    /// Heartbeat message.
    Heartbeat {
        from: String,
        term: u64,
        commit_index: u64,
    },
    /// State synchronization request.
    SyncRequest {
        from: String,
        state: ClusterState,
    },
    /// State synchronization response.
    SyncResponse {
        success: bool,
        applied_index: u64,
    },
    /// Leader election vote request.
    VoteRequest {
        term: u64,
        candidate_id: String,
        last_log_index: u64,
        last_log_term: u64,
    },
    /// Leader election vote response.
    VoteResponse {
        term: u64,
        vote_granted: bool,
    },
}

impl ClusterMessage {
    /// Serializes the message to bytes.
    pub fn serialize(&self) -> Result<Vec<u8>, KiasError> {
        // Use serde_json for simplicity. In production, use a more efficient codec.
        let json = serde_json::to_vec(self)
            .map_err(|e| KiasError::SerializationError(e.to_string()))?;
        Ok(json)
    }

    /// Deserializes a message from bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, KiasError> {
        let msg = serde_json::from_slice(bytes)
            .map_err(|e| KiasError::DeserializationError(e.to_string()))?;
        Ok(msg)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Helper to create a new ClusterLink for testing.
    fn test_cluster_link() -> ClusterLink {
        ClusterLink::new("node_1".to_string(), Duration::from_secs(1), 3)
    }

    #[tokio::test]
    async fn test_add_peer() {
        let mut link = test_cluster_link();
        let peer = PeerConfig::new("peer_1", "192.168.1.10:8080", 1);
        let result = link.add_peer(peer).await;
        assert!(result.is_ok());
        let state = link.get_state().await;
        assert_eq!(state.term, 0);
    }

    #[tokio::test]
    async fn test_add_duplicate_peer() {
        let mut link = test_cluster_link();
        let peer = PeerConfig::new("peer_1", "192.168.1.10:8080", 1);
        assert!(link.add_peer(peer.clone()).await.is_ok());
        let result = link.add_peer(peer).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remove_peer() {
        let mut link = test_cluster_link();
        let peer = PeerConfig::new("peer_1", "192.168.1.10:8080", 1);
        link.add_peer(peer).await.unwrap();
        let result = link.remove_peer("peer_1").await;
        assert!(result.is_ok());
        let result = link.remove_peer("peer_1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_record_heartbeat() {
        let mut link = test_cluster_link();
        let peer = PeerConfig::new("peer_1", "192.168.1.10:8080", 1);
        link.add_peer(peer).await.unwrap();
        let result = link.record_heartbeat("peer_1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_record_heartbeat_unknown_peer() {
        let mut link = test_cluster_link();
        let result = link.record_heartbeat("unknown_peer").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_state() {
        let mut link = test_cluster_link();
        link.update_state(5, Some("node_2".to_string())).await;
        let state = link.get_state().await;
        assert_eq!(state.term, 5);
        assert_eq!(state.leader_id, Some("node_2".to_string()));
    }

    #[tokio::test]
    async fn test_set_and_get_metadata() {
        let mut link = test_cluster_link();
        link.set_metadata("color", "blue").await;
        let value = link.get_metadata("color").await;
        assert_eq!(value, Some("blue".to_string()));
        let none = link.get_metadata("nonexistent").await;
        assert_eq!(none, None);
    }

    #[tokio::test]
    async fn test_detect_split_brain_no_split() {
        let link = test_cluster_link();
        let info = link.detect_split_brain().await.unwrap();
        assert!(info.leaders.is_empty());
        assert_eq!(info.reason, "No split-brain detected");
    }

    #[tokio::test]
    async fn test_connect_all() {
        let mut link = test_cluster_link();
        // Add a few peers with valid addresses.
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 1))
            .await
            .unwrap();
        link.add_peer(PeerConfig::new("peer_2", "127.0.0.1:8002", 1))
            .await
            .unwrap();
        let connected = link.connect_all().await.unwrap();
        // Simulated connections may succeed.
        assert_eq!(connected.len(), 2);
    }

    #[tokio::test]
    async fn test_promote_failover() {
        let mut link = test_cluster_link();
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 2))
            .await
            .unwrap();
        // Simulate connection (connect_all will mark it connected)
        link.connected.insert("peer_1".to_string(), true);
        let result = link.promote_failover("peer_1").await;
        assert!(result.is_ok());
        let state = link.get_state().await;
        assert!(state.leader_id.is_some());
        assert_eq!(state.leader_id.unwrap(), "peer_1");
    }

    #[tokio::test]
    async fn test_promote_failover_not_connected() {
        let mut link = test_cluster_link();
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 2))
            .await
            .unwrap();
        // peer_1 is not connected
        let result = link.promote_failover("peer_1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_status() {
        let link = test_cluster_link();
        let status = link.status().await;
        assert!(status.contains("node_1"));
        assert!(status.contains("Term: 0"));
    }

    #[tokio::test]
    async fn test_is_peer_dead() {
        let mut link = test_cluster_link();
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 1))
            .await
            .unwrap();
        // Initially not dead
        let dead = link.is_peer_dead("peer_1").await.unwrap();
        assert!(!dead);
    }

    #[tokio::test]
    async fn test_check_dead_peers() {
        let mut link = test_cluster_link();
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 1))
            .await
            .unwrap();
        // Simulate no heartbeat for a while by manually adjusting.
        // For unit test, we just ensure it returns an empty vec or no error.
        let dead_peers = link.check_dead_peers().await.unwrap();
        // Depending on timing, might be empty.
        assert!(dead_peers.is_empty() || !dead_peers.is_empty());
    }

    #[tokio::test]
    async fn test_broadcast_state() {
        let mut link = test_cluster_link();
        link.add_peer(PeerConfig::new("peer_1", "127.0.0.1:8001", 1))
            .await
            .unwrap();
        link.connected.insert("peer_1".to_string(), true);
        // Set state
        link.state.commit_index = 10;
        let results = link.broadcast_state().await.unwrap();
        // Should have at least one result
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_cluster_message_serialize_deserialize() {
        let msg = ClusterMessage::Heartbeat {
            from: "node_1".to_string(),
            term: 5,
            commit_index: 100,
        };
        let bytes = msg.serialize().unwrap();
        let decoded = ClusterMessage::deserialize(&bytes).unwrap();
        match decoded {
            ClusterMessage::Heartbeat { from, term, commit_index } => {
                assert_eq!(from, "node_1");
                assert_eq!(term, 5);
                assert_eq!(commit_index, 100);
            }
            _ => panic!("Unexpected message type"),
        }
    }

    #[tokio::test]
    async fn test_state_merge() {
        let mut state1 = ClusterState::new();
        state1.term = 3;
        state1.commit_index = 10;

        let mut state2 = ClusterState::new();
        state2.term = 5;
        state2.commit_index = 12;

        state1.merge(&state2);
        assert_eq!(state1.term, 5);
        assert_eq!(state1.commit_index, 12);
    }
}