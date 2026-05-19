//! 数据模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 自动化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxAutomationConfig {
    /// 数据库路径
    pub database_path: std::path::PathBuf,
    /// Ansible playbook 目录
    pub playbook_dir: std::path::PathBuf,
    /// SSH 密钥路径
    pub ssh_key_path: Option<std::path::PathBuf>,
    /// 日志目录
    pub log_dir: std::path::PathBuf,
    /// 目标服务器列表
    pub target_hosts: Vec<String>,
    /// 合规扫描工具
    pub compliance_tool: ComplianceTool,
}

/// 合规扫描工具
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplianceTool {
    OpenScap,
    Lynis,
    CisCat,
    Custom(String),
}

/// 自动化任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTask {
    pub id: Uuid,
    pub task_type: TaskType,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub priority: TaskPriority,
}

/// 任务类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    ComplianceScan { profile: String, hosts: Vec<String> },
    PatchInstall { packages: Vec<String>, hosts: Vec<String> },
    ConfigDeploy { playbook: String, hosts: Vec<String> },
    SecurityUpdate { hosts: Vec<String> },
    LogCollection { hosts: Vec<String>, log_paths: Vec<String> },
    DiskCleanup { hosts: Vec<String>, targets: Vec<String> },
    ServiceRestart { service: String, hosts: Vec<String> },
    CustomCommand { command: String, hosts: Vec<String> },
}

/// 任务优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
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

/// 自动化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationResult {
    pub task_id: Uuid,
    pub task_type: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub host_results: Vec<HostResult>,
    pub summary: String,
    pub audit_trail: Vec<AuditEntry>,
}

/// 单主机结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostResult {
    pub host: String,
    pub status: TaskStatus,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub user: String,
    pub action: String,
    pub target: String,
    pub result: String,
    pub details: Option<String>,
    pub signature: Option<String>,
}

/// 合规报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub host: String,
    pub scan_time: DateTime<Utc>,
    pub profile: String,
    pub score: f64,
    pub passed: usize,
    pub failed: usize,
    pub not_applicable: usize,
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
    pub remediation: Option<String>,
}

/// 严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// 发现状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FindingStatus {
    Pass,
    Fail,
    NotApplicable,
    NotChecked,
}

/// 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationStatistics {
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub failed_tasks: usize,
    pub pending_tasks: usize,
    pub compliance_score: f64,
    pub audit_entries: usize,
    pub last_scan_time: Option<DateTime<Utc>>,
}
