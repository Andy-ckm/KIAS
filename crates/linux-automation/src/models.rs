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
    /// 日常巡检 (R023)
    HealthCheck {
        hosts: Vec<String>,
        checks: Vec<HealthCheckType>,
    },
    /// 服务器初始化 (R024)
    ServerProvision {
        hosts: Vec<String>,
        template: ProvisionTemplate,
    },
    /// Docker 运维 (R025)
    DockerOps {
        hosts: Vec<String>,
        action: DockerAction,
    },
    /// K8s 运维 (R026)
    K8sOps {
        context: String,
        action: K8sAction,
    },
    /// 备份恢复 (R029)
    BackupOps {
        hosts: Vec<String>,
        action: BackupAction,
    },
    /// 用户和权限管理 (R031)
    UserManage {
        hosts: Vec<String>,
        action: UserAction,
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

// ============================================================
// R023: 日常巡检数据模型
// ============================================================

/// 巡检类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthCheckType {
    Cpu,
    Memory,
    Disk,
    Process,
    Log,
    Network,
    Security,
    All,
}

/// 巡检报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckReport {
    pub host: String,
    pub check_time: DateTime<Utc>,
    pub overall_status: HealthStatus,
    pub checks: Vec<HealthCheckItem>,
    pub recommendations: Vec<String>,
}

/// 单项巡检结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckItem {
    pub check_type: HealthCheckType,
    pub status: HealthStatus,
    pub metric_name: String,
    pub metric_value: String,
    pub threshold: Option<String>,
    pub message: String,
}

/// 健康状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

// ============================================================
// R024: 服务器初始化数据模型
// ============================================================

/// 初始化模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionTemplate {
    pub name: String,
    pub steps: Vec<ProvisionStep>,
}

/// 初始化步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionStep {
    pub name: String,
    pub step_type: ProvisionStepType,
    pub required: bool,
}

/// 初始化步骤类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProvisionStepType {
    SystemUpdate,
    InstallPackages {
        packages: Vec<String>,
    },
    CreateUser {
        username: String,
        ssh_key: Option<String>,
    },
    SudoConfig {
        username: String,
        rules: Vec<String>,
    },
    SshHardening {
        config: SshConfig,
    },
    Firewall {
        rules: Vec<FirewallRule>,
    },
    Timezone {
        tz: String,
    },
    NtpServer {
        server: String,
    },
    KernelParams {
        params: Vec<(String, String)>,
    },
    ServiceManagement {
        enable: Vec<String>,
        disable: Vec<String>,
    },
    CustomScript {
        script: String,
    },
}

/// SSH 加固配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SshConfig {
    pub permit_root_login: bool,
    pub password_auth: bool,
    pub max_auth_tries: u32,
    pub port: u16,
}

/// 防火墙规则
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewallRule {
    pub port: u16,
    pub protocol: String,
    pub action: String,
    pub source: Option<String>,
}

/// 初始化报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionReport {
    pub host: String,
    pub template_name: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub step_results: Vec<ProvisionStepResult>,
    pub overall_status: TaskStatus,
}

/// 单步初始化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionStepResult {
    pub step_name: String,
    pub status: TaskStatus,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

// ============================================================
// R025: Docker 运维数据模型
// ============================================================

/// Docker 操作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DockerAction {
    /// 列出容器
    ListContainers { all: bool },
    /// 容器状态
    ContainerStatus { container: String },
    /// 启动容器
    Start { container: String },
    /// 停止容器
    Stop { container: String },
    /// 重启容器
    Restart { container: String },
    /// 删除容器
    Remove { container: String, force: bool },
    /// 查看日志
    Logs { container: String, tail: u32 },
    /// 资源监控
    Stats,
    /// 清理(悬挂镜像/停止容器/未使用volume)
    Prune {
        images: bool,
        containers: bool,
        volumes: bool,
    },
    /// 镜像列表
    ListImages,
    /// 拉取镜像
    Pull { image: String },
}

