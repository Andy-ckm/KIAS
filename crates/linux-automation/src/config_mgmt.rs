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
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drift_detector_new() {
        let detector = DriftDetector::new("/tmp/baseline");
        assert_eq!(detector.baseline_path, "/tmp/baseline");
    }

    #[test]
    fn test_drift_detector_default_monitored_paths() {
        let detector = DriftDetector::new("/tmp/baseline");
        let paths = detector.monitored_paths();
        assert_eq!(paths.len(), 10);
        assert!(paths.contains(&"/etc/ssh/sshd_config".to_string()));
        assert!(paths.contains(&"/etc/passwd".to_string()));
        assert!(paths.contains(&"/etc/sudoers".to_string()));
    }

    #[test]
    fn test_build_check_commands_count_matches_paths() {
        let detector = DriftDetector::new("/tmp/baseline");
        let commands = detector.build_check_commands();
        assert_eq!(commands.len(), detector.monitored_paths().len());
    }

    #[test]
    fn test_build_check_commands_format() {
        let detector = DriftDetector::new("/tmp/baseline");
        let commands = detector.build_check_commands();
        for cmd in &commands {
            assert!(cmd.starts_with("md5sum "));
            assert!(cmd.contains("2>/dev/null || echo 'FILE_NOT_FOUND "));
        }
    }

    #[test]
    fn test_add_monitor_new_path() {
        let mut detector = DriftDetector::new("/tmp/baseline");
        let initial_count = detector.monitored_paths().len();
        detector.add_monitor("/etc/nginx/nginx.conf".to_string());
        assert_eq!(detector.monitored_paths().len(), initial_count + 1);
        assert!(detector
            .monitored_paths()
            .contains(&"/etc/nginx/nginx.conf".to_string()));
    }

    #[test]
    fn test_add_monitor_duplicate_ignored() {
        let mut detector = DriftDetector::new("/tmp/baseline");
        let initial_count = detector.monitored_paths().len();
        detector.add_monitor("/etc/passwd".to_string()); // already exists
        assert_eq!(detector.monitored_paths().len(), initial_count);
    }

    #[test]
    fn test_monitored_paths_returns_slice() {
        let detector = DriftDetector::new("/tmp/baseline");
        let paths = detector.monitored_paths();
        // returns a &[String] slice
        assert!(!paths.is_empty());
        assert_eq!(paths[0], "/etc/ssh/sshd_config");
    }
}
