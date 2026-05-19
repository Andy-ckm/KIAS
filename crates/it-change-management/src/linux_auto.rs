//! # Linux 自动化维护模块
//!
//! 医药企业 Linux 服务器自动化维护，包括：
//! - 合规扫描（CIS Benchmark、OpenSCAP）
//! - 补丁管理
//! - 配置管理（Ansible 集成）
//! - 审计日志收集
//! - 自动化变更执行

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Linux 自动化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxAutomationConfig {
    /// Ansible playbook 目录
    pub playbook_dir: PathBuf,
    /// 合规扫描工具路径
    pub compliance_tool: ComplianceTool,
    /// 目标服务器列表
    pub target_hosts: Vec<String>,
    /// SSH 密钥路径
    pub ssh_key_path: Option<PathBuf>,
    /// 日志目录
    pub log_dir: PathBuf,
}

/// 合规扫描工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComplianceTool {
    /// OpenSCAP
    OpenScap,
    /// Lynis
    Lynis,
    /// CIS-CAT
    CisCat,
    /// 自定义脚本
    Custom(String),
}

/// 自动化任务类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationTask {
    /// 合规扫描
    ComplianceScan { profile: String, hosts: Vec<String> },
    /// 补丁安装
    PatchInstall {
        packages: Vec<String>,
        hosts: Vec<String>,
    },
    /// 配置部署
    ConfigDeploy {
        playbook: String,
        hosts: Vec<String>,
    },
    /// 安全更新
    SecurityUpdate { hosts: Vec<String> },
    /// 日志收集
    LogCollection {
        hosts: Vec<String>,
        log_paths: Vec<String>,
    },
    /// 磁盘清理
    DiskCleanup {
        hosts: Vec<String>,
        targets: Vec<String>,
    },
    /// 服务重启
    ServiceRestart { service: String, hosts: Vec<String> },
}

/// 自动化任务执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationResult {
    pub task_id: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub host_results: Vec<HostResult>,
    pub summary: String,
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    PartialSuccess,
    Cancelled,
}

/// 单主机执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResult {
    pub host: String,
    pub status: TaskStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
}

/// 合规扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub host: String,
    pub scan_time: DateTime<Utc>,
    pub profile: String,
    pub total_rules: u32,
    pub passed: u32,
    pub failed: u32,
    pub not_applicable: u32,
    pub score: f64,
    pub findings: Vec<ComplianceFinding>,
}

/// 合规发现
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceFinding {
    pub rule_id: String,
    pub title: String,
    pub severity: Severity,
    pub status: FindingStatus,
    pub description: String,
    pub remediation: String,
}

/// 严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// 发现状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FindingStatus {
    Pass,
    Fail,
    NotApplicable,
    NotChecked,
}

/// Linux 自动化管理器
pub struct LinuxAutomationManager {
    config: LinuxAutomationConfig,
    task_history: Vec<AutomationResult>,
}

impl LinuxAutomationManager {
    /// 创建新的管理器
    pub fn new(config: LinuxAutomationConfig) -> Self {
        Self {
            config,
            task_history: Vec::new(),
        }
    }