/// Docker 容器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub ports: String,
    pub created: String,
    pub cpu_percent: Option<f64>,
    pub mem_usage: Option<String>,
}

/// Docker 操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerOpsResult {
    pub host: String,
    pub action: DockerAction,
    pub status: TaskStatus,
    pub containers: Vec<DockerContainer>,
    pub message: String,
    pub audit_trail: Vec<AuditEntry>,
}

// ============================================================
// R026: K8s 运维数据模型
// ============================================================

/// K8s 操作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum K8sAction {
    /// 集群健康检查
    ClusterHealth,
    /// 节点状态
    NodeStatus,
    /// Pod 状态
    PodStatus { namespace: Option<String> },
    /// 失败Pod排查
    TroubleshootFailedPods { namespace: Option<String> },
    /// 资源使用
    ResourceUsage { namespace: Option<String> },
    /// 事件查看
    Events {
        namespace: Option<String>,
        limit: u32,
    },
    /// 描述资源
    Describe {
        resource_type: String,
        name: String,
        namespace: Option<String>,
    },
    /// 删除资源
    Delete {
        resource_type: String,
        name: String,
        namespace: Option<String>,
        force: bool,
    },
    /// 自定义 kubectl
    Kubectl { args: Vec<String> },
}

/// K8s 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sNode {
    pub name: String,
    pub status: String,
    pub roles: String,
    pub age: String,
    pub version: String,
    pub cpu: String,
    pub memory: String,
}

/// K8s Pod 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sPod {
    pub name: String,
    pub namespace: String,
    pub ready: String,
    pub status: String,
    pub restarts: u32,
    pub age: String,
    pub cpu: Option<String>,
    pub memory: Option<String>,
}

/// K8s 操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sOpsResult {
    pub context: String,
    pub action: K8sAction,
    pub status: TaskStatus,
    pub nodes: Vec<K8sNode>,
    pub pods: Vec<K8sPod>,
    pub output: String,
    pub recommendations: Vec<String>,
    pub audit_trail: Vec<AuditEntry>,
}

/// 备份操作类型 (R029)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupAction {
    /// 创建备份
    Create {
        sources: Vec<String>,
        destination: String,
        backup_type: BackupType,
        compression: CompressionType,
        encryption: bool,
        exclude_patterns: Vec<String>,
    },
    /// 恢复备份
    Restore {
        backup_id: String,
        restore_path: String,
        point_in_time: Option<DateTime<Utc>>,
    },
    /// 验证备份完整性
    Verify { backup_id: String },
    /// 列出备份
    List {
        source_filter: Option<String>,
        limit: Option<u32>,
    },
    /// 清理旧备份
    Prune {
        retention_days: u32,
        keep_daily: u32,
        keep_weekly: u32,
        keep_monthly: u32,
    },
    /// 恢复测试（自动恢复到临时目录并验证）
    RestoreTest { backup_id: String },
    /// 备份状态检查
    Status,
}

/// 备份类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupType {
    /// 全量备份
    Full,
    /// 增量备份（基于上次备份）
    Incremental,
    /// 差异备份（基于上次全量备份）
    Differential,
}

/// 压缩类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompressionType {
    None,
    Gzip,
    Zstd,
    Lz4,
}

/// 备份任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupJob {
    pub id: Uuid,
    pub name: String,
    pub sources: Vec<String>,
    pub destination: String,
    pub backup_type: BackupType,
    pub schedule: BackupSchedule,
    pub compression: CompressionType,
    pub encryption: bool,
    pub encryption_key_id: Option<String>,
    pub exclude_patterns: Vec<String>,
    pub retention: RetentionPolicy,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub status: BackupJobStatus,
    pub last_run: Option<DateTime<Utc>>,
    pub last_backup_id: Option<String>,
    pub total_backups: u32,
    pub total_size_bytes: u64,
}

