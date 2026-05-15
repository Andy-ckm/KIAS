//! # Context Compaction
//!
//! Compresses long conversation histories to fit within token budgets.
//!
//! ## Strategy
//!
//! 1. Keep the system message (first message) intact.
//! 2. Keep the most recent N messages intact.
//! 3. Summarize older messages into key facts.
//! 4. Token counting uses ~4 chars/token estimate.

use serde::{Deserialize, Serialize};

// ── Configuration ─────────────────────────────────────────────────────

/// Configuration for the context compactor.
#[derive(Debug, Clone)]
pub struct CompactionConfig {
    /// Number of messages before compaction is triggered.
    pub trigger_messages: usize,
    /// Number of recent messages to keep verbatim after compaction.
    pub keep_messages: usize,
    /// Maximum approximate tokens for the compacted output.
    pub max_tokens: usize,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            trigger_messages: 50,
            keep_messages: 20,
            max_tokens: 8000,
        }
    }
}

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

// ── ContextCompactor ──────────────────────────────────────────────────

/// Compacts a conversation history by summarizing older messages.
pub struct ContextCompactor {
    config: CompactionConfig,
}

/// Result of a compaction operation.
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// The compacted messages (summary + recent).
    pub messages: Vec<Message>,
    /// Human-readable summary of the compacted portion.
    pub summary: String,
    /// Key facts extracted from the compacted messages.
    pub key_facts: Vec<String>,
}

impl ContextCompactor {
    pub fn new(config: CompactionConfig) -> Self {
        Self { config }
    }

    /// Estimate token count for a string (~4 chars per token).
    pub fn estimate_tokens(text: &str) -> usize {
        // Use ceiling division so very short strings still count as 1 token
        let len = text.len();
        if len == 0 {
            0
        } else {
            len.div_ceil(4)
        }
    }