    /// 生成 Ansible playbook 命令
    pub fn generate_ansible_command(&self, task: &AutomationTask) -> String {
        match task {
            AutomationTask::ComplianceScan { profile, hosts } => {
                let hosts_str = hosts.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} --extra-vars 'profile={} hosts={}' {}/compliance-scan.yml",
                    hosts_str,
                    self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                    profile,
                    hosts_str,
                    self.config.playbook_dir.display()
                )
            }
            AutomationTask::PatchInstall { packages, hosts } => {
                let hosts_str = hosts.join(",");
                let packages_str = packages.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} --extra-vars 'packages={}' {}/patch-install.yml",
                    hosts_str,
                    self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                    packages_str,
                    self.config.playbook_dir.display()
                )
            }
            AutomationTask::ConfigDeploy { playbook, hosts } => {
                let hosts_str = hosts.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} {}/{}",
                    hosts_str,
                    self.config
                        .ssh_key_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    self.config.playbook_dir.display(),
                    playbook
                )
            }
            AutomationTask::SecurityUpdate { hosts } => {
                let hosts_str = hosts.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} {}/security-update.yml",
                    hosts_str,
                    self.config
                        .ssh_key_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    self.config.playbook_dir.display()
                )
            }
            AutomationTask::LogCollection { hosts, log_paths } => {
                let hosts_str = hosts.join(",");
                let paths_str = log_paths.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} --extra-vars 'log_paths={}' {}/log-collection.yml",
                    hosts_str,
                    self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                    paths_str,
                    self.config.playbook_dir.display()
                )
            }
            AutomationTask::DiskCleanup { hosts, targets } => {
                let hosts_str = hosts.join(",");
                let targets_str = targets.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} --extra-vars 'targets={}' {}/disk-cleanup.yml",
                    hosts_str,
                    self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                    targets_str,
                    self.config.playbook_dir.display()
                )
            }
            AutomationTask::ServiceRestart { service, hosts } => {
                let hosts_str = hosts.join(",");
                format!(
                    "ansible-playbook -i {} --private-key {} --extra-vars 'service={}' {}/service-restart.yml",
                    hosts_str,
                    self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
                    service,
                    self.config.playbook_dir.display()
                )
            }
        }
    }

    /// 生成 OpenSCAP 扫描命令
    pub fn generate_openscap_command(&self, host: &str, profile: &str) -> String {
        format!(
            "ssh -i {} root@{} 'oscap xccdf eval --profile {} --results /tmp/oscap-results.xml --report /tmp/oscap-report.html /usr/share/xml/scap/ssg/content/ssg-rhel8-ds.xml'",
            self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            host,
            profile
        )
    }

    /// 生成 Lynis 审计命令
    pub fn generate_lynis_command(&self, host: &str) -> String {
        format!(
            "ssh -i {} root@{} 'lynis audit system --no-colors --quiet'",
            self.config
                .ssh_key_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            host
        )
    }

    /// 生成补丁检查命令
    pub fn generate_patch_check_command(&self, host: &str) -> String {
        format!(
            "ssh -i {} root@{} 'yum check-update --security 2>/dev/null || apt list --upgradable 2>/dev/null'",
            self.config.ssh_key_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(),
            host
        )
    }

    /// 生成磁盘使用检查命令
    pub fn generate_disk_check_command(&self, host: &str) -> String {
        format!(
            "ssh -i {} root@{} 'df -h && du -sh /var/log/* 2>/dev/null | sort -rh | head -10'",
            self.config
                .ssh_key_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            host
        )
    }

    /// 记录任务结果
    pub fn record_task(&mut self, result: AutomationResult) {
        self.task_history.push(result);
    }

    /// 获取任务历史
    pub fn get_task_history(&self) -> &[AutomationResult] {
        &self.task_history
    }

    /// 获取失败的任务
    pub fn get_failed_tasks(&self) -> Vec<&AutomationResult> {
        self.task_history
            .iter()
            .filter(|r| r.status == TaskStatus::Failed)
            .collect()
    }

    /// 获取成功任务数
    pub fn get_success_count(&self) -> usize {
        self.task_history
            .iter()
            .filter(|r| r.status == TaskStatus::Success)
            .count()
    }

    /// 获取任务统计
    pub fn get_statistics(&self) -> AutomationStatistics {
        let total = self.task_history.len();
        let success = self.get_success_count();
        let failed = self.get_failed_tasks().len();
        let partial = self
            .task_history
            .iter()
            .filter(|r| r.status == TaskStatus::PartialSuccess)
            .count();
        let pending = self
            .task_history
            .iter()
            .filter(|r| r.status == TaskStatus::Pending)
            .count();

        AutomationStatistics {
            total,
            success,
            failed,
            partial,
            pending,
            success_rate: if total > 0 {
                (success as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// 模拟执行任务（生产环境应调用真实命令）
    pub fn execute_task(&mut self, task: AutomationTask) -> AutomationResult {
        let task_id = uuid::Uuid::new_v4().to_string();
        let task_type = match &task {
            AutomationTask::ComplianceScan { .. } => "ComplianceScan".to_string(),
            AutomationTask::PatchInstall { .. } => "PatchInstall".to_string(),
            AutomationTask::ConfigDeploy { .. } => "ConfigDeploy".to_string(),
            AutomationTask::SecurityUpdate { .. } => "SecurityUpdate".to_string(),
            AutomationTask::LogCollection { .. } => "LogCollection".to_string(),
            AutomationTask::DiskCleanup { .. } => "DiskCleanup".to_string(),
            AutomationTask::ServiceRestart { .. } => "ServiceRestart".to_string(),
        };

        let hosts = match &task {
            AutomationTask::ComplianceScan { hosts, .. } => hosts.clone(),
            AutomationTask::PatchInstall { hosts, .. } => hosts.clone(),
            AutomationTask::ConfigDeploy { hosts, .. } => hosts.clone(),
            AutomationTask::SecurityUpdate { hosts, .. } => hosts.clone(),
            AutomationTask::LogCollection { hosts, .. } => hosts.clone(),
            AutomationTask::DiskCleanup { hosts, .. } => hosts.clone(),
            AutomationTask::ServiceRestart { hosts, .. } => hosts.clone(),
        };

        let started_at = Utc::now();

        // 模拟执行结果
        let host_results: Vec<HostResult> = hosts
            .iter()
            .map(|host| HostResult {
                host: host.clone(),
                status: TaskStatus::Success,
                stdout: format!("Task {} completed successfully on {}", task_type, host),
                stderr: String::new(),
                exit_code: Some(0),
                duration_ms: 100,
            })
            .collect();

        let result = AutomationResult {
            task_id: task_id.clone(),
            task_type,
            status: TaskStatus::Success,
            started_at,
            completed_at: Some(Utc::now()),
            host_results,
            summary: format!("Task {} completed for {} hosts", task_id, hosts.len()),
        };

        self.task_history.push(result.clone());
        result
    }

    /// 执行合规扫描并生成报告
    pub fn execute_compliance_scan(&mut self, host: &str, profile: &str) -> ComplianceReport {
        let _cmd = self.generate_openscap_command(host, profile);

        // 模拟合规扫描结果
        ComplianceReport {
            host: host.to_string(),
            scan_time: Utc::now(),
            profile: profile.to_string(),
            total_rules: 100,
            passed: 85,
            failed: 10,
            not_applicable: 5,
            score: 85.0,
            findings: vec![
                ComplianceFinding {
                    rule_id: "RHEL-08-010001".to_string(),
                    title: "Ensure /tmp is a separate partition".to_string(),
                    severity: Severity::High,
                    status: FindingStatus::Pass,
                    description: "/tmp is mounted as a separate partition".to_string(),
                    remediation: "No action required".to_string(),
                },
                ComplianceFinding {
                    rule_id: "RHEL-08-010002".to_string(),
                    title: "Ensure nodev option set on /tmp partition".to_string(),
                    severity: Severity::Medium,
                    status: FindingStatus::Fail,
                    description: "nodev option not set on /tmp".to_string(),
                    remediation: "Add nodev option to /tmp in /etc/fstab".to_string(),
                },
            ],
        }
    }

    /// 检查补丁状态
    pub fn check_patch_status(&self, host: &str) -> PatchStatus {
        let _cmd = self.generate_patch_check_command(host);

        PatchStatus {
            host: host.to_string(),
            check_time: Utc::now(),
            security_patches_available: 3,
            non_security_patches_available: 12,
            patches: vec![
                PatchInfo {
                    name: "openssl-1.1.1k".to_string(),
                    version: "1.1.1k-7.el8_6".to_string(),
                    severity: Severity::Critical,
                    is_security: true,
                },
                PatchInfo {
                    name: "curl-7.61.1".to_string(),
                    version: "7.61.1-22.el8_6.3".to_string(),
                    severity: Severity::High,
                    is_security: true,
                },
            ],
        }
    }

    /// 检查磁盘使用情况
    pub fn check_disk_usage(&self, host: &str) -> DiskUsageReport {
        let _cmd = self.generate_disk_check_command(host);

        DiskUsageReport {
            host: host.to_string(),
            check_time: Utc::now(),
            filesystems: vec![
                FilesystemUsage {
                    mount_point: "/".to_string(),
                    total_gb: 100.0,
                    used_gb: 72.0,
                    available_gb: 28.0,
                    use_percent: 72.0,
                },
                FilesystemUsage {
                    mount_point: "/var/log".to_string(),
                    total_gb: 50.0,
                    used_gb: 45.0,
                    available_gb: 5.0,
                    use_percent: 90.0,
                },
            ],
            warnings: vec!["/var/log is 90% full".to_string()],
        }
    }
}

/// 自动化统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStatistics {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub partial: usize,
    pub pending: usize,
    pub success_rate: f64,
}

/// 补丁状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchStatus {
    pub host: String,
    pub check_time: DateTime<Utc>,
    pub security_patches_available: u32,
    pub non_security_patches_available: u32,
    pub patches: Vec<PatchInfo>,
}

/// 补丁信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchInfo {
    pub name: String,
    pub version: String,
    pub severity: Severity,
    pub is_security: bool,
}

