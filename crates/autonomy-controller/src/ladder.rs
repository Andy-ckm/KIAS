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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_level_is_suggest() {
        let ladder = AutonomyLadder::new();
        assert_eq!(ladder.current_level, AutonomyLevel::Suggest);
    }

    #[test]
    fn test_default_has_no_overrides() {
        let ladder = AutonomyLadder::new();
        assert!(ladder.tool_overrides.is_empty());
    }

    #[test]
    fn test_set_level() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::FullAuto);
        assert_eq!(ladder.current_level, AutonomyLevel::FullAuto);
    }

    #[test]
    fn test_set_tool_level_override() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_tool_level("shell", AutonomyLevel::FullAuto);
        assert_eq!(ladder.get_tool_level("shell"), &AutonomyLevel::FullAuto);
    }

    #[test]
    fn test_get_tool_level_falls_back_to_global() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::AutoEdit);
        assert_eq!(
            ladder.get_tool_level("unknown_tool"),
            &AutonomyLevel::AutoEdit
        );
    }

    #[test]
    fn test_get_tool_level_override_wins() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::Suggest);
        ladder.set_tool_level("read_file", AutonomyLevel::FullAuto);
        assert_eq!(ladder.get_tool_level("read_file"), &AutonomyLevel::FullAuto);
        // Other tools still use global
        assert_eq!(ladder.get_tool_level("write_file"), &AutonomyLevel::Suggest);
    }

    #[test]
    fn test_can_auto_execute_only_full_auto() {
        let mut ladder = AutonomyLadder::new();
        assert!(!ladder.can_auto_execute("shell"));

        ladder.set_level(AutonomyLevel::AutoEdit);
        assert!(!ladder.can_auto_execute("shell"));

        ladder.set_level(AutonomyLevel::FullAuto);
        assert!(ladder.can_auto_execute("shell"));
    }

    #[test]
    fn test_can_auto_execute_per_tool_override() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::Suggest);
        ladder.set_tool_level("read_file", AutonomyLevel::FullAuto);
        assert!(ladder.can_auto_execute("read_file"));
        assert!(!ladder.can_auto_execute("write_file"));
    }

    #[test]
    fn test_can_auto_edit_auto_edit_or_full_auto() {
        let mut ladder = AutonomyLadder::new();
        assert!(!ladder.can_auto_edit("editor"));

        ladder.set_level(AutonomyLevel::AutoEdit);
        assert!(ladder.can_auto_edit("editor"));

        ladder.set_level(AutonomyLevel::FullAuto);
        assert!(ladder.can_auto_edit("editor"));
    }

    #[test]
    fn test_can_auto_edit_suggest_is_false() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::Suggest);
        assert!(!ladder.can_auto_edit("editor"));
    }

    #[test]
    fn test_tool_level_updated_at_changes() {
        let mut ladder = AutonomyLadder::new();
        let before = ladder.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        ladder.set_tool_level("shell", AutonomyLevel::FullAuto);
        assert!(ladder.updated_at > before);
    }

    #[test]
    fn test_set_level_updated_at_changes() {
        let mut ladder = AutonomyLadder::new();
        let before = ladder.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        ladder.set_level(AutonomyLevel::FullAuto);
        assert!(ladder.updated_at > before);
    }

    #[test]
    fn test_autonomy_level_partial_eq() {
        assert_eq!(AutonomyLevel::Suggest, AutonomyLevel::Suggest);
        assert_ne!(AutonomyLevel::Suggest, AutonomyLevel::FullAuto);
        assert_ne!(AutonomyLevel::AutoEdit, AutonomyLevel::FullAuto);
    }

    #[test]
    fn test_autonomy_ladder_clone() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::FullAuto);
        ladder.set_tool_level("shell", AutonomyLevel::AutoEdit);
        let cloned = ladder.clone();
        assert_eq!(cloned.current_level, AutonomyLevel::FullAuto);
        assert_eq!(cloned.get_tool_level("shell"), &AutonomyLevel::AutoEdit);
    }

    #[test]
    fn test_multiple_tool_overrides() {
        let mut ladder = AutonomyLadder::new();
        ladder.set_level(AutonomyLevel::Suggest);
        ladder.set_tool_level("read_file", AutonomyLevel::FullAuto);
        ladder.set_tool_level("write_file", AutonomyLevel::AutoEdit);
        ladder.set_tool_level("shell", AutonomyLevel::Suggest);

        assert_eq!(ladder.get_tool_level("read_file"), &AutonomyLevel::FullAuto);
        assert_eq!(
            ladder.get_tool_level("write_file"),
            &AutonomyLevel::AutoEdit
        );
        assert_eq!(ladder.get_tool_level("shell"), &AutonomyLevel::Suggest);
        assert_eq!(ladder.get_tool_level("unknown"), &AutonomyLevel::Suggest);
    }
}
