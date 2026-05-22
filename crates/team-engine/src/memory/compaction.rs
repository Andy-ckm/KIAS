//! # Conversation History Compaction
//!
//! Compresses long conversation histories to fit within token budgets.
//!
//! ## Strategies
//!
//! - **SlidingWindow**: Keeps system prompt + N most recent messages
//! - **SummarizeOld**: Summarizes old messages into a template-based summary,
//!   keeping only recent messages verbatim
//!
//! ## Features
//!
//! - Token counting via ~4 chars/token heuristic
//! - Pre/post snapshot backup with rollback support
//! - No LLM calls — all summarization is template-based

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Message ───────────────────────────────────────────────────────────

/// A single conversation message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }
}

// ── Token Counter ─────────────────────────────────────────────────────

/// Estimate token count for a string (~4 chars per token).
pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.len().div_ceil(4)
    }
}

/// Estimate total tokens for a slice of messages (including formatting overhead).
pub fn estimate_messages_tokens(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| estimate_tokens(&m.role) + estimate_tokens(&m.content) + 4) // +4 overhead
        .sum()
}

/// Determine whether compaction should be triggered based on token budget.
pub fn should_compact(messages: &[Message], max_tokens: usize) -> bool {
    estimate_messages_tokens(messages) > max_tokens
}

// ── CompactedHistory ──────────────────────────────────────────────────

/// Result of a compaction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactedHistory {
    /// The compacted messages.
    pub messages: Vec<Message>,
    /// Optional summary of what was removed (None if nothing was compacted).
    pub summary: Option<String>,
    /// Number of messages before compaction.
    pub original_count: usize,
    /// Number of messages after compaction.
    pub compacted_count: usize,
}

impl CompactedHistory {
    /// Returns true if compaction actually changed the message list.
    pub fn was_compacted(&self) -> bool {
        self.original_count != self.compacted_count || self.summary.is_some()
    }
}

// ── Snapshot (for rollback) ──────────────────────────────────────────

/// A point-in-time snapshot of conversation history for rollback support.
#[derive(Debug, Clone)]
pub struct HistorySnapshot {
    /// The messages at snapshot time.
    pub messages: Vec<Message>,
    /// When the snapshot was taken.
    pub timestamp: DateTime<Utc>,
    /// Optional label (e.g., "pre-compaction").
    pub label: Option<String>,
}

// ── CompactionStrategy trait ──────────────────────────────────────────

/// Trait for conversation history compaction strategies.
#[async_trait]
pub trait CompactionStrategy: Send + Sync {
    /// Compact the given messages and return a `CompactedHistory`.
    async fn compact(&self, messages: &[Message]) -> CompactedHistory;
}

// ── SlidingWindow strategy ───────────────────────────────────────────

/// Keeps the system prompt (if any) plus the N most recent messages.
pub struct SlidingWindow {
    /// Number of recent messages to keep verbatim.
    pub keep_recent: usize,
}

impl SlidingWindow {
    pub fn new(keep_recent: usize) -> Self {
        Self { keep_recent }
    }
}

#[async_trait]
impl CompactionStrategy for SlidingWindow {
    async fn compact(&self, messages: &[Message]) -> CompactedHistory {
        let original_count = messages.len();

        if messages.is_empty() {
            return CompactedHistory {
                messages: vec![],
                summary: None,
                original_count: 0,
                compacted_count: 0,
            };
        }

        let has_system = messages[0].role == "system";
        let system_offset: usize = if has_system { 1 } else { 0 };

        // If messages fit within the window, return as-is
        if messages.len() <= self.keep_recent + system_offset {
            return CompactedHistory {
                messages: messages.to_vec(),
                summary: None,
                original_count,
                compacted_count: original_count,
            };
        }

        let mut result = Vec::new();

        // 1. Preserve system prompt
        if has_system {
            result.push(messages[0].clone());
        }

        // 2. Keep the most recent N messages
        let recent_start = messages.len().saturating_sub(self.keep_recent);
        result.extend_from_slice(&messages[recent_start..]);

        let result_count = result.len();
        let dropped = original_count - result_count;
        let summary = format!(
            "SlidingWindow: kept system prompt + {} recent messages, dropped {} older messages",
            self.keep_recent, dropped
        );

        CompactedHistory {
            messages: result,
            summary: Some(summary),
            original_count,
            compacted_count: result_count,
        }
    }
}