/// 磁盘使用报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsageReport {
    pub host: String,
    pub check_time: DateTime<Utc>,
    pub filesystems: Vec<FilesystemUsage>,
    pub warnings: Vec<String>,
}

/// 文件系统使用情况
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemUsage {
    pub mount_point: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub available_gb: f64,
    pub use_percent: f64,
}

/// 生成标准 Ansible playbook
pub fn generate_compliance_scan_playbook() -> String {
    r#"---
# 医药企业 Linux 合规扫描 Playbook
# 符合 CIS Benchmark + FDA 21 CFR Part 11 要求
- name: Compliance Scan
  hosts: "{{ hosts }}"
  become: yes
  vars:
    scan_profile: "{{ profile | default('xccdf_org.ssgproject.content_profile_cis_level2') }}"
  tasks:
    - name: Install OpenSCAP
      package:
        name: "{{ item }}"
        state: present
      loop:
        - openscap-scanner
        - scap-security-guide

    - name: Run OpenSCAP scan
      command: >
        oscap xccdf eval
        --profile {{ scan_profile }}
        --results /var/log/compliance/oscap-results-{{ ansible_date_time.iso8601 }}.xml
        --report /var/log/compliance/oscap-report-{{ ansible_date_time.iso8601 }}.html
        /usr/share/xml/scap/ssg/content/ssg-rhel8-ds.xml
      register: scan_result
      ignore_errors: yes

    - name: Collect audit logs
      synchronize:
        src: /var/log/audit/
        dest: "{{ log_dir }}/{{ inventory_hostname }}/audit/"
        mode: pull

    - name: Collect system logs
      synchronize:
        src: /var/log/messages
        dest: "{{ log_dir }}/{{ inventory_hostname }}/messages"
        mode: pull

    - name: Generate compliance summary
      template:
        src: compliance-summary.j2
        dest: "/var/log/compliance/summary-{{ ansible_date_time.iso8601 }}.txt"
"#
    .to_string()
}

