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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = CliConfig::default();
        assert_eq!(cfg.profiles.len(), 1);
        assert_eq!(cfg.active_profile, "default");
        let profile = cfg.active_profile().expect("should have active profile");
        assert_eq!(profile.name, "default");
        assert_eq!(profile.api_endpoint, "http://localhost:8080");
        assert_eq!(profile.namespace, Some("default".to_string()));
        assert_eq!(profile.output_format, Some("json".to_string()));
        assert!(profile.api_key.is_none());
    }

    #[test]
    fn test_active_profile_found() {
        let cfg = CliConfig::default();
        let profile = cfg.active_profile();
        assert!(profile.is_some());
        assert_eq!(profile.expect("should exist").name, "default");
    }

    #[test]
    fn test_active_profile_not_found() {
        let cfg = CliConfig {
            profiles: vec![Profile {
                name: "prod".to_string(),
                api_endpoint: "https://prod.example.com".to_string(),
                api_key: Some("key".to_string()),
                namespace: None,
                output_format: None,
            }],
            active_profile: "staging".to_string(),
        };
        assert!(cfg.active_profile().is_none());
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let cfg = CliConfig::default();
        let json = serde_json::to_string_pretty(&cfg).expect("should serialize");
        let loaded: CliConfig = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(loaded.profiles.len(), cfg.profiles.len());
        assert_eq!(loaded.active_profile, cfg.active_profile);
        assert_eq!(
            loaded.profiles[0].api_endpoint,
            cfg.profiles[0].api_endpoint
        );
    }

    #[test]
    fn test_profile_with_api_key() {
        let profile = Profile {
            name: "prod".to_string(),
            api_endpoint: "https://api.kias.io".to_string(),
            api_key: Some("sk-test-key".to_string()),
            namespace: Some("production".to_string()),
            output_format: Some("table".to_string()),
        };
        let cfg = CliConfig {
            profiles: vec![profile],
            active_profile: "prod".to_string(),
        };
        let active = cfg.active_profile().expect("should exist");
        assert_eq!(active.api_key, Some("sk-test-key".to_string()));
    }

    #[test]
    fn test_multiple_profiles() {
        let cfg = CliConfig {
            profiles: vec![
                Profile {
                    name: "dev".to_string(),
                    api_endpoint: "http://localhost:8080".to_string(),
                    api_key: None,
                    namespace: None,
                    output_format: None,
                },
                Profile {
                    name: "prod".to_string(),
                    api_endpoint: "https://prod.kias.io".to_string(),
                    api_key: Some("key".to_string()),
                    namespace: Some("production".to_string()),
                    output_format: Some("json".to_string()),
                },
            ],
            active_profile: "prod".to_string(),
        };
        let active = cfg.active_profile().expect("should find prod");
        assert_eq!(active.api_endpoint, "https://prod.kias.io");
    }

    #[test]
    fn test_config_path_not_empty() {
        let path = CliConfig::config_path();
        assert!(path.to_string_lossy().contains(".kias"));
        assert!(path.to_string_lossy().contains("config.json"));
    }

    #[test]
    fn test_empty_profiles_no_active() {
        let cfg = CliConfig {
            profiles: vec![],
            active_profile: "nonexistent".to_string(),
        };
        assert!(cfg.active_profile().is_none());
    }

    #[test]
    fn test_profile_clone_debug() {
        let profile = Profile {
            name: "test".to_string(),
            api_endpoint: "http://test".to_string(),
            api_key: Some("key".to_string()),
            namespace: Some("ns".to_string()),
            output_format: Some("table".to_string()),
        };
        let cloned = profile.clone();
        assert_eq!(cloned.name, "test");
        assert_eq!(cloned.api_key, Some("key".to_string()));
        let _debug = format!("{:?}", cloned);
    }

    #[test]
    fn test_config_json_roundtrip_with_multiple_profiles() {
        let cfg = CliConfig {
            profiles: vec![
                Profile {
                    name: "dev".to_string(),
                    api_endpoint: "http://localhost:8080".to_string(),
                    api_key: None,
                    namespace: None,
                    output_format: None,
                },
                Profile {
                    name: "staging".to_string(),
                    api_endpoint: "https://staging.kias.io".to_string(),
                    api_key: Some("staging-key".to_string()),
                    namespace: Some("staging".to_string()),
                    output_format: Some("yaml".to_string()),
                },
            ],
            active_profile: "staging".to_string(),
        };
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let loaded: CliConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.profiles.len(), 2);
        assert_eq!(loaded.active_profile, "staging");
        assert_eq!(loaded.profiles[1].api_key, Some("staging-key".to_string()));
    }

    #[test]
    fn test_profile_all_none_optional_fields() {
        let profile = Profile {
            name: "minimal".to_string(),
            api_endpoint: "http://minimal".to_string(),
            api_key: None,
            namespace: None,
            output_format: None,
        };
        let cfg = CliConfig {
            profiles: vec![profile],
            active_profile: "minimal".to_string(),
        };
        let active = cfg.active_profile().unwrap();
        assert!(active.api_key.is_none());
        assert!(active.namespace.is_none());
        assert!(active.output_format.is_none());
    }
}