/// 备份调度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupSchedule {
    /// 手动触发
    Manual,
    /// 每小时
    Hourly,
    /// 每天（指定时间）
    Daily { hour: u32, minute: u32 },
    /// 每周（指定天和时间）
    Weekly {
        day_of_week: u32,
        hour: u32,
        minute: u32,
    },
    /// 每月（指定日和时间）
    Monthly {
        day_of_month: u32,
        hour: u32,
        minute: u32,
    },
    /// Cron 表达式
    Cron { expression: String },
}

/// 保留策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub keep_daily: u32,
    pub keep_weekly: u32,
    pub keep_monthly: u32,
    pub keep_yearly: u32,
    pub max_total_size_gb: Option<f64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
            keep_yearly: 3,
            max_total_size_gb: None,
        }
    }
}

/// 备份作业状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupJobStatus {
    Active,
    Paused,
    Failed,
    Disabled,
}

/// 备份记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub backup_type: BackupType,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: BackupStatus,
    pub size_bytes: u64,
    pub file_count: u64,
    pub checksum: String,
    pub encryption: bool,
    pub compression: CompressionType,
    pub duration_secs: u64,
    pub source_hosts: Vec<String>,
    pub error_message: Option<String>,
    pub verification: Option<BackupVerification>,
}

/// 备份状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BackupStatus {
    InProgress,
    Completed,
    Failed,
    Verified,
    Corrupted,
}

/// 备份验证结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVerification {
    pub verified_at: DateTime<Utc>,
    pub checksum_match: bool,
    pub file_count_match: bool,
    pub restore_test_passed: bool,
    pub integrity_score: f64, // 0.0-1.0
    pub notes: String,
}

/// 备份统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupStatistics {
    pub total_jobs: u32,
    pub active_jobs: u32,
    pub total_backups: u32,
    pub total_size_bytes: u64,
    pub last_24h_backups: u32,
    pub last_24h_failures: u32,
    pub average_duration_secs: u64,
    pub storage_used_gb: f64,
    pub oldest_backup: Option<DateTime<Utc>>,
    pub newest_backup: Option<DateTime<Utc>>,
    pub verification_pass_rate: f64,
}

// === R031: 用户和权限管理 ===

/// 用户管理操作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserAction {
    /// 创建用户
    Create {
        username: String,
        uid: Option<u32>,
        shell: Option<String>,
        home_dir: Option<String>,
        groups: Vec<String>,
        ssh_key: Option<String>,
    },
    /// 删除用户
    Delete { username: String, remove_home: bool },
    /// 修改用户
    Modify {
        username: String,
        new_shell: Option<String>,
        new_home: Option<String>,
        add_groups: Vec<String>,
        remove_groups: Vec<String>,
        lock: Option<bool>,
    },
    /// 列出用户
    List { system_users: bool },
    /// 检查用户状态
    Check { username: String },
    /// 锁定用户
    Lock { username: String },
    /// 解锁用户
    Unlock { username: String },
    /// 创建用户组
    CreateGroup { groupname: String, gid: Option<u32> },
    /// 删除用户组
    DeleteGroup { groupname: String },
    /// 管理sudo权限
    SudoManage {
        username: String,
        rules: Vec<String>,
        remove: bool,
    },
    /// 检查文件权限
    CheckPermissions {
        path: String,
        expected_owner: String,
        expected_mode: String,
    },
    /// 修复文件权限
    FixPermissions {
        path: String,
        owner: String,
        mode: String,
        recursive: bool,
    },
}

/// 用户信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInfo {
    pub username: String,
    pub uid: u32,
    pub gid: u32,
    pub home_dir: String,
    pub shell: String,
    pub groups: Vec<String>,
    pub locked: bool,
    pub last_login: Option<String>,
    pub comment: String,
}

/// 用户组信息
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GroupInfo {
    pub name: String,
    pub gid: u32,
    pub members: Vec<String>,
}

/// 用户管理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserManageResult {
    pub action: String,
    pub host: String,
    pub status: TaskStatus,
    pub message: String,
    pub users: Vec<UserInfo>,
    pub groups: Vec<GroupInfo>,
    pub permission_checks: Vec<PermissionCheckResult>,
    pub commands_executed: Vec<String>,
    pub audit_id: String,
}

