//! 沙箱管理模块

use serde::{Deserialize, Serialize};

/// 沙箱定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sandbox {
    pub id: String,
    pub name: String,
    pub template: String,
    pub status: SandboxStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxStatus {
    Creating,
    Running,
    Stopped,
    Error,
}

/// 沙箱模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxTemplate {
    pub name: String,
    pub description: String,
    pub image: String,
    pub resources: SandboxResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResources {
    pub memory: String,
    pub cpu: f64,
    pub disk: String,
}
