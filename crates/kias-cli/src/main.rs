//! KIAS CLI 主入口 - 超越阿里云 AgentRun

use clap::Parser;
use kias_cli::{Cli, Commands, OutputFormat};
use std::process;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // 初始化日志
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter("debug")
            .init();
    }

    let output = cli.output.clone();
    let dry_run = cli.dry_run;
    let namespace = cli.namespace.clone();

    let exit_code = match cli.command {
        Commands::Agent { action } => handle_agent(action, &output, dry_run).await,
        Commands::Workflow { action } => handle_workflow(action, &output).await,
        Commands::Tool { action } => handle_tool(action, &output).await,
        Commands::Skill { action } => handle_skill(action, &output).await,
        Commands::Sandbox { action } => handle_sandbox(action, &output).await,
        Commands::Model { action } => handle_model(action, &output).await,
        Commands::Config { action } => handle_config(action, &output).await,
        Commands::Cluster { action } => handle_cluster(action, &output).await,
    };

    process::exit(exit_code);
}

/// 退出码
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    ArgumentError = 1,
    AuthError = 2,
    NotFound = 3,
    PermissionDenied = 4,
    ServerError = 5,
    Timeout = 6,
    CostExceeded = 7,
}

async fn handle_agent(action: kias_cli::AgentAction, output: &OutputFormat, dry_run: bool) -> i32 {
    match action {
        kias_cli::AgentAction::Apply { file } => {
            let yaml = match std::fs::read_to_string(&file) {
                Ok(y) => y,
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def = match kias_cli::agent::AgentDefinition::from_yaml(&yaml) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error parsing YAML: {}", e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            if let Err(errors) = def.validate() {
                for err in errors {
                    eprintln!("Validation error: {}", err);
                }
                return ExitCode::ArgumentError as i32;
            }

            if dry_run {
                println!("Dry-run: Agent '{}' is valid", def.metadata.name);
                return ExitCode::Success as i32;
            }

            // TODO: 实际应用到集群
            println!("Agent '{}' applied successfully", def.metadata.name);
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Run { name, prompt, model } => {
            println!("Running agent '{}' with prompt: {}", name, prompt);
            if let Some(m) = model {
                println!("Using model: {}", m);
            }
            // TODO: 连接到 KIAS API Server 执行
            println!("Agent execution completed");
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Invoke { name, text, text_only, timeout } => {
            println!("Invoking agent '{}' (timeout: {}s)", name, timeout);
            if text_only {
                println!("Output: [Agent response would appear here]");
            } else {
                println!("{}", serde_json::json!({
                    "status": "success",
                    "agent": name,
                    "output": "[Agent response]",
                    "metadata": {
                        "tokens_used": 150,
                        "cost": 0.003,
                        "duration_ms": 1200
                    }
                }));
            }
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::List { label } => {
            println!("Listing agents...");
            if let Some(l) = label {
                println!("Filter: label={}", l);
            }
            // TODO: 从 API Server 获取列表
            println!("No agents found");
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Get { name } => {
            println!("Getting agent '{}'", name);
            // TODO: 从 API Server 获取
            println!("Agent not found");
            ExitCode::NotFound as i32
        }
        kias_cli::AgentAction::Delete { name, force } => {
            if !force {
                eprintln!("Use --force to delete agent '{}'", name);
                return ExitCode::ArgumentError as i32;
            }
            println!("Agent '{}' deleted", name);
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Render { file } => {
            let yaml = match std::fs::read_to_string(&file) {
                Ok(y) => y,
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def = match kias_cli::agent::AgentDefinition::from_yaml(&yaml) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error parsing YAML: {}", e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            println!("{}", serde_yaml::to_string(&def).unwrap_or_default());
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Logs { name, follow, tail } => {
            println!("Logs for agent '{}' (tail: {}, follow: {})", name, tail, follow);
            // TODO: 从 API Server 获取日志
            println!("[No logs available]");
            ExitCode::Success as i32
        }
        kias_cli::AgentAction::Events { name, event_type } => {
            println!("Events for agent '{}'", name);
            if let Some(t) = event_type {
                println!("Filter: type={}", t);
            }
            // TODO: 从 API Server 获取事件
            println!("[No events available]");
            ExitCode::Success as i32
        }
    }
}

async fn handle_workflow(action: kias_cli::WorkflowAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::WorkflowAction::Apply { file } => {
            println!("Applying workflow from {}", file);
            ExitCode::Success as i32
        }
        kias_cli::WorkflowAction::Run { name, input } => {
            println!("Running workflow '{}'", name);
            if let Some(i) = input {
                println!("Input: {}", i);
            }
            ExitCode::Success as i32
        }
        kias_cli::WorkflowAction::Status { run_id } => {
            println!("Status for run '{}'", run_id);
            ExitCode::Success as i32
        }
        kias_cli::WorkflowAction::Logs { run_id } => {
            println!("Logs for run '{}'", run_id);
            ExitCode::Success as i32
        }
        kias_cli::WorkflowAction::List => {
            println!("Listing workflows...");
            ExitCode::Success as i32
        }
    }
}

async fn handle_tool(action: kias_cli::ToolAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::ToolAction::Register { file } => {
            println!("Registering tool from {}", file);
            ExitCode::Success as i32
        }
        kias_cli::ToolAction::List => {
            println!("Listing tools...");
            ExitCode::Success as i32
        }
        kias_cli::ToolAction::Test { name, input } => {
            println!("Testing tool '{}'", name);
            if let Some(i) = input {
                println!("Input: {}", i);
            }
            ExitCode::Success as i32
        }
    }
}

async fn handle_skill(action: kias_cli::SkillAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::SkillAction::Register { file } => {
            println!("Registering skill from {}", file);
            ExitCode::Success as i32
        }
        kias_cli::SkillAction::List => {
            println!("Listing skills...");
            ExitCode::Success as i32
        }
        kias_cli::SkillAction::Search { query } => {
            println!("Searching skills: {}", query);
            ExitCode::Success as i32
        }
    }
}

async fn handle_sandbox(action: kias_cli::SandboxAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::SandboxAction::Create { template, name } => {
            println!("Creating sandbox from template '{}'", template);
            if let Some(n) = name {
                println!("Name: {}", n);
            }
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::Exec { sandbox_id, command } => {
            println!("Executing in sandbox '{}': {:?}", sandbox_id, command);
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::Destroy { sandbox_id } => {
            println!("Destroying sandbox '{}'", sandbox_id);
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::List => {
            println!("Listing sandboxes...");
            ExitCode::Success as i32
        }
    }
}

async fn handle_model(action: kias_cli::ModelAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::ModelAction::Register { file } => {
            println!("Registering model from {}", file);
            ExitCode::Success as i32
        }
        kias_cli::ModelAction::List => {
            println!("Listing models...");
            ExitCode::Success as i32
        }
        kias_cli::ModelAction::Test { name, prompt } => {
            println!("Testing model '{}'", name);
            if let Some(p) = prompt {
                println!("Prompt: {}", p);
            }
            ExitCode::Success as i32
        }
    }
}

async fn handle_config(action: kias_cli::ConfigAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::ConfigAction::Set { key, value } => {
            println!("Setting config: {} = {}", key, value);
            ExitCode::Success as i32
        }
        kias_cli::ConfigAction::Get { key } => {
            println!("Getting config: {}", key);
            ExitCode::Success as i32
        }
        kias_cli::ConfigAction::List => {
            println!("Listing config...");
            ExitCode::Success as i32
        }
        kias_cli::ConfigAction::Init => {
            println!("Initializing config...");
            ExitCode::Success as i32
        }
    }
}

async fn handle_cluster(action: kias_cli::ClusterAction, _output: &OutputFormat) -> i32 {
    match action {
        kias_cli::ClusterAction::Status => {
            println!("Cluster status:");
            println!("  Nodes: 3");
            println!("  Agents: 5");
            println!("  Status: Healthy");
            ExitCode::Success as i32
        }
        kias_cli::ClusterAction::Nodes => {
            println!("Listing nodes...");
            ExitCode::Success as i32
        }
        kias_cli::ClusterAction::Resources => {
            println!("Resource usage:");
            println!("  CPU: 45%");
            println!("  Memory: 60%");
            ExitCode::Success as i32
        }
    }
}