/// 权限检查结果
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionCheckResult {
    pub path: String,
    pub owner: String,
    pub group: String,
    pub mode: String,
    pub expected_owner: Option<String>,
    pub expected_mode: Option<String>,
    pub compliant: bool,
    pub issues: Vec<String>,
}

// === R030: 性能监控和优化 ===

/// 性能指标类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PerfMetricType {
    CpuUsage,
    MemoryUsage,
    DiskIoRead,
    DiskIoWrite,
    NetworkRx,
    NetworkTx,
    LoadAverage,
    SwapUsage,
    DiskUsage,
    ProcessCount,
    ContextSwitches,
    Interrupts,
}

/// 单个性能指标采样
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfSample {
    pub metric_type: PerfMetricType,
    pub value: f64,
    pub unit: String,
    pub timestamp: DateTime<Utc>,
    pub host: String,
}

/// 性能基线（正常行为范围）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfBaseline {
    pub metric_type: PerfMetricType,
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub p95: f64,
    pub p99: f64,
    pub sample_count: u32,
    pub established_at: DateTime<Utc>,
}

/// 性能异常
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnomalySeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfAnomaly {
    pub metric_type: PerfMetricType,
    pub value: f64,
    pub baseline_mean: f64,
    pub deviation_sigma: f64,
    pub severity: AnomalySeverity,
    pub message: String,
    pub detected_at: DateTime<Utc>,
}

/// 瓶颈类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BottleneckType {
    CpuBound,
    MemoryBound,
    DiskIoBound,
    NetworkBound,
    SwapThrashing,
    ProcessSaturation,
    NoBottleneck,
}

/// 瓶颈分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BottleneckAnalysis {
    pub primary: BottleneckType,
    pub secondary: Option<BottleneckType>,
    pub cpu_score: f64,
    pub memory_score: f64,
    pub disk_io_score: f64,
    pub network_score: f64,
    pub description: String,
}

/// 优化建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub category: BottleneckType,
    pub priority: u8,
    pub title: String,
    pub description: String,
    pub expected_improvement: String,
    pub command: Option<String>,
}

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfReport {
    pub host: String,
    pub collected_at: DateTime<Utc>,
    pub samples: Vec<PerfSample>,
    pub baselines: Vec<PerfBaseline>,
    pub anomalies: Vec<PerfAnomaly>,
    pub bottleneck: BottleneckAnalysis,
    pub recommendations: Vec<OptimizationRecommendation>,
    pub overall_score: f64,
}

/// 性能监控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfMonitorConfig {
    /// 采集间隔（秒）
    pub interval_secs: u64,
    /// 基线建立所需最少样本数
    pub min_baseline_samples: u32,
    /// 异常检测阈值（标准差倍数）
    pub anomaly_sigma_threshold: f64,
    /// 严重异常阈值
    pub critical_sigma_threshold: f64,
    /// 监控的指标类型
    pub metrics: Vec<PerfMetricType>,
}

