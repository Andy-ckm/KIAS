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
/// 沙箱资源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResources {
    pub memory: String,
    pub cpu: f64,
    pub disk: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_serialize() {
        let sandbox = Sandbox {
            id: "sb-001".to_string(),
            name: "test-sandbox".to_string(),
            template: "python3.11".to_string(),
            status: SandboxStatus::Running,
            created_at: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&sandbox).expect("should serialize");
        assert!(json.contains("sb-001"));
        assert!(json.contains("Running"));
    }

    #[test]
    fn test_sandbox_deserialize() {
        let json = r#"{
            "id": "sb-002",
            "name": "my-sandbox",
            "template": "node20",
            "status": "Creating",
            "created_at": "2024-06-01T12:00:00Z"
        }"#;
        let sandbox: Sandbox = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(sandbox.id, "sb-002");
        assert!(matches!(sandbox.status, SandboxStatus::Creating));
    }

    #[test]
    fn test_sandbox_template() {
        let template = SandboxTemplate {
            name: "python3.11".to_string(),
            description: "Python 3.11 environment".to_string(),
            image: "python:3.11-slim".to_string(),
            resources: SandboxResources {
                memory: "512Mi".to_string(),
                cpu: 0.5,
                disk: "1Gi".to_string(),
            },
        };
        let json = serde_json::to_string(&template).expect("should serialize");
        assert!(json.contains("python:3.11-slim"));
    }
}