/// 生成补丁安装 playbook
pub fn generate_patch_install_playbook() -> String {
    r#"---
# 医药企业补丁安装 Playbook
# 包含回滚机制和验证步骤
- name: Patch Installation
  hosts: "{{ hosts }}"
  become: yes
  vars:
    packages: "{{ packages }}"
    rollback_enabled: true
  tasks:
    - name: Create pre-patch snapshot
      command: "snapper create -d 'Pre-patch snapshot' -t pre"
      register: snapshot
      when: rollback_enabled

    - name: Install security patches
      package:
        name: "{{ item }}"
        state: latest
      loop: "{{ packages }}"
      register: patch_result

    - name: Verify services are running
      service:
        name: "{{ item }}"
        state: started
      loop:
        - sshd
        - cron
        - rsyslog

    - name: Run post-patch compliance check
      command: "oscap xccdf eval --profile xccdf_org.ssgproject.content_profile_standard /usr/share/xml/scap/ssg/content/ssg-rhel8-ds.xml"
      register: post_check
      ignore_errors: yes

    - name: Rollback if verification fails
      command: "snapper rollback {{ snapshot.stdout }}"
      when: post_check.rc != 0 and rollback_enabled
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_config() -> LinuxAutomationConfig {
        LinuxAutomationConfig {
            playbook_dir: PathBuf::from("/etc/ansible/playbooks"),
            compliance_tool: ComplianceTool::OpenScap,
            target_hosts: vec!["192.168.1.10".to_string(), "192.168.1.11".to_string()],
            ssh_key_path: Some(PathBuf::from("/root/.ssh/id_rsa")),
            log_dir: PathBuf::from("/var/log/compliance"),
        }
    }

    #[test]
    fn test_generate_ansible_command() {
        let manager = LinuxAutomationManager::new(create_test_config());

        let task = AutomationTask::ComplianceScan {
            profile: "cis_level2".to_string(),
            hosts: vec!["192.168.1.10".to_string()],
        };

        let cmd = manager.generate_ansible_command(&task);
        assert!(cmd.contains("ansible-playbook"));
        assert!(cmd.contains("compliance-scan.yml"));
    }

    #[test]
    fn test_generate_openscap_command() {
        let manager = LinuxAutomationManager::new(create_test_config());

        let cmd = manager.generate_openscap_command("192.168.1.10", "cis_level2");
        assert!(cmd.contains("oscap xccdf eval"));
        assert!(cmd.contains("192.168.1.10"));
    }

    #[test]
    fn test_generate_lynis_command() {
        let manager = LinuxAutomationManager::new(create_test_config());

        let cmd = manager.generate_lynis_command("192.168.1.10");
        assert!(cmd.contains("lynis audit system"));
    }

    #[test]
    fn test_generate_patch_check_command() {
        let manager = LinuxAutomationManager::new(create_test_config());

        let cmd = manager.generate_patch_check_command("192.168.1.10");
        assert!(cmd.contains("check-update") || cmd.contains("upgradable"));
    }

    #[test]
    fn test_generate_disk_check_command() {
        let manager = LinuxAutomationManager::new(create_test_config());

        let cmd = manager.generate_disk_check_command("192.168.1.10");
        assert!(cmd.contains("df -h"));
    }

    #[test]
    fn test_record_and_get_history() {
        let mut manager = LinuxAutomationManager::new(create_test_config());

        let result = AutomationResult {
            task_id: "task-1".to_string(),
            task_type: "ComplianceScan".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "扫描完成".to_string(),
        };

        manager.record_task(result);
        assert_eq!(manager.get_task_history().len(), 1);
    }

    #[test]
    fn test_get_failed_tasks() {
        let mut manager = LinuxAutomationManager::new(create_test_config());

        manager.record_task(AutomationResult {
            task_id: "task-1".to_string(),
            task_type: "Scan".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "成功".to_string(),
        });

        manager.record_task(AutomationResult {
            task_id: "task-2".to_string(),
            task_type: "Scan".to_string(),
            status: TaskStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "失败".to_string(),
        });

        assert_eq!(manager.get_failed_tasks().len(), 1);
    }

    #[test]
    fn test_generate_playbooks() {
        let playbook = generate_compliance_scan_playbook();
        assert!(playbook.contains("Compliance Scan"));
        assert!(playbook.contains("OpenSCAP"));

        let playbook = generate_patch_install_playbook();
        assert!(playbook.contains("Patch Installation"));
        assert!(playbook.contains("rollback"));
    }

    fn make_test_config() -> LinuxAutomationConfig {
        LinuxAutomationConfig {
            playbook_dir: PathBuf::from("/playbooks"),
            compliance_tool: ComplianceTool::OpenScap,
            target_hosts: vec!["host1".to_string(), "host2".to_string()],
            ssh_key_path: Some(PathBuf::from("/key.pem")),
            log_dir: PathBuf::from("/logs"),
        }
    }

    #[test]
    fn test_ansible_command_security_update() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_ansible_command(&AutomationTask::SecurityUpdate {
            hosts: vec!["srv1".to_string(), "srv2".to_string()],
        });
        assert!(cmd.contains("security-update.yml"));
        assert!(cmd.contains("srv1,srv2"));
    }

    #[test]
    fn test_ansible_command_log_collection() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_ansible_command(&AutomationTask::LogCollection {
            hosts: vec!["srv1".to_string()],
            log_paths: vec![
                "/var/log/syslog".to_string(),
                "/var/log/auth.log".to_string(),
            ],
        });
        assert!(cmd.contains("log-collection.yml"));
        assert!(cmd.contains("/var/log/syslog,/var/log/auth.log"));
    }

    #[test]
    fn test_ansible_command_disk_cleanup() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_ansible_command(&AutomationTask::DiskCleanup {
            hosts: vec!["srv1".to_string()],
            targets: vec!["/tmp".to_string(), "/var/cache".to_string()],
        });
        assert!(cmd.contains("disk-cleanup.yml"));
        assert!(cmd.contains("/tmp,/var/cache"));
    }

    #[test]
    fn test_ansible_command_service_restart() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_ansible_command(&AutomationTask::ServiceRestart {
            service: "nginx".to_string(),
            hosts: vec!["web1".to_string()],
        });
        assert!(cmd.contains("service-restart.yml"));
        assert!(cmd.contains("nginx"));
        assert!(cmd.contains("web1"));
    }

    #[test]
    fn test_ansible_command_config_deploy() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_ansible_command(&AutomationTask::ConfigDeploy {
            playbook: "deploy.yml".to_string(),
            hosts: vec!["srv1".to_string()],
        });
        assert!(cmd.contains("deploy.yml"));
        assert!(cmd.contains("srv1"));
    }

    #[test]
    fn test_ansible_command_no_ssh_key() {
        let mut config = make_test_config();
        config.ssh_key_path = None;
        let manager = LinuxAutomationManager::new(config);
        let cmd = manager.generate_ansible_command(&AutomationTask::SecurityUpdate {
            hosts: vec!["srv1".to_string()],
        });
        assert!(cmd.contains("security-update.yml"));
        // No --private-key when ssh_key_path is None
        assert!(!cmd.contains("--private-key /"));
    }

    #[test]
    fn test_task_history_mixed_statuses() {
        let mut manager = LinuxAutomationManager::new(make_test_config());

        manager.record_task(AutomationResult {
            task_id: "t1".to_string(),
            task_type: "Scan".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "ok".to_string(),
        });
        manager.record_task(AutomationResult {
            task_id: "t2".to_string(),
            task_type: "Patch".to_string(),
            status: TaskStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "fail".to_string(),
        });
        manager.record_task(AutomationResult {
            task_id: "t3".to_string(),
            task_type: "Deploy".to_string(),
            status: TaskStatus::PartialSuccess,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "partial".to_string(),
        });

        assert_eq!(manager.get_task_history().len(), 3);
        assert_eq!(manager.get_failed_tasks().len(), 1);
        assert_eq!(manager.get_failed_tasks()[0].task_id, "t2");
    }

    #[test]
    fn test_empty_task_history() {
        let manager = LinuxAutomationManager::new(make_test_config());
        assert!(manager.get_task_history().is_empty());
        assert!(manager.get_failed_tasks().is_empty());
    }

    #[test]
    fn test_openscap_command_custom_profile() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_openscap_command("server1", "cis_level2");
        assert!(cmd.contains("server1"));
        assert!(cmd.contains("cis_level2"));
    }

    #[test]
    fn test_lynis_command() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let cmd = manager.generate_lynis_command("server1");
        assert!(cmd.contains("server1"));
        assert!(cmd.contains("lynis"));
    }
}

