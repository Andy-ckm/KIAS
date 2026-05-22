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
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // DriftDetector::new tests
    // ============================================================

    #[test]
    fn test_drift_detector_new() {
        let detector = DriftDetector::new("/tmp/baseline");
        assert_eq!(detector.baseline_path, "/tmp/baseline");
    }

    #[test]
    fn test_drift_detector_new_empty_path() {
        let detector = DriftDetector::new("");
        assert_eq!(detector.baseline_path, "");
    }

    #[test]
    fn test_drift_detector_new_preserves_path() {
        let detector = DriftDetector::new("/opt/agentguard/baselines/prod");
        assert_eq!(detector.baseline_path, "/opt/agentguard/baselines/prod");
    }

    // ============================================================
    // monitored_paths tests
    // ============================================================

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
    fn test_monitored_paths_returns_slice() {
        let detector = DriftDetector::new("/tmp/baseline");
        let paths = detector.monitored_paths();
        // returns a &[String] slice
        assert!(!paths.is_empty());
        assert_eq!(paths[0], "/etc/ssh/sshd_config");
    }

    #[test]
    fn test_monitored_paths_contains_all_security_files() {
        let detector = DriftDetector::new("/tmp/baseline");
        let paths = detector.monitored_paths();
        let expected = vec![
            "/etc/ssh/sshd_config",
            "/etc/sudoers",
            "/etc/passwd",
            "/etc/shadow",
            "/etc/group",
            "/etc/hosts",
            "/etc/resolv.conf",
            "/etc/fstab",
            "/etc/sysctl.conf",
            "/etc/security/limits.conf",
        ];
        for p in expected {
            assert!(paths.contains(&p.to_string()), "missing: {}", p);
        }
    }

    #[test]
    fn test_monitored_paths_no_duplicates() {
        let detector = DriftDetector::new("/tmp/baseline");
        let paths = detector.monitored_paths();
        let mut sorted = paths.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), paths.len());
    }

    // ============================================================
    // build_check_commands tests
    // ============================================================

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
    fn test_build_check_commands_contain_all_paths() {
        let detector = DriftDetector::new("/tmp/baseline");
        let commands = detector.build_check_commands();
        for path in detector.monitored_paths() {
            let found = commands.iter().any(|cmd| cmd.contains(path));
            assert!(found, "command missing for path: {}", path);
        }
    }

    #[test]
    fn test_build_check_commands_end_with_quote() {
        let detector = DriftDetector::new("/tmp/baseline");
        let commands = detector.build_check_commands();
        for cmd in &commands {
            assert!(
                cmd.ends_with('\''),
                "command should end with quote: {}",
                cmd
            );
        }
    }

    #[test]
    fn test_build_check_commands_first_is_sshd() {
        let detector = DriftDetector::new("/tmp/baseline");
        let commands = detector.build_check_commands();
        assert!(commands[0].contains("/etc/ssh/sshd_config"));
    }

    // ============================================================
    // add_monitor tests
    // ============================================================

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
    fn test_add_monitor_multiple_paths() {
        let mut detector = DriftDetector::new("/tmp/baseline");
        let initial_count = detector.monitored_paths().len();
        detector.add_monitor("/etc/nginx/nginx.conf".to_string());
        detector.add_monitor("/etc/mysql/my.cnf".to_string());
        detector.add_monitor("/etc/redis/redis.conf".to_string());
        assert_eq!(detector.monitored_paths().len(), initial_count + 3);
    }

    #[test]
    fn test_add_monitor_then_build_commands() {
        let mut detector = DriftDetector::new("/tmp/baseline");
        detector.add_monitor("/etc/custom.conf".to_string());
        let commands = detector.build_check_commands();
        assert_eq!(commands.len(), detector.monitored_paths().len());
        let found = commands.iter().any(|cmd| cmd.contains("/etc/custom.conf"));
        assert!(found);
    }

    #[test]
    fn test_add_monitor_empty_string() {
        let mut detector = DriftDetector::new("/tmp/baseline");
        let initial_count = detector.monitored_paths().len();
        detector.add_monitor("".to_string());
        assert_eq!(detector.monitored_paths().len(), initial_count + 1);
    }

    // ============================================================
    // ConfigFile tests
    // ============================================================

    #[test]
    fn test_config_file_creation() {
        let f = ConfigFile {
            path: "/etc/ssh/sshd_config".to_string(),
            content_hash: "abc123".to_string(),
            permissions: "644".to_string(),
            owner: "root".to_string(),
            group: "root".to_string(),
        };
        assert_eq!(f.path, "/etc/ssh/sshd_config");
        assert_eq!(f.content_hash, "abc123");
        assert_eq!(f.permissions, "644");
        assert_eq!(f.owner, "root");
        assert_eq!(f.group, "root");
    }

    #[test]
    fn test_config_file_clone() {
        let f = ConfigFile {
            path: "/etc/passwd".to_string(),
            content_hash: "def456".to_string(),
            permissions: "644".to_string(),
            owner: "root".to_string(),
            group: "root".to_string(),
        };
        let cloned = f.clone();
        assert_eq!(cloned.path, f.path);
        assert_eq!(cloned.content_hash, f.content_hash);
    }

    #[test]
    fn test_config_file_debug() {
        let f = ConfigFile {
            path: "/etc/hosts".to_string(),
            content_hash: "ghi789".to_string(),
            permissions: "644".to_string(),
            owner: "root".to_string(),
            group: "root".to_string(),
        };
        let debug = format!("{:?}", f);
        assert!(debug.contains("ConfigFile"));
        assert!(debug.contains("/etc/hosts"));
    }

    #[test]
    fn test_config_file_serialization() {
        let f = ConfigFile {
            path: "/etc/fstab".to_string(),
            content_hash: "hash123".to_string(),
            permissions: "644".to_string(),
            owner: "root".to_string(),
            group: "disk".to_string(),
        };
        let json = serde_json::to_string(&f).unwrap();
        let deserialized: ConfigFile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.path, f.path);
        assert_eq!(deserialized.permissions, f.permissions);
        assert_eq!(deserialized.group, "disk");
    }

    // ============================================================
    // ConfigSnapshot tests
    // ============================================================

    #[test]
    fn test_config_snapshot_creation() {
        let snap = ConfigSnapshot {
            id: "snap-001".to_string(),
            host: "server1".to_string(),
            timestamp: Utc::now(),
            files: vec![],
            checksum: "abc".to_string(),
        };
        assert_eq!(snap.id, "snap-001");
        assert_eq!(snap.host, "server1");
        assert!(snap.files.is_empty());
    }

    #[test]
    fn test_config_snapshot_with_files() {
        let snap = ConfigSnapshot {
            id: "snap-002".to_string(),
            host: "server2".to_string(),
            timestamp: Utc::now(),
            files: vec![
                ConfigFile {
                    path: "/etc/passwd".to_string(),
                    content_hash: "h1".to_string(),
                    permissions: "644".to_string(),
                    owner: "root".to_string(),
                    group: "root".to_string(),
                },
                ConfigFile {
                    path: "/etc/shadow".to_string(),
                    content_hash: "h2".to_string(),
                    permissions: "640".to_string(),
                    owner: "root".to_string(),
                    group: "shadow".to_string(),
                },
            ],
            checksum: "def".to_string(),
        };
        assert_eq!(snap.files.len(), 2);
        assert_eq!(snap.files[0].path, "/etc/passwd");
        assert_eq!(snap.files[1].permissions, "640");
    }

    #[test]
    fn test_config_snapshot_clone() {
        let snap = ConfigSnapshot {
            id: "snap-003".to_string(),
            host: "server3".to_string(),
            timestamp: Utc::now(),
            files: vec![],
            checksum: "ghi".to_string(),
        };
        let cloned = snap.clone();
        assert_eq!(cloned.id, snap.id);
        assert_eq!(cloned.host, snap.host);
        assert_eq!(cloned.checksum, snap.checksum);
    }

    #[test]
    fn test_config_snapshot_debug() {
        let snap = ConfigSnapshot {
            id: "snap-004".to_string(),
            host: "server4".to_string(),
            timestamp: Utc::now(),
            files: vec![],
            checksum: "jkl".to_string(),
        };
        let debug = format!("{:?}", snap);
        assert!(debug.contains("ConfigSnapshot"));
        assert!(debug.contains("snap-004"));
    }

    #[test]
    fn test_config_snapshot_serialization() {
        let snap = ConfigSnapshot {
            id: "snap-005".to_string(),
            host: "server5".to_string(),
            timestamp: Utc::now(),
            files: vec![ConfigFile {
                path: "/etc/hosts".to_string(),
                content_hash: "h3".to_string(),
                permissions: "644".to_string(),
                owner: "root".to_string(),
                group: "root".to_string(),
            }],
            checksum: "mno".to_string(),
        };
        let json = serde_json::to_string(&snap).unwrap();
        let deserialized: ConfigSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, snap.id);
        assert_eq!(deserialized.files.len(), 1);
    }

    // ============================================================
    // DriftDetector clone/debug tests
    // ============================================================

    #[test]
    fn test_drift_detector_clone() {
        let detector = DriftDetector::new("/tmp/baseline");
        let cloned = detector.clone();
        assert_eq!(cloned.baseline_path, detector.baseline_path);
        assert_eq!(
            cloned.monitored_paths().len(),
            detector.monitored_paths().len()
        );
    }

    #[test]
    fn test_drift_detector_debug() {
        let detector = DriftDetector::new("/tmp/baseline");
        let debug = format!("{:?}", detector);
        assert!(debug.contains("DriftDetector"));
        assert!(debug.contains("/tmp/baseline"));
    }
}
