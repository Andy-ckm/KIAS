//! Agent 上下文管理 — 增强版
//!
//! 集成七层记忆架构：
//! - L1: 工具结果存储
//! - L3: 会话记忆
//! - L6: 做梦巩固

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent 上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    /// 项目名称
    pub project: String,
    /// 工作目录
    pub working_dir: String,
    /// Git 分支
    pub branch: Option<String>,
    /// 上下文文件内容 (kias.md)
    pub context_file: Option<String>,
    /// 对话历史
    pub conversation_history: Vec<ConversationTurn>,
    /// 项目知识
    pub project_knowledge: Vec<String>,
    /// 会话记忆 (L3)
    pub session_memory: SessionMemory,
    /// 工具结果缓存 (L1)
    pub tool_results: HashMap<String, ToolResultEntry>,
}

/// 对话轮次
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub tool_calls: Option<Vec<ToolCallRecord>>,
}

/// 工具调用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: String,
    pub success: bool,
}

/// 会话记忆 (L3)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMemory {
    /// 关键发现
    pub key_findings: Vec<String>,
    /// 引用的文档
    pub referenced_docs: Vec<String>,
    /// 工具调用统计
    pub tool_stats: ToolStats,
}

/// 工具调用统计
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolStats {
    pub search_calls: usize,
    pub find_calls: usize,
    pub open_calls: usize,
    pub summarize_calls: usize,
    pub total_tokens: usize,
}

/// 工具结果条目 (L1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEntry {
    pub id: String,
    pub tool_name: String,
    pub preview: String,
    pub full_content: String,
    pub stored_at: String,
}

impl AgentContext {
    pub fn new(project: &str, working_dir: &str) -> Self {
        Self {
            project: project.to_string(),
            working_dir: working_dir.to_string(),
            branch: None,
            context_file: None,
            conversation_history: Vec::new(),
            project_knowledge: Vec::new(),
            session_memory: SessionMemory {
                key_findings: Vec::new(),
                referenced_docs: Vec::new(),
                tool_stats: ToolStats::default(),
            },
            tool_results: HashMap::new(),
        }
    }

    /// 添加对话轮次
    pub fn add_turn(&mut self, role: &str, content: &str) {
        self.conversation_history.push(ConversationTurn {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            tool_calls: None,
        });
    }

    /// 记录工具调用
    pub fn record_tool_call(
        &mut self,
        name: &str,
        args: serde_json::Value,
        result: &str,
        success: bool,
    ) {
        if let Some(last) = self.conversation_history.last_mut() {
            if last.tool_calls.is_none() {
                last.tool_calls = Some(Vec::new());
            }
            last.tool_calls.as_mut().unwrap().push(ToolCallRecord {
                name: name.to_string(),
                arguments: args,
                result: result.to_string(),
                success,
            });
        }

        // 更新统计
        match name {
            "search" => self.session_memory.tool_stats.search_calls += 1,
            "find" => self.session_memory.tool_stats.find_calls += 1,
            "open" => self.session_memory.tool_stats.open_calls += 1,
            "summarize" => self.session_memory.tool_stats.summarize_calls += 1,
            _ => {}
        }
    }

    /// 存储工具结果 (L1)
    pub fn store_tool_result(&mut self, id: &str, tool_name: &str, content: &str) -> String {
        let preview = if content.len() > 2048 {
            format!("{}...", &content[..2048])
        } else {
            content.to_string()
        };

        self.tool_results.insert(
            id.to_string(),
            ToolResultEntry {
                id: id.to_string(),
                tool_name: tool_name.to_string(),
                preview: preview.clone(),
                full_content: content.to_string(),
                stored_at: chrono::Utc::now().to_rfc3339(),
            },
        );

        preview
    }

    /// 添加关键发现 (L3)
    pub fn add_finding(&mut self, finding: &str, doc_id: Option<&str>) {
        self.session_memory.key_findings.push(finding.to_string());
        if let Some(doc) = doc_id {
            if !self
                .session_memory
                .referenced_docs
                .contains(&doc.to_string())
            {
                self.session_memory.referenced_docs.push(doc.to_string());
            }
        }
    }

    /// 生成会话摘要
    pub fn generate_summary(&self) -> String {
        format!(
            "## Session Summary\n\n### Project: {}\n\n### Key Findings\n{}\n\n### Referenced Docs\n{}\n\n### Tool Stats\n- Search: {}, Find: {}, Open: {}, Summarize: {}\n- Total tokens: {}",
            self.project,
            self.session_memory.key_findings.join("\n- "),
            self.session_memory.referenced_docs.join(", "),
            self.session_memory.tool_stats.search_calls,
            self.session_memory.tool_stats.find_calls,
            self.session_memory.tool_stats.open_calls,
            self.session_memory.tool_stats.summarize_calls,
            self.session_memory.tool_stats.total_tokens
        )
    }

    /// 检查是否需要压缩
    pub fn needs_compaction(&self, token_threshold: usize) -> bool {
        let estimated_tokens: usize = self
            .conversation_history
            .iter()
            .map(|t| t.content.len() / 4)
            .sum();
        estimated_tokens > token_threshold && !self.session_memory.key_findings.is_empty()
    }
}

