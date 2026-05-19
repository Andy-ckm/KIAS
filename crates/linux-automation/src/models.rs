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
    ComplianceScan {
        profile: String,
        hosts: Vec<String>,
    },
    PatchInstall {
        packages: Vec<String>,
        hosts: Vec<String>,
    },
    ConfigDeploy {
        playbook: String,
        hosts: Vec<String>,
    },
    SecurityUpdate {
        hosts: Vec<String>,
    },
    LogCollection {
        hosts: Vec<String>,
        log_paths: Vec<String>,
    },
    DiskCleanup {
        hosts: Vec<String>,
        targets: Vec<String>,
    },
    ServiceRestart {
        service: String,
        hosts: Vec<String>,
    },
    CustomCommand {
        command: String,
        hosts: Vec<String>,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_tool_variants() {
        let tools = vec![
            ComplianceTool::OpenScap,
            ComplianceTool::Lynis,
            ComplianceTool::CisCat,
            ComplianceTool::Custom("custom".to_string()),
        ];
        assert_eq!(tools.len(), 4);
        assert_eq!(tools[0], ComplianceTool::OpenScap);
        assert_ne!(tools[0], tools[1]);
    }

    #[test]
    fn test_task_priority_ordering() {
        let priorities = vec![
            TaskPriority::Low,
            TaskPriority::Normal,
            TaskPriority::High,
            TaskPriority::Critical,
        ];
        assert_eq!(priorities.len(), 4);
        assert_eq!(priorities[0], TaskPriority::Low);
        assert_ne!(priorities[0], priorities[3]);
    }

    #[test]
    fn test_task_status_variants() {
        let statuses = vec![
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Success,
            TaskStatus::Failed,
            TaskStatus::PartialSuccess,
            TaskStatus::Cancelled,
        ];
        assert_eq!(statuses.len(), 6);
        assert_eq!(statuses[0], TaskStatus::Pending);
        assert_ne!(statuses[0], statuses[2]);
    }

    #[test]
    fn test_severity_variants() {
        let severities = vec![
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ];
        assert_eq!(severities.len(), 4);
        assert_eq!(severities[0], Severity::Low);
        assert_ne!(severities[0], severities[3]);
    }

    #[test]
    fn test_finding_status_variants() {
        let statuses = vec![
            FindingStatus::Pass,
            FindingStatus::Fail,
            FindingStatus::NotApplicable,
            FindingStatus::NotChecked,
        ];
        assert_eq!(statuses.len(), 4);
        assert_eq!(statuses[0], FindingStatus::Pass);
        assert_ne!(statuses[0], statuses[1]);
    }

    #[test]
    fn test_automation_task_creation() {
        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test".to_string(),
            priority: TaskPriority::Normal,
        };
        assert_eq!(task.created_by, "test");
        assert_eq!(task.priority, TaskPriority::Normal);
    }

    #[test]
    fn test_host_result_creation() {
        let result = HostResult {
            host: "localhost".to_string(),
            status: TaskStatus::Success,
            stdout: "ok".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 100,
        };
        assert_eq!(result.host, "localhost");
        assert_eq!(result.status, TaskStatus::Success);
        assert_eq!(result.exit_code, 0);
    }
}
