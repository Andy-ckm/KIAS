use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 自主级别（借鉴 Codex CLI 三模式）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AutonomyLevel {
    /// 建议模式：仅提供建议，不执行任何操作
    Suggest,
    /// 自动编辑模式：自动修改文件但不执行命令
    AutoEdit,
    /// 完全自主模式：完全自主执行包括命令运行
    FullAuto,
}

/// 自主度梯度配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomyLadder {
    pub current_level: AutonomyLevel,
    pub tool_overrides: std::collections::HashMap<String, AutonomyLevel>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Default for AutonomyLadder {
    fn default() -> Self {
        Self::new()
    }
}

impl AutonomyLadder {
    pub fn new() -> Self {
        Self {
            current_level: AutonomyLevel::Suggest,
            tool_overrides: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// 设置全局自主级别
    pub fn set_level(&mut self, level: AutonomyLevel) {
        self.current_level = level;
        self.updated_at = Utc::now();
    }

    /// 为单个工具设置自主级别
    pub fn set_tool_level(&mut self, tool: &str, level: AutonomyLevel) {
        self.tool_overrides.insert(tool.to_string(), level);
        self.updated_at = Utc::now();
    }

    /// 获取工具的自主级别
    pub fn get_tool_level(&self, tool: &str) -> &AutonomyLevel {
        self.tool_overrides.get(tool).unwrap_or(&self.current_level)
    }

    /// 检查工具是否可以自动执行
    pub fn can_auto_execute(&self, tool: &str) -> bool {
        matches!(self.get_tool_level(tool), AutonomyLevel::FullAuto)
    }

    /// 检查工具是否可以自动编辑
    pub fn can_auto_edit(&self, tool: &str) -> bool {
        matches!(
            self.get_tool_level(tool),
            AutonomyLevel::AutoEdit | AutonomyLevel::FullAuto
        )
    }
}