// ── SummarizeOld strategy ────────────────────────────────────────────

/// Summarizes old messages into a single template-based summary message,
/// keeping the system prompt and most recent messages verbatim.
///
/// No LLM is called — the summary is built from message metadata and
/// user message first lines.
pub struct SummarizeOld {
    /// Number of recent messages to keep verbatim.
    pub keep_recent: usize,
}

impl SummarizeOld {
    pub fn new(keep_recent: usize) -> Self {
        Self { keep_recent }
    }
}

#[async_trait]
impl CompactionStrategy for SummarizeOld {
    async fn compact(&self, messages: &[Message]) -> CompactedHistory {
        let original_count = messages.len();

        if messages.is_empty() {
            return CompactedHistory {
                messages: vec![],
                summary: None,
                original_count: 0,
                compacted_count: 0,
            };
        }

        let has_system = messages[0].role == "system";
        let system_offset: usize = if has_system { 1 } else { 0 };

        // If messages fit within the window, return as-is
        if messages.len() <= self.keep_recent + system_offset {
            return CompactedHistory {
                messages: messages.to_vec(),
                summary: None,
                original_count,
                compacted_count: original_count,
            };
        }

        let recent_start = messages.len().saturating_sub(self.keep_recent);

        // Old messages are between system prompt and recent window
        let old_messages = &messages[system_offset..recent_start];
        let summary_text = build_template_summary(old_messages);

        let mut result = Vec::new();

        // 1. System prompt (appended with summary context)
        if has_system {
            let mut sys = messages[0].clone();
            sys.content = format!(
                "{}\n\n[Conversation Compacted]\n{}",
                sys.content, summary_text
            );
            result.push(sys);
        } else {
            // Inject a synthetic summary message
            result.push(Message::system(format!(
                "[Conversation Compacted]\n{}",
                summary_text
            )));
        }

        // 2. Recent messages
        result.extend_from_slice(&messages[recent_start..]);
        let result_count = result.len();

        CompactedHistory {
            messages: result,
            summary: Some(summary_text),
            original_count,
            compacted_count: result_count,
        }
    }
}

/// Build a template-based summary from a slice of messages (no LLM).
fn build_template_summary(messages: &[Message]) -> String {
    let user_count = messages.iter().filter(|m| m.role == "user").count();
    let assistant_count = messages.iter().filter(|m| m.role == "assistant").count();
    let system_count = messages.iter().filter(|m| m.role == "system").count();

    // Extract topics from user message first lines
    let mut topics: Vec<String> = Vec::new();
    for msg in messages {
        if msg.role == "user" {
            if let Some(first_line) = msg.content.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() && trimmed.len() > 5 && trimmed.len() < 200 {
                    topics.push(trimmed.to_string());
                }
            }
        }
    }
    topics.dedup();
    topics.truncate(5);

    // Extract key actions from assistant messages
    let mut actions: Vec<String> = Vec::new();
    let action_keywords = [
        "created",
        "deleted",
        "updated",
        "installed",
        "configured",
        "deployed",
        "fixed",
        "resolved",
        "added",
        "removed",
        "implemented",
        "refactored",
    ];
    for msg in messages {
        if msg.role == "assistant" {
            for line in msg.content.lines() {
                let lower = line.to_lowercase();
                if action_keywords.iter().any(|kw| lower.contains(kw)) {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && trimmed.len() < 300 {
                        actions.push(trimmed.to_string());
                    }
                }
            }
        }
    }
    actions.dedup();
    actions.truncate(5);

    let mut summary = format!(
        "Summary of {} earlier messages ({} user, {} assistant, {} system)",
        messages.len(),
        user_count,
        assistant_count,
        system_count,
    );

    if !topics.is_empty() {
        summary.push_str("\n\nTopics discussed:");
        for topic in &topics {
            summary.push_str(&format!("\n- {}", topic));
        }
    }

    if !actions.is_empty() {
        summary.push_str("\n\nKey actions:");
        for action in &actions {
            summary.push_str(&format!("\n- {}", action));
        }
    }

    summary
}

