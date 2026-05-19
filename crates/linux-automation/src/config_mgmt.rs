//! 配置管理模块
//! 管理 Linux 系统配置的版本化和漂移检测

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 配置快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub id: String,
    pub host: String,
    pub timestamp: DateTime<Utc>,
    pub files: Vec<ConfigFile>,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    pub path: String,
    pub content_hash: String,
    pub permissions: String,
    pub owner: String,
    pub group: String,
}

/// 配置漂移检测
#[derive(Debug, Clone)]
pub struct DriftDetector {
    baseline_path: String,
    monitored_paths: Vec<String>,
}

impl DriftDetector {
    pub fn new(baseline_path: &str) -> Self {
        Self {
            baseline_path: baseline_path.to_string(),
            monitored_paths: vec![
                "/etc/ssh/sshd_config".to_string(),
                "/etc/sudoers".to_string(),
                "/etc/passwd".to_string(),
                "/etc/shadow".to_string(),
                "/etc/group".to_string(),
                "/etc/hosts".to_string(),
                "/etc/resolv.conf".to_string(),
                "/etc/fstab".to_string(),
                "/etc/sysctl.conf".to_string(),
                "/etc/security/limits.conf".to_string(),
            ],
        }
    }

    /// 生成配置检查命令
    pub fn build_check_commands(&self) -> Vec<String> {
        self.monitored_paths
            .iter()
            .map(|path| {
                format!(
                    "md5sum {} 2>/dev/null || echo 'FILE_NOT_FOUND {}'",
                    path, path
                )
            })
            .collect()
    }

    /// 添加监控路径
    pub fn add_monitor(&mut self, path: String) {
        if !self.monitored_paths.contains(&path) {
            self.monitored_paths.push(path);
        }
    }

    /// 获取所有监控路径
    pub fn monitored_paths(&self) -> &[String] {
        &self.monitored_paths
    }
}
