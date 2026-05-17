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

    #[test]
    fn test_sandbox_status_all_variants() {
        let variants = vec![
            ("Creating", SandboxStatus::Creating),
            ("Running", SandboxStatus::Running),
            ("Stopped", SandboxStatus::Stopped),
            ("Error", SandboxStatus::Error),
        ];
        for (name, status) in variants {
            let sandbox = Sandbox {
                id: "test".to_string(),
                name: "test".to_string(),
                template: "test".to_string(),
                status,
                created_at: "2024-01-01".to_string(),
            };
            let json = serde_json::to_string(&sandbox).unwrap();
            assert!(json.contains(name), "expected {name} in JSON");
        }
    }

    #[test]
    fn test_sandbox_clone_debug() {
        let sandbox = Sandbox {
            id: "sb-clone".to_string(),
            name: "clone-test".to_string(),
            template: "node20".to_string(),
            status: SandboxStatus::Stopped,
            created_at: "2024-01-01".to_string(),
        };
        let cloned = sandbox.clone();
        assert_eq!(cloned.id, "sb-clone");
        assert!(matches!(cloned.status, SandboxStatus::Stopped));
        let _debug = format!("{:?}", cloned);
    }

    #[test]
    fn test_sandbox_template_deserialize() {
        let json = r#"{
            "name": "node20",
            "description": "Node.js 20",
            "image": "node:20-slim",
            "resources": {"memory": "1Gi", "cpu": 1.0, "disk": "2Gi"}
        }"#;
        let template: SandboxTemplate = serde_json::from_str(json).unwrap();
        assert_eq!(template.name, "node20");
        assert_eq!(template.resources.cpu, 1.0);
        assert_eq!(template.resources.memory, "1Gi");
    }

    #[test]
    fn test_sandbox_resources_clone() {
        let res = SandboxResources {
            memory: "256Mi".to_string(),
            cpu: 0.25,
            disk: "512Mi".to_string(),
        };
        let cloned = res.clone();
        assert_eq!(cloned.memory, "256Mi");
        assert_eq!(cloned.cpu, 0.25);
    }
}
