//! 配置管理

use crate::error::{AutomationError, Result};
use crate::models::LinuxAutomationConfig;
use std::path::Path;

/// 配置加载和验证
pub trait ConfigLoader {
    /// 从文件加载配置
    fn load_from_file(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// 验证配置
    fn validate(&self) -> Result<()>;
}

impl ConfigLoader for LinuxAutomationConfig {
    fn load_from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AutomationError::Config(format!("读取配置文件失败: {}", e)))?;

        let config: Self = serde_json::from_str(&content)
            .map_err(|e| AutomationError::Config(format!("解析配置文件失败: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.target_hosts.is_empty() {
            return Err(AutomationError::Config("目标服务器列表为空".to_string()));
        }

        if !self.database_path.parent().is_some_and(|p| p.exists()) {
            return Err(AutomationError::Config("数据库目录不存在".to_string()));
        }

        Ok(())
    }
}

impl LinuxAutomationConfig {
    /// 创建默认配置
    pub fn default_config() -> Self {
        Self {
            database_path: Path::new("/var/lib/agentguard/automation.db").to_path_buf(),
            playbook_dir: Path::new("/etc/agentguard/playbooks").to_path_buf(),
            ssh_key_path: Some(Path::new("/root/.ssh/id_rsa").to_path_buf()),
            log_dir: Path::new("/var/log/agentguard").to_path_buf(),
            target_hosts: vec![],
            compliance_tool: crate::models::ComplianceTool::OpenScap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ComplianceTool;
    use tempfile::TempDir;

    // ============================================================
    // validate() tests
    // ============================================================

    #[test]
    fn test_validate_empty_hosts() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec![],
            compliance_tool: ComplianceTool::OpenScap,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_valid_config() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_nonexistent_db_dir() {
        let config = LinuxAutomationConfig {
            database_path: std::path::PathBuf::from("/nonexistent/dir/test.db"),
            playbook_dir: std::path::PathBuf::from("/tmp/playbooks"),
            ssh_key_path: None,
            log_dir: std::path::PathBuf::from("/tmp/logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_multiple_hosts() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec![
                "host1".to_string(),
                "host2".to_string(),
                "host3".to_string(),
            ],
            compliance_tool: ComplianceTool::OpenScap,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_hosts_error_message() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec![],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let err = config.validate().unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("目标服务器列表为空"));
    }

    #[test]
    fn test_validate_nonexistent_db_dir_error_message() {
        let config = LinuxAutomationConfig {
            database_path: std::path::PathBuf::from("/nonexistent/dir/test.db"),
            playbook_dir: std::path::PathBuf::from("/tmp/playbooks"),
            ssh_key_path: None,
            log_dir: std::path::PathBuf::from("/tmp/logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let err = config.validate().unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("数据库目录不存在"));
    }

    #[test]
    fn test_validate_with_lynis_tool() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::Lynis,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_with_ciscat_tool() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::CisCat,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_with_custom_tool() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::Custom("my-scanner".to_string()),
        };
        assert!(config.validate().is_ok());
    }

    // ============================================================
    // default_config() tests
    // ============================================================

    #[test]
    fn test_default_config() {
        let config = LinuxAutomationConfig::default_config();
        assert!(config.target_hosts.is_empty());
        assert!(config.ssh_key_path.is_some());
    }

    #[test]
    fn test_default_config_database_path() {
        let config = LinuxAutomationConfig::default_config();
        assert_eq!(
            config.database_path,
            std::path::PathBuf::from("/var/lib/agentguard/automation.db")
        );
    }

    #[test]
    fn test_default_config_playbook_dir() {
        let config = LinuxAutomationConfig::default_config();
        assert_eq!(
            config.playbook_dir,
            std::path::PathBuf::from("/etc/agentguard/playbooks")
        );
    }

    #[test]
    fn test_default_config_ssh_key_path() {
        let config = LinuxAutomationConfig::default_config();
        assert_eq!(
            config.ssh_key_path.unwrap(),
            std::path::PathBuf::from("/root/.ssh/id_rsa")
        );
    }

    #[test]
    fn test_default_config_log_dir() {
        let config = LinuxAutomationConfig::default_config();
        assert_eq!(
            config.log_dir,
            std::path::PathBuf::from("/var/log/agentguard")
        );
    }

    #[test]
    fn test_default_config_compliance_tool() {
        let config = LinuxAutomationConfig::default_config();
        assert_eq!(config.compliance_tool, ComplianceTool::OpenScap);
    }

    #[test]
    fn test_default_config_fails_validate_empty_hosts() {
        let config = LinuxAutomationConfig::default_config();
        assert!(config.validate().is_err());
    }

    // ============================================================
    // load_from_file() tests
    // ============================================================

    #[test]
    fn test_load_from_file_valid() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let json = serde_json::json!({
            "database_path": tmp.path().join("test.db"),
            "playbook_dir": tmp.path().join("playbooks"),
            "ssh_key_path": null,
            "log_dir": tmp.path().join("logs"),
            "target_hosts": ["localhost"],
            "compliance_tool": "OpenScap"
        });
        std::fs::write(&config_path, json.to_string()).unwrap();

        let config = LinuxAutomationConfig::load_from_file(&config_path).unwrap();
        assert_eq!(config.target_hosts, vec!["localhost"]);
        assert_eq!(config.compliance_tool, ComplianceTool::OpenScap);
    }

    #[test]
    fn test_load_from_file_missing_file() {
        let result =
            LinuxAutomationConfig::load_from_file(std::path::Path::new("/nonexistent/config.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_file_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("bad.json");
        std::fs::write(&config_path, "not valid json {{{").unwrap();

        let result = LinuxAutomationConfig::load_from_file(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_file_empty_hosts_fails_validation() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let json = serde_json::json!({
            "database_path": tmp.path().join("test.db"),
            "playbook_dir": tmp.path().join("playbooks"),
            "ssh_key_path": null,
            "log_dir": tmp.path().join("logs"),
            "target_hosts": [],
            "compliance_tool": "OpenScap"
        });
        std::fs::write(&config_path, json.to_string()).unwrap();

        let result = LinuxAutomationConfig::load_from_file(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_file_with_ssh_key() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let json = serde_json::json!({
            "database_path": tmp.path().join("test.db"),
            "playbook_dir": tmp.path().join("playbooks"),
            "ssh_key_path": "/home/user/.ssh/id_ed25519",
            "log_dir": tmp.path().join("logs"),
            "target_hosts": ["10.0.0.1"],
            "compliance_tool": "Lynis"
        });
        std::fs::write(&config_path, json.to_string()).unwrap();

        let config = LinuxAutomationConfig::load_from_file(&config_path).unwrap();
        assert!(config.ssh_key_path.is_some());
        assert_eq!(config.target_hosts, vec!["10.0.0.1"]);
        assert_eq!(config.compliance_tool, ComplianceTool::Lynis);
    }

    #[test]
    fn test_load_from_file_custom_compliance_tool() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let json = serde_json::json!({
            "database_path": tmp.path().join("test.db"),
            "playbook_dir": tmp.path().join("playbooks"),
            "ssh_key_path": null,
            "log_dir": tmp.path().join("logs"),
            "target_hosts": ["server1"],
            "compliance_tool": {"Custom": "trivy"}
        });
        std::fs::write(&config_path, json.to_string()).unwrap();

        let config = LinuxAutomationConfig::load_from_file(&config_path).unwrap();
        assert_eq!(
            config.compliance_tool,
            ComplianceTool::Custom("trivy".to_string())
        );
    }

    // ============================================================
    // Serialization roundtrip tests
    // ============================================================

    #[test]
    fn test_config_serialization_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: Some(std::path::PathBuf::from("/root/.ssh/id_rsa")),
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string(), "host2".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: LinuxAutomationConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.target_hosts, config.target_hosts);
        assert_eq!(deserialized.compliance_tool, config.compliance_tool);
        assert_eq!(deserialized.ssh_key_path, config.ssh_key_path);
    }

    #[test]
    fn test_config_serialization_with_null_ssh_key() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::Lynis,
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: LinuxAutomationConfig = serde_json::from_str(&json).unwrap();
        assert!(deserialized.ssh_key_path.is_none());
    }

    #[test]
    fn test_config_clone() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let cloned = config.clone();
        assert_eq!(cloned.target_hosts, config.target_hosts);
        assert_eq!(cloned.compliance_tool, config.compliance_tool);
    }