impl AgentContext {
    /// 获取系统提示
    pub fn get_system_prompt(&self, base_prompt: &str) -> String {
        let mut prompt = base_prompt.to_string();

        // 添加项目信息
        prompt.push_str(&format!("\n\n## Project: {}", self.project));
        prompt.push_str(&format!("\nWorking Directory: {}", self.working_dir));

        if let Some(branch) = &self.branch {
            prompt.push_str(&format!("\nGit Branch: {}", branch));
        }

        // 添加上下文文件内容
        if let Some(ctx_file) = &self.context_file {
            prompt.push_str(&format!("\n\n## Context File\n{}", ctx_file));
        }

        // 添加项目知识
        if !self.project_knowledge.is_empty() {
            prompt.push_str("\n\n## Project Knowledge");
            for knowledge in &self.project_knowledge {
                prompt.push_str(&format!("\n- {}", knowledge));
            }
        }

        // 添加会话记忆摘要
        if !self.session_memory.key_findings.is_empty() {
            prompt.push_str("\n\n## Session Memory");
            prompt.push_str(&format!(
                "\nKey Findings: {}",
                self.session_memory.key_findings.join(", ")
            ));
        }

        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = AgentContext::new("kias", "/workspace/kias");
        assert_eq!(ctx.project, "kias");
        assert_eq!(ctx.working_dir, "/workspace/kias");
        assert!(ctx.conversation_history.is_empty());
    }

    #[test]
    fn test_add_turn() {
        let mut ctx = AgentContext::new("test", "/tmp");
        ctx.add_turn("user", "hello");
        ctx.add_turn("assistant", "hi there");
        assert_eq!(ctx.conversation_history.len(), 2);
        assert_eq!(ctx.conversation_history[0].role, "user");
        assert_eq!(ctx.conversation_history[1].role, "assistant");
    }

    #[test]
    fn test_record_tool_call() {
        let mut ctx = AgentContext::new("test", "/tmp");
        ctx.add_turn("user", "search for something");
        ctx.record_tool_call(
            "search",
            serde_json::json!({"query": "test"}),
            "found 5 results",
            true,
        );

        assert_eq!(ctx.session_memory.tool_stats.search_calls, 1);
        assert!(ctx.conversation_history[0].tool_calls.is_some());
    }

    #[test]
    fn test_store_tool_result() {
        let mut ctx = AgentContext::new("test", "/tmp");
        let content = "x".repeat(5000);
        let preview = ctx.store_tool_result("r1", "search", &content);

        assert!(preview.len() < content.len());
        assert!(preview.ends_with("..."));
        assert_eq!(ctx.tool_results.len(), 1);
    }

    #[test]
    fn test_store_short_tool_result() {
        let mut ctx = AgentContext::new("test", "/tmp");
        let content = "short content";
        let preview = ctx.store_tool_result("r1", "search", content);

        assert_eq!(preview, content);
    }

    #[test]
    fn test_add_finding() {
        let mut ctx = AgentContext::new("test", "/tmp");
        ctx.add_finding("Found a bug in auth", Some("doc1"));
        ctx.add_finding("Fixed null pointer", Some("doc2"));

        assert_eq!(ctx.session_memory.key_findings.len(), 2);
        assert_eq!(ctx.session_memory.referenced_docs.len(), 2);
    }

    #[test]
    fn test_add_finding_dedup_docs() {
        let mut ctx = AgentContext::new("test", "/tmp");
        ctx.add_finding("Finding 1", Some("doc1"));
        ctx.add_finding("Finding 2", Some("doc1"));

        assert_eq!(ctx.session_memory.key_findings.len(), 2);
        assert_eq!(ctx.session_memory.referenced_docs.len(), 1);
    }

    #[test]
    fn test_generate_summary() {
        let mut ctx = AgentContext::new("kias", "/workspace/kias");
        ctx.add_turn("user", "test query");
        ctx.add_finding("Found issue", Some("doc1"));

        let summary = ctx.generate_summary();
        assert!(summary.contains("kias"));
        assert!(summary.contains("Found issue"));
    }

    #[test]
    fn test_needs_compaction() {
        let mut ctx = AgentContext::new("test", "/tmp");

        // 不需要压缩（没有findings）
        for i in 0..100 {
            ctx.add_turn("user", &format!("message {}", i));
        }
        // 100 messages = 200 estimated tokens; threshold=500 → no compaction
        assert!(!ctx.needs_compaction(500));

        // 添加findings后需要压缩
        ctx.add_finding("some finding", None);
        assert!(ctx.needs_compaction(199)); // 200 > 199
                                            // Still false if threshold is very high (strict > comparison)
        assert!(!ctx.needs_compaction(500));
    }

    #[test]
    fn test_tool_stats_accumulation() {
        let mut ctx = AgentContext::new("test", "/tmp");
        ctx.add_turn("user", "query");

        ctx.record_tool_call("search", serde_json::json!({}), "r1", true);
        ctx.record_tool_call("search", serde_json::json!({}), "r2", true);
        ctx.record_tool_call("find", serde_json::json!({}), "r3", true);

        assert_eq!(ctx.session_memory.tool_stats.search_calls, 2);
        assert_eq!(ctx.session_memory.tool_stats.find_calls, 1);
        assert_eq!(ctx.session_memory.tool_stats.open_calls, 0);
    }
}
