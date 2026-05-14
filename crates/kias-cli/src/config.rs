//! 配置管理模块

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// KIAS CLI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub profiles: Vec<Profile>,
    pub active_profile: String,
}

/// 配置 Profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub api_endpoint: String,
    pub api_key: Option<String>,
    pub namespace: Option<String>,
    pub output_format: Option<String>,
}

impl CliConfig {
    /// 获取配置文件路径
    pub fn config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".kias")
            .join("config.json")
    }

    /// 加载配置
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// 保存配置
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// 获取当前 Profile
    pub fn active_profile(&self) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == self.active_profile)
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            profiles: vec![Profile {
                name: "default".to_string(),
                api_endpoint: "http://localhost:8080".to_string(),
                api_key: None,
                namespace: Some("default".to_string()),
                output_format: Some("json".to_string()),
            }],
            active_profile: "default".to_string(),
        }
    }
}
