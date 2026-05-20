//! 补丁管理模块
//! 支持 yum/apt 包管理器的安全补丁

use serde::{Deserialize, Serialize};

/// 包管理器类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageManager {
    Yum,
    Apt,
    Dnf,
    Zypper,
}

/// 补丁信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    pub name: String,
    pub current_version: String,
    pub available_version: String,
    pub severity: PatchSeverity,
    pub advisory_id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchSeverity {
    Critical,
    Important,
    Moderate,
    Low,
}

/// 补丁管理器
#[allow(dead_code)]
pub struct PatchManager {
    package_manager: PackageManager,
    auto_reboot: bool,
    exclude_packages: Vec<String>,
}

impl PatchManager {
    pub fn new(pm: PackageManager) -> Self {
        Self {
            package_manager: pm,
            auto_reboot: false,
            exclude_packages: Vec::new(),
        }
    }

    /// 构建更新命令
    pub fn build_update_command(&self, security_only: bool) -> String {
        match self.package_manager {
            PackageManager::Yum | PackageManager::Dnf => {
                if security_only {
                    "dnf update --security -y".to_string()
                } else {
                    "dnf update -y".to_string()
                }
            }
            PackageManager::Apt => {
                if security_only {
                    "apt-get update && apt-get upgrade -y -o Dir::Etc::SourceList=/etc/apt/sources.list.d/security.list".to_string()
                } else {
                    "apt-get update && apt-get upgrade -y".to_string()
                }
            }
            PackageManager::Zypper => {
                if security_only {
                    "zypper patch --category security".to_string()
                } else {
                    "zypper update -y".to_string()
                }
            }
        }
    }

    /// 构建检查更新命令
    pub fn build_check_command(&self) -> String {
        match self.package_manager {
            PackageManager::Yum | PackageManager::Dnf => "dnf check-update --security".to_string(),
            PackageManager::Apt => "apt list --upgradable 2>/dev/null".to_string(),
            PackageManager::Zypper => "zypper list-patches".to_string(),
        }
    }

