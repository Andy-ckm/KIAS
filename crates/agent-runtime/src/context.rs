//! Agent 上下文管理

use serde::{Deserialize, Serialize};

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

impl AgentContext {
    pub fn new(project: &str, working_dir: &str) -> Self {
        Self {
            project: project.to_string(),
            working_dir: working_dir.to_string(),
            branch: None,
            context_file: None,
            conversation_history: Vec::new(),
            project_knowledge: Vec::new(),
        }
    }

    /// 加载上下文文件 (kias.md)
    pub async fn load_context_file(&mut self) {
        let path = format!("{}/kias.md", self.working_dir);
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            self.context_file = Some(content);
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

    /// 获取系统提示
    pub fn get_system_prompt(&self, base_prompt: &str) -> String {
        let mut prompt = base_prompt.to_string();

        // 添加项目上下文
        prompt.push_str(&format!(
            "\n\n## 项目信息\n- 项目: {}\n- 工作目录: {}",
            self.project, self.working_dir
        ));

        if let Some(branch) = &self.branch {
            prompt.push_str(&format!("\n- Git 分支: {}", branch));
        }

        // 添加上下文文件内容
        if let Some(context) = &self.context_file {
            prompt.push_str(&format!("\n\n## 项目上下文\n{}", context));
        }

        // 添加对话历史摘要
        if !self.conversation_history.is_empty() {
            prompt.push_str("\n\n## 对话历史\n");
            for turn in self.conversation_history.iter().rev().take(5) {
                prompt.push_str(&format!(
                    "{}: {}\n",
                    turn.role,
                    &turn.content[..turn.content.len().min(200)]
                ));
            }
        }

        prompt
    }
}