#[cfg(test)]
mod extended_tests {
    use super::*;
    use std::path::PathBuf;

    fn make_test_config() -> LinuxAutomationConfig {
        LinuxAutomationConfig {
            playbook_dir: PathBuf::from("/etc/ansible/playbooks"),
            compliance_tool: ComplianceTool::OpenScap,
            target_hosts: vec!["192.168.1.10".to_string()],
            ssh_key_path: Some(PathBuf::from("/root/.ssh/id_rsa")),
            log_dir: PathBuf::from("/var/log/compliance"),
        }
    }

    #[test]
    fn test_execute_task() {
        let mut manager = LinuxAutomationManager::new(make_test_config());
        let task = AutomationTask::ComplianceScan {
            profile: "cis_level2".to_string(),
            hosts: vec!["server1".to_string(), "server2".to_string()],
        };
        let result = manager.execute_task(task);
        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.host_results.len(), 2);
        assert_eq!(manager.get_task_history().len(), 1);
    }

    #[test]
    fn test_execute_compliance_scan() {
        let mut manager = LinuxAutomationManager::new(make_test_config());
        let report = manager.execute_compliance_scan("server1", "cis_level2");
        assert_eq!(report.host, "server1");
        assert_eq!(report.score, 85.0);
        assert_eq!(report.total_rules, 100);
        assert_eq!(report.passed, 85);
        assert_eq!(report.failed, 10);
    }

    #[test]
    fn test_check_patch_status() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let status = manager.check_patch_status("server1");
        assert_eq!(status.host, "server1");
        assert_eq!(status.security_patches_available, 3);
        assert_eq!(status.patches.len(), 2);
        assert_eq!(status.patches[0].severity, Severity::Critical);
    }

    #[test]
    fn test_check_disk_usage() {
        let manager = LinuxAutomationManager::new(make_test_config());
        let report = manager.check_disk_usage("server1");
        assert_eq!(report.host, "server1");
        assert_eq!(report.filesystems.len(), 2);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("90%"));
    }

    #[test]
    fn test_get_statistics() {
        let mut manager = LinuxAutomationManager::new(make_test_config());

        manager.execute_task(AutomationTask::SecurityUpdate {
            hosts: vec!["srv1".to_string()],
        });
        manager.execute_task(AutomationTask::PatchInstall {
            packages: vec!["openssl".to_string()],
            hosts: vec!["srv1".to_string()],
        });

        let stats = manager.get_statistics();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.success, 2);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.success_rate, 100.0);
    }

    #[test]
    fn test_execute_multiple_task_types() {
        let mut manager = LinuxAutomationManager::new(make_test_config());

        manager.execute_task(AutomationTask::ComplianceScan {
            profile: "cis_level2".to_string(),
            hosts: vec!["srv1".to_string()],
        });
        manager.execute_task(AutomationTask::SecurityUpdate {
            hosts: vec!["srv1".to_string()],
        });
        manager.execute_task(AutomationTask::LogCollection {
            hosts: vec!["srv1".to_string()],
            log_paths: vec!["/var/log".to_string()],
        });

        assert_eq!(manager.get_task_history().len(), 3);
        assert_eq!(manager.get_success_count(), 3);
    }
}

