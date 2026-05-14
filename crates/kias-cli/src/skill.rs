//! 技能管理模块

use serde::{Deserialize, Serialize};

/// 技能定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub parameters: Option<serde_json::Value>,
}
