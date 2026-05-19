//! Linux 自动化 CLI 命令

use clap::Subcommand;
use serde::{Deserialize, Serialize};

/// Linux 自动化命令
#[derive(Clone, Subcommand)]
pub enum LinuxAction {
    /// 合规扫描
    Scan {
        /// 目标主机
        #[arg(short, long)]
        host: String,
        /// 合规配置文件
        #[arg(short, long, default_value = "cis")]
        profile: String,
    },
    /// 安装补丁
    Patch {
        /// 目标主机
        #[arg(short, long)]
        host: String,
        /// 包名列表
        #[arg(short, long, value_delimiter = ',')]
        packages: Vec<String>,
    },
    /// 部署配置
    Deploy {
        /// 目标主机
        #[arg(short, long)]
        host: String,
        /// Playbook 文件
        #[arg(short, long)]
        playbook: String,
    },
    /// 安全更新
    SecurityUpdate {
        /// 目标主机
        #[arg(short, long)]
        host: String,
    },
    /// 查看任务状态
    Status {
        /// 任务 ID
        #[arg(short, long)]
        task_id: String,
    },
    /// 查看任务历史
    History {
        /// 显示数量
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// 查看合规报告
    Report {
        /// 目标主机
        #[arg(short, long)]
        host: String,
        /// 输出格式
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// 查看审计日志
    Audit {
        /// 显示数量
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// 执行自定义命令
    Exec {
        /// 目标主机
        #[arg(short, long)]
        host: String,
        /// 要执行的命令
        #[arg(short, long)]
        command: String,
    },
    /// 查看统计信息
    Stats,
}

/// Linux 任务输出
#[derive(Debug, Serialize, Deserialize)]
pub struct LinuxTaskOutput {
    pub task_id: String,
    pub status: String,
    pub summary: String,
    pub host_results: Vec<HostResultOutput>,
}

/// 主机结果输出
#[derive(Debug, Serialize, Deserialize)]
pub struct HostResultOutput {
    pub host: String,
    pub status: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// 统计信息输出
#[derive(Debug, Serialize, Deserialize)]
pub struct LinuxStatsOutput {
    pub total_tasks: usize,
    pub successful_tasks: usize,
    pub failed_tasks: usize,
    pub pending_tasks: usize,
    pub compliance_score: f64,
    pub audit_entries: usize,
}