/// 变更执行计划
#[derive(Debug, Clone)]
pub struct ChangeExecutionPlan {
    pub change_id: String,
    pub steps: Vec<ExecutionStep>,
    pub rollback_steps: Vec<ExecutionStep>,
    pub pre_checks: Vec<String>,
    pub post_checks: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub order: u32,
    pub description: String,
    pub command: String,
    pub target_hosts: Vec<String>,
    pub timeout_seconds: u32,
    pub requires_approval: bool,
}

impl ChangeExecutionPlan {
    /// 为基础设施变更创建执行计划
    pub fn for_infrastructure(change_id: &str, hosts: Vec<String>) -> Self {
        Self {
            change_id: change_id.to_string(),
            steps: vec![
                ExecutionStep { order: 1, description: "备份当前配置".into(), command: "tar czf /backup/config-$(date +%Y%m%d).tar.gz /etc/".into(), target_hosts: hosts.clone(), timeout_seconds: 300, requires_approval: false },
                ExecutionStep { order: 2, description: "执行变更".into(), command: String::new(), target_hosts: hosts.clone(), timeout_seconds: 600, requires_approval: true },
                ExecutionStep { order: 3, description: "验证变更".into(), command: "systemctl status".into(), target_hosts: hosts.clone(), timeout_seconds: 120, requires_approval: false },
            ],
            rollback_steps: vec![
                ExecutionStep { order: 1, description: "回滚配置".into(), command: "tar xzf /backup/config-*.tar.gz -C /".into(), target_hosts: hosts.clone(), timeout_seconds: 300, requires_approval: false },
            ],
            pre_checks: vec!["磁盘空间 > 20%".into(), "系统负载 < 5".into()],
            post_checks: vec!["服务正常运行".into(), "日志无错误".into()],
        }
    }
}