impl Default for PerfMonitorConfig {
    fn default() -> Self {
        Self {
            interval_secs: 60,
            min_baseline_samples: 30,
            anomaly_sigma_threshold: 2.0,
            critical_sigma_threshold: 3.0,
            metrics: vec![
                PerfMetricType::CpuUsage,
                PerfMetricType::MemoryUsage,
                PerfMetricType::DiskIoRead,
                PerfMetricType::DiskIoWrite,
                PerfMetricType::NetworkRx,
                PerfMetricType::NetworkTx,
                PerfMetricType::LoadAverage,
                PerfMetricType::SwapUsage,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compliance_tool_variants() {
        let tools = [
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
        let priorities = [
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
        let statuses = [
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
        let severities = [
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
        let statuses = [
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

    #[test]
    fn test_task_type_variants() {
        let scan = TaskType::ComplianceScan {
            profile: "CIS".to_string(),
            hosts: vec!["h1".to_string()],
        };
        assert!(matches!(scan, TaskType::ComplianceScan { .. }));

        let patch = TaskType::PatchInstall {
            packages: vec!["vim".to_string()],
            hosts: vec!["h1".to_string()],
        };
        assert!(matches!(patch, TaskType::PatchInstall { .. }));

        let deploy = TaskType::ConfigDeploy {
            playbook: "site.yml".to_string(),
            hosts: vec!["h1".to_string()],
        };
        assert!(matches!(deploy, TaskType::ConfigDeploy { .. }));
    }

    #[test]
    fn test_compliance_report_creation() {
        let report = ComplianceReport {
            host: "server1".to_string(),
            scan_time: Utc::now(),
            profile: "CIS Level 1".to_string(),
            score: 85.5,
            passed: 100,
            failed: 15,
            not_applicable: 5,
            findings: vec![],
        };
        assert_eq!(report.score, 85.5);
        assert_eq!(report.passed + report.failed + report.not_applicable, 120);
    }

    #[test]
    fn test_compliance_finding_creation() {
        let finding = ComplianceFinding {
            rule_id: "CIS-1.1.1".to_string(),
            title: "Disable unused filesystem".to_string(),
            severity: Severity::High,
            status: FindingStatus::Fail,
            description: "Test".to_string(),
            remediation: Some("Fix it".to_string()),
        };
        assert_eq!(finding.severity, Severity::High);
        assert_eq!(finding.status, FindingStatus::Fail);
        assert!(finding.remediation.is_some());
    }

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            user: "admin".to_string(),
            action: "deploy".to_string(),
            target: "server1".to_string(),
            result: "success".to_string(),
            details: Some("deployed v2.0".to_string()),
            signature: None,
        };
        assert_eq!(entry.user, "admin");
        assert!(entry.details.is_some());
        assert!(entry.signature.is_none());
    }

    #[test]
    fn test_automation_result_creation() {
        let result = AutomationResult {
            task_id: Uuid::new_v4(),
            task_type: "ComplianceScan".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "All good".to_string(),
            audit_trail: vec![],
        };
        assert_eq!(result.status, TaskStatus::Success);
        assert!(result.completed_at.is_some());
    }

    #[test]
    fn test_automation_statistics_creation() {
        let stats = AutomationStatistics {
            total_tasks: 100,
            successful_tasks: 90,
            failed_tasks: 5,
            pending_tasks: 5,
            compliance_score: 95.0,
            audit_entries: 200,
            last_scan_time: Some(Utc::now()),
        };
        assert_eq!(stats.total_tasks, 100);
        assert_eq!(stats.compliance_score, 95.0);
    }

    #[test]
    fn test_serialization_roundtrip_task_status() {
        let statuses = vec![
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Success,
            TaskStatus::Failed,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_serialization_roundtrip_severity() {
        let severities = vec![
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ];
        for sev in severities {
            let json = serde_json::to_string(&sev).unwrap();
            let deserialized: Severity = serde_json::from_str(&json).unwrap();
            assert_eq!(sev, deserialized);
        }
    }

    #[test]
    fn test_task_type_security_update() {
        let task = TaskType::SecurityUpdate {
            hosts: vec!["h1".to_string(), "h2".to_string()],
        };
        assert!(matches!(task, TaskType::SecurityUpdate { .. }));
    }

    #[test]
    fn test_task_type_log_collection() {
        let task = TaskType::LogCollection {
            hosts: vec!["h1".to_string()],
            log_paths: vec!["/var/log/syslog".to_string()],
        };
        assert!(matches!(task, TaskType::LogCollection { .. }));
    }

    #[test]
    fn test_task_type_disk_cleanup() {
        let task = TaskType::DiskCleanup {
            hosts: vec!["h1".to_string()],
            targets: vec!["/tmp".to_string()],
        };
        assert!(matches!(task, TaskType::DiskCleanup { .. }));
    }

    #[test]
    fn test_task_type_service_restart() {
        let task = TaskType::ServiceRestart {
            service: "nginx".to_string(),
            hosts: vec!["h1".to_string()],
        };
        assert!(matches!(task, TaskType::ServiceRestart { .. }));
    }
}