    #[test]
    fn test_config_debug_format() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["host1".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let debug = format!("{:?}", config);
        assert!(debug.contains("LinuxAutomationConfig"));
        assert!(debug.contains("target_hosts"));
        assert!(debug.contains("OpenScap"));
    }

    // ============================================================
    // ComplianceTool tests
    // ============================================================

    #[test]
    fn test_compliance_tool_openscap_serialization() {
        let tool = ComplianceTool::OpenScap;
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ComplianceTool = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ComplianceTool::OpenScap);
    }

    #[test]
    fn test_compliance_tool_lynis_serialization() {
        let tool = ComplianceTool::Lynis;
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ComplianceTool = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ComplianceTool::Lynis);
    }

    #[test]
    fn test_compliance_tool_ciscat_serialization() {
        let tool = ComplianceTool::CisCat;
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ComplianceTool = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ComplianceTool::CisCat);
    }

    #[test]
    fn test_compliance_tool_custom_serialization() {
        let tool = ComplianceTool::Custom("trivy".to_string());
        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: ComplianceTool = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ComplianceTool::Custom("trivy".to_string()));
    }

    #[test]
    fn test_compliance_tool_clone() {
        let tool = ComplianceTool::Custom("custom-scanner".to_string());
        let cloned = tool.clone();
        assert_eq!(tool, cloned);
    }

    #[test]
    fn test_compliance_tool_debug() {
        let tool = ComplianceTool::OpenScap;
        let debug = format!("{:?}", tool);
        assert_eq!(debug, "OpenScap");
    }

    #[test]
    fn test_compliance_tool_partial_eq() {
        assert_eq!(ComplianceTool::OpenScap, ComplianceTool::OpenScap);
        assert_eq!(ComplianceTool::Lynis, ComplianceTool::Lynis);
        assert_eq!(ComplianceTool::CisCat, ComplianceTool::CisCat);
        assert_eq!(
            ComplianceTool::Custom("a".to_string()),
            ComplianceTool::Custom("a".to_string())
        );
        assert_ne!(ComplianceTool::OpenScap, ComplianceTool::Lynis);
        assert_ne!(
            ComplianceTool::Custom("a".to_string()),
            ComplianceTool::Custom("b".to_string())
        );
    }
}
