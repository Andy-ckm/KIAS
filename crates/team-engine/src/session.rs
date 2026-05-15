//! # Session Persistence
//!
//! JSONL-based session logging with context snapshots for conversation recovery.
//!
//! ## Storage Layout
//!
//! ```text
//! {workspace_root}/sessions/{session_id}/
//!   ├── log.jsonl       # Append-only message log (one JSON object per line)
//!   ├── context.json    # Compacted context snapshot for fast restore
//!   └── metadata.json   # Session metadata (created_at, message_count, last_active)
//! ```
//!
//! ## Design Principles
//!
//! 1. Append-only JSONL log for crash-safe message recording
//! 2. Periodic context snapshots for fast session restore
//! 3. VFS-backed via `kias_common::vfs::VirtualFs` (testable with `MemoryFs`)

use chrono::{DateTime, Utc};
use kias_common::vfs::{VfsError, VirtualFs};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Session Config ────────────────────────────────────────────────────

/// Configuration for creating a new session.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Unique session identifier
    pub session_id: String,
    /// User who owns this session
    pub user_id: String,
    /// Root path within the VFS for workspace storage
    pub workspace_root: String,
}

impl SessionConfig {
    pub fn new(
        session_id: impl Into<String>,
        user_id: impl Into<String>,
        workspace_root: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            user_id: user_id.into(),
            workspace_root: workspace_root.into(),
        }
    }
}

// ── Session Metadata ──────────────────────────────────────────────────

/// Metadata tracked for each session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    /// Session identifier
    pub session_id: String,
    /// User who owns this session
    pub user_id: String,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// When the session was last active (last message appended)
    pub last_active: DateTime<Utc>,
    /// Total number of messages in the session
    pub message_count: usize,
}

// ── Session Message ───────────────────────────────────────────────────

/// A single message in a session log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMessage {
    /// Role: "system", "user", "assistant", etc.
    pub role: String,
    /// Message content
    pub content: String,
    /// When this message was recorded
    pub timestamp: DateTime<Utc>,
}

impl SessionMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }
}

// ── Session ───────────────────────────────────────────────────────────

/// A persistent session backed by a virtual filesystem.
///
/// Messages are appended to a JSONL log for crash safety.
/// Context snapshots can be saved/restored for fast session recovery.
pub struct Session {
    config: SessionConfig,
    fs: Arc<dyn VirtualFs>,
    metadata: SessionMetadata,
}

impl Session {
    /// Create a new session, initializing storage on the VFS.
    ///
    /// If a metadata file already exists for this session, it is loaded
    /// (resuming the session). Otherwise a fresh session is created.
    pub async fn new(config: SessionConfig, fs: Arc<dyn VirtualFs>) -> Result<Self, VfsError> {
        let meta_path = Self::metadata_path(&config);

        let metadata = if let Ok(bytes) = fs.read(&meta_path).await {
            serde_json::from_slice(&bytes)
                .map_err(|e| VfsError::Other(format!("corrupt session metadata: {e}")))?
        } else {
            let now = Utc::now();
            SessionMetadata {
                session_id: config.session_id.clone(),
                user_id: config.user_id.clone(),
                created_at: now,
                last_active: now,
                message_count: 0,
            }
        };

        let session = Self { config, fs, metadata };
        session.persist_metadata().await?;
        Ok(session)
    }

    /// Append a message to the session's JSONL log.
    ///
    /// Each call writes one complete JSON object followed by a newline,
    /// making the log crash-safe and streamable.
    pub async fn append_message(
        &mut self,
        role: &str,
        content: &str,
    ) -> Result<SessionMessage, VfsError> {
        let msg = SessionMessage::new(role, content);
        let line = serde_json::to_string(&msg)
            .map_err(|e| VfsError::Other(format!("serialize message: {e}")))?;
        let entry = format!("{line}\n");

        let log_path = Self::log_path(&self.config);
        self.fs.append(&log_path, entry.as_bytes()).await?;

        self.metadata.message_count += 1;
        self.metadata.last_active = Utc::now();
        self.persist_metadata().await?;

        Ok(msg)
    }

