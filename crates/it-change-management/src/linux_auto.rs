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
}