    /// 检查是否需要重启
    pub fn build_reboot_check_command(&self) -> String {
        match self.package_manager {
            PackageManager::Yum | PackageManager::Dnf => "needs-restarting -r".to_string(),
            PackageManager::Apt => {
                "[ -f /var/run/reboot-required ] && echo 'REBOOT_REQUIRED' || echo 'NO_REBOOT'"
                    .to_string()
            }
            PackageManager::Zypper => "zypper needs-rebooting".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_manager_new_yum() {
        let pm = PatchManager::new(PackageManager::Yum);
        assert!(!pm.auto_reboot);
        assert!(pm.exclude_packages.is_empty());
    }

    #[test]
    fn test_patch_manager_new_apt() {
        let pm = PatchManager::new(PackageManager::Apt);
        assert!(!pm.auto_reboot);
    }

    #[test]
    fn test_build_update_command_yum_security() {
        let pm = PatchManager::new(PackageManager::Yum);
        assert_eq!(pm.build_update_command(true), "dnf update --security -y");
    }

    #[test]
    fn test_build_update_command_yum_all() {
        let pm = PatchManager::new(PackageManager::Yum);
        assert_eq!(pm.build_update_command(false), "dnf update -y");
    }

    #[test]
    fn test_build_update_command_apt_security() {
        let pm = PatchManager::new(PackageManager::Apt);
        let cmd = pm.build_update_command(true);
        assert!(cmd.contains("apt-get update"));
        assert!(cmd.contains("security.list"));
    }

    #[test]
    fn test_build_update_command_apt_all() {
        let pm = PatchManager::new(PackageManager::Apt);
        let cmd = pm.build_update_command(false);
        assert_eq!(cmd, "apt-get update && apt-get upgrade -y");
    }

    #[test]
    fn test_build_update_command_dnf_security() {
        let pm = PatchManager::new(PackageManager::Dnf);
        assert_eq!(pm.build_update_command(true), "dnf update --security -y");
    }

    #[test]
    fn test_build_update_command_zypper_security() {
        let pm = PatchManager::new(PackageManager::Zypper);
        assert_eq!(
            pm.build_update_command(true),
            "zypper patch --category security"
        );
    }

    #[test]
    fn test_build_update_command_zypper_all() {
        let pm = PatchManager::new(PackageManager::Zypper);
        assert_eq!(pm.build_update_command(false), "zypper update -y");
    }

    #[test]
    fn test_build_check_command_yum() {
        let pm = PatchManager::new(PackageManager::Yum);
        assert_eq!(pm.build_check_command(), "dnf check-update --security");
    }

    #[test]
    fn test_build_check_command_apt() {
        let pm = PatchManager::new(PackageManager::Apt);
        assert!(pm.build_check_command().contains("apt list"));
    }

    #[test]
    fn test_build_check_command_zypper() {
        let pm = PatchManager::new(PackageManager::Zypper);
        assert_eq!(pm.build_check_command(), "zypper list-patches");
    }

    #[test]
    fn test_build_reboot_check_command_yum() {
        let pm = PatchManager::new(PackageManager::Yum);
        assert_eq!(pm.build_reboot_check_command(), "needs-restarting -r");
    }

    #[test]
    fn test_build_reboot_check_command_apt() {
        let pm = PatchManager::new(PackageManager::Apt);
        let cmd = pm.build_reboot_check_command();
        assert!(cmd.contains("reboot-required"));
    }

    #[test]
    fn test_build_reboot_check_command_zypper() {
        let pm = PatchManager::new(PackageManager::Zypper);
        assert_eq!(pm.build_reboot_check_command(), "zypper needs-rebooting");
    }

    // ============================================================
    // PackageManager tests
    // ============================================================

    #[test]
    fn test_package_manager_variants() {
        let variants = [
            PackageManager::Yum,
            PackageManager::Apt,
            PackageManager::Dnf,
            PackageManager::Zypper,
        ];
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn test_package_manager_clone() {
        let pm = PackageManager::Apt;
        let cloned = pm.clone();
        assert!(matches!(cloned, PackageManager::Apt));
    }

    #[test]
    fn test_package_manager_debug() {
        assert_eq!(format!("{:?}", PackageManager::Yum), "Yum");
        assert_eq!(format!("{:?}", PackageManager::Apt), "Apt");
        assert_eq!(format!("{:?}", PackageManager::Dnf), "Dnf");
        assert_eq!(format!("{:?}", PackageManager::Zypper), "Zypper");
    }

    #[test]
    fn test_package_manager_serialization() {
        let variants = [
            PackageManager::Yum,
            PackageManager::Apt,
            PackageManager::Dnf,
            PackageManager::Zypper,
        ];
        for pm in variants {
            let json = serde_json::to_string(&pm).unwrap();
            let deserialized: PackageManager = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, json2);
        }
    }

    // ============================================================
    // PatchInfo tests
    // ============================================================

    #[test]
    fn test_patch_info_creation() {
        let info = PatchInfo {
            name: "openssl".to_string(),
            current_version: "1.1.1k".to_string(),
            available_version: "1.1.1w".to_string(),
            severity: PatchSeverity::Critical,
            advisory_id: "CESA-2024:0001".to_string(),
            description: "Security fix".to_string(),
        };
        assert_eq!(info.name, "openssl");
        assert!(matches!(info.severity, PatchSeverity::Critical));
    }

    #[test]
    fn test_patch_info_clone() {
        let info = PatchInfo {
            name: "curl".to_string(),
            current_version: "7.68".to_string(),
            available_version: "7.88".to_string(),
            severity: PatchSeverity::Important,
            advisory_id: "CESA-2024:0002".to_string(),
            description: "Bug fix".to_string(),
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert!(matches!(cloned.severity, PatchSeverity::Important));
    }

    #[test]
    fn test_patch_info_debug() {
        let info = PatchInfo {
            name: "vim".to_string(),
            current_version: "8.1".to_string(),
            available_version: "9.0".to_string(),
            severity: PatchSeverity::Low,
            advisory_id: "CESA-2024:0003".to_string(),
            description: "Enhancement".to_string(),
        };
        let debug = format!("{:?}", info);
        assert!(debug.contains("PatchInfo"));
        assert!(debug.contains("vim"));
    }

    #[test]
    fn test_patch_info_serialization() {
        let info = PatchInfo {
            name: "nginx".to_string(),
            current_version: "1.20".to_string(),
            available_version: "1.24".to_string(),
            severity: PatchSeverity::Moderate,
            advisory_id: "CESA-2024:0004".to_string(),
            description: "Update".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: PatchInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "nginx");
        assert!(matches!(deserialized.severity, PatchSeverity::Moderate));
    }

    // ============================================================
    // PatchSeverity tests
    // ============================================================

    #[test]
    fn test_patch_severity_variants() {
        let variants = [
            PatchSeverity::Critical,
            PatchSeverity::Important,
            PatchSeverity::Moderate,
            PatchSeverity::Low,
        ];
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn test_patch_severity_clone() {
        let s = PatchSeverity::Critical;
        let cloned = s.clone();
        assert!(matches!(cloned, PatchSeverity::Critical));
    }

    #[test]
    fn test_patch_severity_debug() {
        assert_eq!(format!("{:?}", PatchSeverity::Critical), "Critical");
        assert_eq!(format!("{:?}", PatchSeverity::Low), "Low");
    }

    #[test]
    fn test_patch_severity_serialization() {
        let variants = [
            PatchSeverity::Critical,
            PatchSeverity::Important,
            PatchSeverity::Moderate,
            PatchSeverity::Low,
        ];
        for s in variants {
            let json = serde_json::to_string(&s).unwrap();
            let deserialized: PatchSeverity = serde_json::from_str(&json).unwrap();
            let json2 = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, json2);
        }
    }

    // ============================================================
    // PatchManager edge cases
    // ============================================================

    #[test]
    fn test_patch_manager_new_dnf() {
        let pm = PatchManager::new(PackageManager::Dnf);
        assert!(!pm.auto_reboot);
        assert!(pm.exclude_packages.is_empty());
    }

    #[test]
    fn test_patch_manager_new_zypper() {
        let pm = PatchManager::new(PackageManager::Zypper);
        assert!(!pm.auto_reboot);
        assert!(pm.exclude_packages.is_empty());
    }

    #[test]
    fn test_build_update_command_dnf_all() {
        let pm = PatchManager::new(PackageManager::Dnf);
        assert_eq!(pm.build_update_command(false), "dnf update -y");
    }

    #[test]
    fn test_build_check_command_dnf() {
        let pm = PatchManager::new(PackageManager::Dnf);
        assert_eq!(pm.build_check_command(), "dnf check-update --security");
    }

    #[test]
    fn test_build_reboot_check_command_dnf() {
        let pm = PatchManager::new(PackageManager::Dnf);
        assert_eq!(pm.build_reboot_check_command(), "needs-restarting -r");
    }

    #[test]
    fn test_yum_and_dnf_same_commands() {
        let yum = PatchManager::new(PackageManager::Yum);
        let dnf = PatchManager::new(PackageManager::Dnf);
        assert_eq!(yum.build_update_command(true), dnf.build_update_command(true));
        assert_eq!(yum.build_update_command(false), dnf.build_update_command(false));
        assert_eq!(yum.build_check_command(), dnf.build_check_command());
        assert_eq!(yum.build_reboot_check_command(), dnf.build_reboot_check_command());
    }
}