    /// Read the full message history from the JSONL log.
    pub async fn get_history(&self) -> Result<Vec<SessionMessage>, VfsError> {
        let log_path = Self::log_path(&self.config);
        let bytes = match self.fs.read(&log_path).await {
            Ok(b) => b,
            Err(VfsError::NotFound(_)) => return Ok(vec![]),
            Err(e) => return Err(e),
        };

        let text = String::from_utf8_lossy(&bytes);
        let mut messages = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: SessionMessage = serde_json::from_str(trimmed)
                .map_err(|e| VfsError::Other(format!("parse log line: {e}")))?;
            messages.push(msg);
        }

        Ok(messages)
    }

    /// Save a context snapshot (e.g. after compaction) for fast restore.
    ///
    /// Overwrites any existing context snapshot.
    pub async fn save_context(&self, messages: &[SessionMessage]) -> Result<(), VfsError> {
        let ctx_path = Self::context_path(&self.config);
        let bytes = serde_json::to_vec_pretty(messages)
            .map_err(|e| VfsError::Other(format!("serialize context: {e}")))?;
        self.fs.write(&ctx_path, &bytes).await
    }

    /// Load the most recent context snapshot.
    ///
    /// Returns an empty vec if no snapshot exists.
    pub async fn load_context(&self) -> Result<Vec<SessionMessage>, VfsError> {
        let ctx_path = Self::context_path(&self.config);
        let bytes = match self.fs.read(&ctx_path).await {
            Ok(b) => b,
            Err(VfsError::NotFound(_)) => return Ok(vec![]),
            Err(e) => return Err(e),
        };

        let messages: Vec<SessionMessage> = serde_json::from_slice(&bytes)
            .map_err(|e| VfsError::Other(format!("parse context snapshot: {e}")))?;
        Ok(messages)
    }

    /// Get the current session metadata.
    pub fn get_metadata(&self) -> &SessionMetadata {
        &self.metadata
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.config.session_id
    }

    /// Get the user ID.
    pub fn user_id(&self) -> &str {
        &self.config.user_id
    }

    // ── Internal helpers ──────────────────────────────────────────────

    fn base_dir(config: &SessionConfig) -> String {
        if config.workspace_root.is_empty() {
            format!("sessions/{}", config.session_id)
        } else {
            format!(
                "{}/sessions/{}",
                config.workspace_root.trim_end_matches('/'),
                config.session_id
            )
        }
    }

    fn log_path(config: &SessionConfig) -> String {
        format!("{}/log.jsonl", Self::base_dir(config))
    }

    fn context_path(config: &SessionConfig) -> String {
        format!("{}/context.json", Self::base_dir(config))
    }

    fn metadata_path(config: &SessionConfig) -> String {
        format!("{}/metadata.json", Self::base_dir(config))
    }

    async fn persist_metadata(&self) -> Result<(), VfsError> {
        let meta_path = Self::metadata_path(&self.config);
        let bytes = serde_json::to_vec_pretty(&self.metadata)
            .map_err(|e| VfsError::Other(format!("serialize metadata: {e}")))?;
        self.fs.write(&meta_path, &bytes).await
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::vfs::MemoryFs;

    fn test_fs() -> Arc<MemoryFs> {
        Arc::new(MemoryFs::new())
    }

    #[tokio::test]
    async fn test_session_new() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "user-1", "");
        let session = Session::new(config, fs).await.unwrap();
        assert_eq!(session.session_id(), "s1");
        assert_eq!(session.user_id(), "user-1");
        assert_eq!(session.get_metadata().message_count, 0);
    }

    #[tokio::test]
    async fn test_append_message_and_get_history() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let mut session = Session::new(config, fs).await.unwrap();

        session.append_message("user", "Hello").await.unwrap();
        session.append_message("assistant", "Hi there!").await.unwrap();
        session.append_message("user", "How are you?").await.unwrap();

        let history = session.get_history().await.unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, "user");
        assert_eq!(history[0].content, "Hello");
        assert_eq!(history[1].role, "assistant");
        assert_eq!(history[1].content, "Hi there!");
        assert_eq!(history[2].role, "user");
        assert_eq!(history[2].content, "How are you?");
    }

    #[tokio::test]
    async fn test_message_count_updates() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let mut session = Session::new(config, fs.clone()).await.unwrap();

        assert_eq!(session.get_metadata().message_count, 0);

        session.append_message("user", "msg1").await.unwrap();
        assert_eq!(session.get_metadata().message_count, 1);

        session.append_message("assistant", "msg2").await.unwrap();
        assert_eq!(session.get_metadata().message_count, 2);

        session.append_message("user", "msg3").await.unwrap();
        assert_eq!(session.get_metadata().message_count, 3);
    }

    #[tokio::test]
    async fn test_save_and_load_context() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let mut session = Session::new(config, fs).await.unwrap();

        session.append_message("system", "You are helpful").await.unwrap();
        session.append_message("user", "question").await.unwrap();
        session.append_message("assistant", "answer").await.unwrap();

        let history = session.get_history().await.unwrap();

        // Save a context snapshot (simulating compaction)
        let snapshot = vec![
            SessionMessage::new("system", "You are helpful"),
            SessionMessage::new("assistant", "Compacted summary of conversation"),
        ];
        session.save_context(&snapshot).await.unwrap();

        // Load it back
        let loaded = session.load_context().await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, "system");
        assert_eq!(loaded[0].content, "You are helpful");
        assert_eq!(loaded[1].role, "assistant");
        assert_eq!(loaded[1].content, "Compacted summary of conversation");

        // History is still intact (JSONL log unaffected by context snapshot)
        let full = session.get_history().await.unwrap();
        assert_eq!(full.len(), 3);
    }

    #[tokio::test]
    async fn test_load_context_empty_when_no_snapshot() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let session = Session::new(config, fs).await.unwrap();

        let ctx = session.load_context().await.unwrap();
        assert!(ctx.is_empty());
    }

    #[tokio::test]
    async fn test_get_history_empty_for_new_session() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let session = Session::new(config, fs).await.unwrap();

        let history = session.get_history().await.unwrap();
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");

        // Create session and add messages
        {
            let mut session = Session::new(config.clone(), fs.clone()).await.unwrap();
            session.append_message("user", "first message").await.unwrap();
            session.append_message("assistant", "first reply").await.unwrap();
        }

        // Resume the same session from the same VFS
        {
            let session = Session::new(config, fs).await.unwrap();
            let meta = session.get_metadata();
            assert_eq!(meta.message_count, 2);

            let history = session.get_history().await.unwrap();
            assert_eq!(history.len(), 2);
            assert_eq!(history[0].content, "first message");
            assert_eq!(history[1].content, "first reply");
        }
    }

    #[tokio::test]
    async fn test_session_with_workspace_root() {
        let fs = test_fs();
        let config = SessionConfig::new("sess-42", "user-7", "workspaces/w1");
        let mut session = Session::new(config, fs.clone()).await.unwrap();

        session.append_message("user", "hello world").await.unwrap();

        // Verify the file is stored under the workspace root
        let log_bytes = fs.read("workspaces/w1/sessions/sess-42/log.jsonl").await.unwrap();
        let log_text = String::from_utf8_lossy(&log_bytes);
        assert!(log_text.contains("hello world"));

        // Metadata also under the workspace root
        let meta_bytes = fs.read("workspaces/w1/sessions/sess-42/metadata.json").await.unwrap();
        let meta: SessionMetadata = serde_json::from_slice(&meta_bytes).unwrap();
        assert_eq!(meta.session_id, "sess-42");
        assert_eq!(meta.message_count, 1);
    }

    #[tokio::test]
    async fn test_context_snapshot_overwrite() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let session = Session::new(config, fs).await.unwrap();

        // Save first snapshot
        let snap1 = vec![SessionMessage::new("system", "v1 context")];
        session.save_context(&snap1).await.unwrap();

        // Overwrite with second snapshot
        let snap2 = vec![
            SessionMessage::new("system", "v2 context"),
            SessionMessage::new("assistant", "updated"),
        ];
        session.save_context(&snap2).await.unwrap();

        let loaded = session.load_context().await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "v2 context");
    }

    #[tokio::test]
    async fn test_last_active_updates() {
        let fs = test_fs();
        let config = SessionConfig::new("s1", "u1", "");
        let mut session = Session::new(config, fs).await.unwrap();

        let initial = session.get_metadata().last_active;

        // Small delay to ensure timestamp differs
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        session.append_message("user", "ping").await.unwrap();
        let updated = session.get_metadata().last_active;

        assert!(updated >= initial);
        assert_eq!(session.get_metadata().message_count, 1);
    }
}