// ── CompactionManager (snapshot + rollback) ───────────────────────────

/// Manages compaction with snapshot-based rollback support.
///
/// Before each compaction, a snapshot is saved. If the compacted result
/// is unsatisfactory, `rollback()` restores the most recent snapshot.
pub struct CompactionManager {
    strategy: Box<dyn CompactionStrategy>,
    snapshots: Vec<HistorySnapshot>,
    max_snapshots: usize,
}

impl CompactionManager {
    /// Create a new manager with the given strategy and snapshot limit.
    pub fn new(strategy: Box<dyn CompactionStrategy>, max_snapshots: usize) -> Self {
        Self {
            strategy,
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    /// Take a snapshot of the current messages (for later rollback).
    pub fn take_snapshot(&mut self, messages: &[Message], label: Option<String>) {
        let snapshot = HistorySnapshot {
            messages: messages.to_vec(),
            timestamp: Utc::now(),
            label,
        };
        self.snapshots.push(snapshot);

        // Evict oldest snapshots beyond the limit
        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }
    }

    /// Compact messages, automatically taking a pre-compaction snapshot.
    pub async fn compact(&mut self, messages: &[Message]) -> CompactedHistory {
        self.take_snapshot(messages, Some("pre-compaction".to_string()));
        self.strategy.compact(messages).await
    }

    /// Rollback to the most recent snapshot, returning the restored messages.
    ///
    /// Returns `None` if no snapshots are available.
    pub fn rollback(&mut self) -> Option<Vec<Message>> {
        self.snapshots.pop().map(|s| s.messages)
    }

    /// Number of available snapshots for rollback.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Clear all snapshots.
    pub fn clear_snapshots(&mut self) {
        self.snapshots.clear();
    }

    /// Get a reference to the most recent snapshot without consuming it.
    pub fn peek_snapshot(&self) -> Option<&HistorySnapshot> {
        self.snapshots.last()
    }

    /// Replace the current compaction strategy.
    pub fn set_strategy(&mut self, strategy: Box<dyn CompactionStrategy>) {
        self.strategy = strategy;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(n: usize) -> Vec<Message> {
        let mut msgs = vec![Message::system("You are a helpful assistant.")];
        for i in 0..n {
            msgs.push(Message::user(format!("User message number {}", i)));
            msgs.push(Message::assistant(format!("Assistant reply number {}", i)));
        }
        msgs
    }

    // -- Token counter tests --

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_tokens_basic() {
        assert_eq!(estimate_tokens("abcd"), 1); // 4 chars = 1 token
        assert_eq!(estimate_tokens("abcde"), 2); // 5 chars = 2 tokens
        assert_eq!(estimate_tokens("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_estimate_messages_tokens() {
        let msgs = vec![Message::user("hello")];
        let tokens = estimate_messages_tokens(&msgs);
        // role "user" ~1, content "hello" ~2, +4 overhead = ~7
        assert!(tokens > 0);
    }

    #[test]
    fn test_should_compact_true() {
        // Create messages that exceed a small budget
        let mut msgs = vec![Message::system("System prompt")];
        for i in 0..50 {
            msgs.push(Message::user(format!(
                "Long user message number {} with extra content",
                i
            )));
        }
        assert!(should_compact(&msgs, 100));
    }

    #[test]
    fn test_should_compact_false() {
        let msgs = vec![Message::user("hi")];
        assert!(!should_compact(&msgs, 10_000));
    }

    // -- SlidingWindow tests --

    #[tokio::test]
    async fn test_sliding_window_preserves_system() {
        let strategy = SlidingWindow::new(2);
        let msgs = make_messages(5); // 11 messages
        let result = strategy.compact(&msgs).await;

        // System prompt should be first
        assert_eq!(result.messages[0].role, "system");
        // Should have system + 2 recent = 3 messages
        assert_eq!(result.messages.len(), 3);
        assert!(result.was_compacted());
    }

    #[tokio::test]
    async fn test_sliding_window_no_compaction_when_within_budget() {
        let strategy = SlidingWindow::new(10);
        let msgs = make_messages(3); // 7 messages
        let result = strategy.compact(&msgs).await;

        // All messages fit within keep_recent + system
        assert_eq!(result.messages.len(), 7);
        assert!(!result.was_compacted());
    }

    #[tokio::test]
    async fn test_sliding_window_empty() {
        let strategy = SlidingWindow::new(5);
        let result = strategy.compact(&[]).await;
        assert!(result.messages.is_empty());
        assert!(!result.was_compacted());
    }

    #[tokio::test]
    async fn test_sliding_window_no_system_prompt() {
        let strategy = SlidingWindow::new(2);
        let msgs = vec![
            Message::user("first"),
            Message::assistant("second"),
            Message::user("third"),
            Message::assistant("fourth"),
            Message::user("fifth"),
        ];
        let result = strategy.compact(&msgs).await;

        // Should keep last 2 (no system prompt, so no system offset)
        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].content, "fourth");
        assert_eq!(result.messages[1].content, "fifth");
    }

    // -- SummarizeOld tests --

    #[tokio::test]
    async fn test_summarize_old_creates_summary() {
        let strategy = SummarizeOld::new(2);
        let msgs = make_messages(5); // 11 messages
        let result = strategy.compact(&msgs).await;

        // System prompt + 2 recent = 3 messages
        assert_eq!(result.messages.len(), 3);
        // System message should contain the summary
        assert!(result.messages[0]
            .content
            .contains("Conversation Compacted"));
        assert!(result.summary.is_some());
        assert!(result.was_compacted());
    }

    #[tokio::test]
    async fn test_summarize_old_no_compaction_when_within_budget() {
        let strategy = SummarizeOld::new(10);
        let msgs = make_messages(3); // 7 messages
        let result = strategy.compact(&msgs).await;

        assert_eq!(result.messages.len(), 7);
        assert!(!result.was_compacted());
    }

    #[tokio::test]
    async fn test_summarize_old_empty() {
        let strategy = SummarizeOld::new(5);
        let result = strategy.compact(&[]).await;
        assert!(result.messages.is_empty());
        assert!(!result.was_compacted());
    }

    #[tokio::test]
    async fn test_summarize_old_no_system_prompt() {
        let strategy = SummarizeOld::new(2);
        let msgs = vec![
            Message::user("Hello, how are you?"),
            Message::assistant("I'm fine, thanks!"),
            Message::user("What about the project?"),
            Message::assistant("The project is going well."),
            Message::user("Great, thanks!"),
        ];
        let result = strategy.compact(&msgs).await;

        // Should inject a system message with summary + 2 recent
        assert_eq!(result.messages.len(), 3);
        assert_eq!(result.messages[0].role, "system");
        assert!(result.messages[0]
            .content
            .contains("Conversation Compacted"));
        // Recent messages preserved
        assert_eq!(result.messages[1].content, "The project is going well.");
        assert_eq!(result.messages[2].content, "Great, thanks!");
    }

    #[tokio::test]
    async fn test_summarize_old_summary_contains_topics() {
        let strategy = SummarizeOld::new(1);
        let msgs = vec![
            Message::system("You are helpful"),
            Message::user("Tell me about Rust programming language"),
            Message::assistant("Rust is a systems programming language."),
            Message::user("What about memory safety?"),
            Message::assistant("Rust ensures memory safety through ownership."),
            Message::user("Thanks!"),
        ];
        let result = strategy.compact(&msgs).await;
        let summary = result.summary.unwrap();
        assert!(summary.contains("Rust programming language"));
    }

    // -- Template summary tests --

    #[test]
    fn test_build_template_summary() {
        let msgs = vec![
            Message::user("How to deploy?"),
            Message::assistant("Deployed to production successfully."),
            Message::user("What about tests?"),
            Message::assistant("All tests resolved and fixed."),
        ];
        let summary = build_template_summary(&msgs);
        assert!(summary.contains("2 user"));
        assert!(summary.contains("2 assistant"));
        assert!(summary.contains("How to deploy?"));
        assert!(summary.contains("Deployed"));
    }

    #[test]
    fn test_build_template_summary_empty() {
        let summary = build_template_summary(&[]);
        assert!(summary.contains("0 earlier messages"));
    }

    // -- CompactionManager tests --

    #[tokio::test]
    async fn test_manager_compact_and_rollback() {
        let strategy = SlidingWindow::new(2);
        let mut manager = CompactionManager::new(Box::new(strategy), 5);

        let msgs = make_messages(5); // 11 messages
        let result = manager.compact(&msgs).await;

        assert!(result.was_compacted());
        assert_eq!(manager.snapshot_count(), 1);

        // Rollback should restore original messages
        let restored = manager.rollback().unwrap();
        assert_eq!(restored.len(), 11);
        assert_eq!(restored[0].role, "system");
        assert_eq!(manager.snapshot_count(), 0);
    }

    #[tokio::test]
    async fn test_manager_rollback_empty() {
        let strategy = SlidingWindow::new(2);
        let mut manager = CompactionManager::new(Box::new(strategy), 5);

        // No snapshots yet
        assert!(manager.rollback().is_none());
    }

    #[tokio::test]
    async fn test_manager_snapshot_limit() {
        let strategy = SlidingWindow::new(100); // Won't actually compact
        let mut manager = CompactionManager::new(Box::new(strategy), 3);

        let msgs = make_messages(1);

        // Take 5 snapshots, but limit is 3
        for _ in 0..5 {
            manager.take_snapshot(&msgs, None);
        }

        assert_eq!(manager.snapshot_count(), 3);
    }

    #[tokio::test]
    async fn test_manager_clear_snapshots() {
        let strategy = SlidingWindow::new(2);
        let mut manager = CompactionManager::new(Box::new(strategy), 5);

        let msgs = make_messages(3);
        manager.take_snapshot(&msgs, None);
        manager.take_snapshot(&msgs, None);
        assert_eq!(manager.snapshot_count(), 2);

        manager.clear_snapshots();
        assert_eq!(manager.snapshot_count(), 0);
    }

    #[tokio::test]
    async fn test_manager_rollback_restores_correct_data() {
        let strategy = SummarizeOld::new(2);
        let mut manager = CompactionManager::new(Box::new(strategy), 5);

        let msgs = make_messages(5); // 11 messages
        let _result = manager.compact(&msgs).await;

        // Rollback
        let restored = manager.rollback().unwrap();
        assert_eq!(restored, msgs);
    }

    #[tokio::test]
    async fn test_manager_peek_snapshot() {
        let strategy = SlidingWindow::new(2);
        let mut manager = CompactionManager::new(Box::new(strategy), 5);

        let msgs = make_messages(3);
        manager.take_snapshot(&msgs, Some("test-snap".to_string()));

        let snap = manager.peek_snapshot().unwrap();
        assert_eq!(snap.label.as_deref(), Some("test-snap"));
        assert_eq!(snap.messages.len(), msgs.len());
    }

    #[tokio::test]
    async fn test_manager_set_strategy() {
        let window = SlidingWindow::new(2);
        let mut manager = CompactionManager::new(Box::new(window), 5);

        let msgs = make_messages(5);
        let r1 = manager.compact(&msgs).await;
        let _r1_count = r1.compacted_count;

        // Switch to summarize strategy
        manager.rollback(); // restore snapshot
        let summarize = SummarizeOld::new(2);
        manager.set_strategy(Box::new(summarize));

        let r2 = manager.compact(&msgs).await;

        // Both should compact, but summaries differ
        assert!(r1.summary.is_some());
        assert!(r2.summary.is_some());
        assert_ne!(r1.summary, r2.summary);
    }

    // -- CompactedHistory helper --

    #[test]
    fn test_compacted_history_was_compacted() {
        let h = CompactedHistory {
            messages: vec![],
            summary: None,
            original_count: 10,
            compacted_count: 10,
        };
        assert!(!h.was_compacted());

        let h2 = CompactedHistory {
            messages: vec![],
            summary: Some("summary".to_string()),
            original_count: 10,
            compacted_count: 5,
        };
        assert!(h2.was_compacted());
    }
}
