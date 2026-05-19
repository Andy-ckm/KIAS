//! KIAS CLI - Enterprise Agent Management Tool
//! 超越阿里云 AgentRun CLI

pub mod agent;
pub mod client;
pub mod config;
pub mod output;
pub mod process;
pub mod sandbox;
pub mod skill;
pub mod tool;
pub mod workflow;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kias",
    about = "AgentGuard - Enterprise Agent Management Platform CLI",
    version,
    long_about = "AgentGuard CLI 是企业级 Agent 管理工具，支持声明式 Agent 定义、运行、部署和管理。\n超越阿里云 AgentRun CLI，提供更好的企业特性。"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 输出格式
    #[arg(long, default_value = "json", global = true)]
    pub output: OutputFormat,

    /// Dry-run 模式
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// 命名空间
    #[arg(long, global = true)]
    pub namespace: Option<String>,

    /// 配置文件路径
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// KIAS API Server 地址
    #[arg(long, global = true, env = "AgentGuard_SERVER")]
    pub server: Option<String>,

    /// API Key
    #[arg(long, global = true, env = "AgentGuard_API_KEY")]
    pub api_key: Option<String>,

    /// 详细输出
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
    Yaml,
    Quiet,
}

#[derive(Clone, Subcommand)]
pub enum Commands {
    /// Agent 管理
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// 工作流管理
    Workflow {
        #[command(subcommand)]
        action: WorkflowAction,
    },
    /// 工具管理
    Tool {
        #[command(subcommand)]
        action: ToolAction,
    },
    /// 技能管理
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// 沙箱管理
    Sandbox {
        #[command(subcommand)]
        action: SandboxAction,
    },
    /// 模型管理
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// 集群管理
    Cluster {
        #[command(subcommand)]
        action: ClusterAction,
    },
    /// 服务管理
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
}

#[derive(Clone, Subcommand)]
pub enum AgentAction {
    /// 声明式应用 Agent 定义
    Apply {
        /// YAML 定义文件
        #[arg(short, long)]
        file: String,
    },
    /// 删除 Agent
    Delete {
        /// Agent 名称
        name: String,
        /// 强制删除
        #[arg(short, long)]
        force: bool,
    },
    /// 获取 Agent 信息
    Get {
        /// Agent 名称
        name: String,
    },
    /// 列出所有 Agent
    List {
        /// 标签过滤
        #[arg(short, long)]
        label: Option<String>,
    },
    /// 运行 Agent（交互式）
    Run {
        /// Agent 名称
        #[arg(short, long)]
        name: String,
        /// Prompt
        #[arg(short, long)]
        prompt: String,
        /// 模型覆盖
        #[arg(long)]
        model: Option<String>,
    },
    /// 非交互调用 Agent（CI 友好）
    Invoke {
        /// Agent 名称
        #[arg(short, long)]
        name: String,
        /// 输入文本
        #[arg(short, long)]
        text: String,
        /// 只输出文本
        #[arg(long)]
        text_only: bool,
        /// 超时时间（秒）
        #[arg(long, default_value = "300")]
        timeout: u64,
    },
    /// 渲染 Agent 定义（本地校验）
    Render {
        /// YAML 定义文件
        #[arg(short, long)]
        file: String,
    },
    /// 查看 Agent 日志
    Logs {
        /// Agent 名称
        name: String,
        /// 跟踪输出
        #[arg(short, long)]
        follow: bool,
        /// 最后 N 行
        #[arg(long, default_value = "100")]
        tail: usize,
    },
    /// 查看 Agent 事件
    Events {
        /// Agent 名称
        name: String,
        /// 事件类型过滤
        #[arg(long)]
        event_type: Option<String>,
    },
}

#[derive(Clone, Subcommand)]
pub enum WorkflowAction {
    /// 应用工作流定义
    Apply {
        #[arg(short, long)]
        file: String,
    },
    /// 运行工作流
    Run {
        name: String,
        /// 输入参数（JSON）
        #[arg(long)]
        input: Option<String>,
    },
    /// 查看工作流状态
    Status { run_id: String },
    /// 查看工作流日志
    Logs { run_id: String },
    /// 列出工作流
    List,
}

#[derive(Clone, Subcommand)]
pub enum ToolAction {
    /// 注册工具
    Register {
        #[arg(short, long)]
        file: String,
    },
    /// 列出工具
    List,
    /// 测试工具
    Test {
        name: String,
        #[arg(long)]
        input: Option<String>,
    },
}

#[derive(Clone, Subcommand)]
pub enum SkillAction {
    /// 注册技能
    Register {
        #[arg(short, long)]
        file: String,
    },
    /// 列出技能
    List,
    /// 搜索技能
    Search { query: String },
}

#[derive(Clone, Subcommand)]
pub enum SandboxAction {
    /// 创建沙箱
    Create {
        /// 模板名称
        #[arg(short, long)]
        template: String,
        /// 名称
        #[arg(long)]
        name: Option<String>,
    },
    /// 在沙箱中执行命令
    Exec {
        sandbox_id: String,
        /// 命令
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// 销毁沙箱
    Destroy { sandbox_id: String },
    /// 列出沙箱
    List,
}

#[derive(Clone, Subcommand)]
pub enum ModelAction {
    /// 注册模型服务
    Register {
        #[arg(short, long)]
        file: String,
    },
    /// 列出模型
    List,
    /// 测试模型
    Test {
        name: String,
        #[arg(long)]
        prompt: Option<String>,
    },
}

#[derive(Clone, Subcommand)]
pub enum ConfigAction {
    /// 设置配置
    Set { key: String, value: String },
    /// 获取配置
    Get { key: String },
    /// 列出配置
    List,
    /// 初始化配置
    Init,
}

#[derive(Clone, Subcommand)]
pub enum ClusterAction {
    /// 查看集群状态
    Status,
    /// 列出节点
    Nodes,
    /// 查看资源使用
    Resources,
}

#[derive(Clone, Subcommand)]
pub enum ServerAction {
    /// 启动服务
    Start {
        /// 配置文件
        #[arg(short, long)]
        config: Option<String>,
        /// 后台运行
        #[arg(short, long)]
        daemon: bool,
    },
    /// 停止服务
    Stop,
    /// 查看服务状态
    Status,
    /// 重启服务
    Restart,
}
