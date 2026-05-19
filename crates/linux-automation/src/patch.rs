//! 补丁管理模块
//! 支持 yum/apt 包管理器的安全补丁

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::{DateTime, Utc};
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
            PackageManager::Yum | PackageManager::Dnf => {
                "needs-restarting -r".to_string()
            }
            PackageManager::Apt => {
                "[ -f /var/run/reboot-required ] && echo 'REBOOT_REQUIRED' || echo 'NO_REBOOT'".to_string()
            }
            PackageManager::Zypper => "zypper needs-rebooting".to_string(),
        }
    }
}
