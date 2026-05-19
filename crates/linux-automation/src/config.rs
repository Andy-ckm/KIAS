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
    use tempfile::TempDir;

    #[test]
    fn test_validate_empty_hosts() {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec![],
            compliance_tool: crate::models::ComplianceTool::OpenScap,
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
            compliance_tool: crate::models::ComplianceTool::OpenScap,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_config() {
        let config = LinuxAutomationConfig::default_config();
        assert!(config.target_hosts.is_empty());
        assert!(config.ssh_key_path.is_some());
    }

    #[test]
    fn test_validate_nonexistent_db_dir() {
        let config = LinuxAutomationConfig {
            database_path: std::path::PathBuf::from("/nonexistent/dir/test.db"),
            playbook_dir: std::path::PathBuf::from("/tmp/playbooks"),
            ssh_key_path: None,
            log_dir: std::path::PathBuf::from("/tmp/logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: crate::models::ComplianceTool::OpenScap,
        };

        assert!(config.validate().is_err());
    }
}