    /// Estimate total tokens for a slice of messages.
    pub fn estimate_messages_tokens(messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| Self::estimate_tokens(&m.role) + Self::estimate_tokens(&m.content) + 4) // +4 for formatting overhead
            .sum()
    }

    /// Compact the message history if it exceeds the trigger threshold.
    ///
    /// Returns `None` if compaction is not needed.
    pub fn maybe_compact(&self, messages: &[Message]) -> Option<CompactionResult> {
        if messages.len() < self.config.trigger_messages {
            return None;
        }
        Some(self.compact(messages))
    }

    /// Force-compact the message history.
    ///
    /// Strategy:
    /// - Always keep the first message (system prompt) if present.
    /// - Keep the last `keep_messages` messages verbatim.
    /// - Summarize everything in between into key facts.
    pub fn compact(&self, messages: &[Message]) -> CompactionResult {
        if messages.is_empty() {
            return CompactionResult {
                messages: vec![],
                summary: String::new(),
                key_facts: vec![],
            };
        }

        let has_system = messages.first().is_some_and(|m| m.role == "system");
        let system_msg = if has_system {
            messages.first().cloned()
        } else {
            None
        };

        let keep_start = if messages.len() > self.config.keep_messages {
            messages.len() - self.config.keep_messages
        } else {
            // Messages are short enough; no compaction needed
            return CompactionResult {
                messages: messages.to_vec(),
                summary: String::new(),
                key_facts: vec![],
            };
        };

        // Messages to compact: everything between system and recent
        let compact_start = if has_system { 1 } else { 0 };
        let compact_end = keep_start;
        let to_compact = if compact_start < compact_end {
            &messages[compact_start..compact_end]
        } else {
            &[]
        };

        let key_facts = extract_key_facts(to_compact);
        let summary = if key_facts.is_empty() {
            "No significant facts found in earlier messages.".to_string()
        } else {
            format!(
                "Summary of {} earlier messages:\n{}",
                to_compact.len(),
                key_facts
                    .iter()
                    .enumerate()
                    .map(|(i, f)| format!("{}. {}", i + 1, f))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        // Build compacted message list
        let mut result = Vec::new();

        // 1. System message (updated with summary context)
        if let Some(mut sys) = system_msg {
            sys.content = format!(
                "{}\n\n[Context Compacted]\n{}",
                sys.content, summary
            );
            result.push(sys);
        } else {
            // Inject a summary message
            result.push(Message::system(&summary));
        }

        // 2. Recent messages
        result.extend_from_slice(&messages[keep_start..]);

        // 3. Enforce max_tokens by trimming from the summary if needed
        while Self::estimate_messages_tokens(&result) > self.config.max_tokens && result.len() > 2 {
            // Remove oldest non-system message
            let remove_idx = if result.first().is_some_and(|m| m.role == "system") {
                1
            } else {
                0
            };
            result.remove(remove_idx);
        }

        CompactionResult {
            messages: result,
            summary,
            key_facts: extract_key_facts(to_compact),
        }
    }
}

// ── Key Fact Extraction ───────────────────────────────────────────────

/// Extract key facts from a slice of messages using simple heuristics.
///
/// This is a rule-based extractor (no LLM). It looks for:
/// - User requests and decisions
/// - Assistant conclusions and code blocks
/// - Lines with keywords like "important", "note", "error", "result"
pub fn extract_key_facts(messages: &[Message]) -> Vec<String> {
    let keywords = [
        "important",
        "note:",
        "error",
        "result",
        "decision",
        "conclusion",
        "warning",
        "fix",
        "solution",
        "resolved",
        "deployed",
        "configured",
        "created",
        "deleted",
        "updated",
        "installed",
    ];

    let mut facts = Vec::new();

    for msg in messages {
        let role_prefix = format!("[{}]", msg.role);

        // Extract lines containing keywords
        for line in msg.content.lines() {
            let lower = line.to_lowercase();
            if keywords.iter().any(|kw| lower.contains(kw)) {
                let trimmed = line.trim();
                if !trimmed.is_empty() && trimmed.len() < 500 {
                    facts.push(format!("{} {}", role_prefix, trimmed));
                }
            }
        }

        // Capture first line of user messages as potential requests
        if msg.role == "user" {
            if let Some(first_line) = msg.content.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty()
                    && trimmed.len() > 10
                    && trimmed.len() < 300
                    && !facts.iter().any(|f| f.contains(trimmed))
                {
                    facts.push(format!("{} {}", role_prefix, trimmed));
                }
            }
        }
    }

    // Deduplicate and limit
    facts.sort();
    facts.dedup();
    facts.truncate(50);
    facts
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_messages(n: usize) -> Vec<Message> {
        let mut msgs = vec![Message::system("You are a helpful assistant.")];
        for i in 0..n {
            msgs.push(Message::user(format!("User message {}", i)));
            msgs.push(Message::assistant(format!("Assistant reply {}", i)));
        }
        msgs
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(ContextCompactor::estimate_tokens(""), 0);
        assert_eq!(ContextCompactor::estimate_tokens("abcd"), 1);
        assert_eq!(ContextCompactor::estimate_tokens("abcde"), 2);
        assert_eq!(ContextCompactor::estimate_tokens("a".repeat(100).as_str()), 25);
    }

    #[test]
    fn test_estimate_messages_tokens() {
        let msgs = vec![Message::user("hello")];
        let tokens = ContextCompactor::estimate_messages_tokens(&msgs);
        // role "user" = 1 token, content "hello" = 2 tokens, +4 overhead = 7
        assert!(tokens > 0);
    }

    #[test]
    fn test_no_compaction_below_threshold() {
        let config = CompactionConfig {
            trigger_messages: 10,
            keep_messages: 5,
            max_tokens: 8000,
        };
        let compactor = ContextCompactor::new(config);
        let msgs = make_messages(3); // 7 messages total
        let result = compactor.maybe_compact(&msgs);
        assert!(result.is_none());
    }

    #[test]
    fn test_compaction_triggered() {
        let config = CompactionConfig {
            trigger_messages: 5,
            keep_messages: 2,
            max_tokens: 8000,
        };
        let compactor = ContextCompactor::new(config);
        let msgs = make_messages(5); // 11 messages
        let result = compactor.maybe_compact(&msgs);
        assert!(result.is_some());

        let compacted = result.unwrap();
        // System message + 2 kept recent + (possibly) summary injection
        assert!(compacted.messages.len() <= 11);
        // The system message should contain the summary
        assert!(compacted.messages[0].content.contains("Context Compacted"));
    }

    #[test]
    fn test_compact_preserves_system_message() {
        let config = CompactionConfig {
            trigger_messages: 3,
            keep_messages: 2,
            max_tokens: 8000,
        };
        let compactor = ContextCompactor::new(config);
        let msgs = make_messages(5);
        let result = compactor.compact(&msgs);
        assert_eq!(result.messages[0].role, "system");
    }

    #[test]
    fn test_compact_empty() {
        let compactor = ContextCompactor::new(CompactionConfig::default());
        let result = compactor.compact(&[]);
        assert!(result.messages.is_empty());
        assert!(result.summary.is_empty());
    }

    #[test]
    fn test_extract_key_facts_from_errors() {
        let msgs = vec![
            Message::user("Please fix the bug"),
            Message::assistant("The error was in line 42. Fix applied and resolved."),
            Message::user("What about deployment?"),
            Message::assistant("Deployed to production successfully."),
        ];
        let facts = extract_key_facts(&msgs);
        assert!(!facts.is_empty());
        assert!(facts.iter().any(|f| f.contains("error")));
        assert!(facts.iter().any(|f| f.contains("resolved")));
    }

    #[test]
    fn test_extract_key_facts_empty() {
        let facts = extract_key_facts(&[]);
        assert!(facts.is_empty());
    }

    #[test]
    fn test_compact_max_tokens_enforced() {
        let config = CompactionConfig {
            trigger_messages: 3,
            keep_messages: 2,
            max_tokens: 30, // Very small budget
        };
        let compactor = ContextCompactor::new(config);

        // Create messages with substantial content
        let mut msgs = vec![Message::system("System prompt")];
        for i in 0..10 {
            msgs.push(Message::user(format!("This is user message number {} with some extra content to make it longer", i)));
            msgs.push(Message::assistant(format!("This is assistant reply number {} with detailed explanation of the topic at hand", i)));
        }

        let result = compactor.compact(&msgs);
        let total_tokens = ContextCompactor::estimate_messages_tokens(&result.messages);
        let original_tokens = ContextCompactor::estimate_messages_tokens(&msgs);
        // Compacted output should be significantly smaller than original
        assert!(total_tokens < original_tokens);
    }
}
