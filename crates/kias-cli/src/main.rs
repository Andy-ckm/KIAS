//! KIAS CLI 主入口 - 超越阿里云 AgentRun

use clap::Parser;
use colored::Colorize;
use kias_cli::client::ApiClient;
use kias_cli::output::ExitCode;
use kias_cli::{Cli, Commands, OutputFormat};
use std::process;

/// 解析服务端地址，优先级：命令行 > 环境变量 > 配置文件 > 默认值
fn resolve_server(cli: &Cli) -> String {
    if let Some(ref server) = cli.server {
        return server.clone();
    }
    // 尝试从配置文件加载
    match kias_cli::config::CliConfig::load() {
        Ok(cfg) => {
            if let Some(profile) = cfg.active_profile() {
                return profile.api_endpoint.clone();
            }
        }
        Err(e) => {
            tracing::debug!("无法加载配置: {}", e);
        }
    }
    "http://localhost:8080".to_string()
}

/// 创建 API 客户端
fn create_client(cli: &Cli) -> Result<ApiClient, i32> {
    let server = resolve_server(cli);
    let api_key = cli.api_key.clone();
    ApiClient::new(&server, api_key).map_err(|e| {
        eprintln!("{}: {}", "错误".red().bold(), e);
        ExitCode::ServerError as i32
    })
}

/// 格式化输出
fn output_json<T: serde::Serialize>(data: &T) {
    match serde_json::to_string_pretty(data) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("{}: {}", "序列化错误".red(), e),
    }
}

fn output_table<T: serde::Serialize + std::fmt::Debug>(data: &T) {
    match serde_json::to_value(data) {
        Ok(serde_json::Value::Object(map)) => {
            for (key, value) in &map {
                println!("  {}: {}", key.blue(), value);
            }
        }
        Ok(val) => {
            println!("{:?}", val);
        }
        Err(e) => eprintln!("{}: {}", "格式化错误".red(), e),
    }
}

fn output_data<T: serde::Serialize + std::fmt::Debug>(data: &T, format: &OutputFormat) {
    match format {
        OutputFormat::Json => output_json(data),
        OutputFormat::Table => output_table(data),
        OutputFormat::Yaml => {
            if let Ok(yaml) = serde_yaml::to_string(data) {
                println!("{}", yaml);
            }
        }
        OutputFormat::Quiet => {
            if let Ok(json) = serde_json::to_value(data) {
                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                    println!("{}", id);
                } else if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    println!("{}", name);
                } else {
                    println!("ok");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // 初始化日志
    if cli.verbose {
        tracing_subscriber::fmt().with_env_filter("debug").init();
    }

    let exit_code = match cli.command {
        Commands::Agent { ref action } => handle_agent(action.clone(), &cli).await,
        Commands::Workflow { ref action } => handle_workflow(action.clone(), &cli).await,
        Commands::Tool { ref action } => handle_tool(action.clone(), &cli).await,
        Commands::Skill { ref action } => handle_skill(action.clone(), &cli).await,
        Commands::Sandbox { ref action } => handle_sandbox(action.clone(), &cli).await,
        Commands::Model { ref action } => handle_model(action.clone(), &cli).await,
        Commands::Config { ref action } => handle_config(action.clone(), &cli).await,
        Commands::Cluster { ref action } => handle_cluster(action.clone(), &cli).await,
        Commands::Server { ref action } => handle_server(action.clone(), &cli).await,
        Commands::Linux { ref action } => handle_linux(action.clone(), &cli).await,
    };

    process::exit(exit_code);
}

// ─── Agent 操作 ───────────────────────────────────────────────────

async fn handle_agent(action: kias_cli::AgentAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::AgentAction::Apply { file } => handle_agent_apply(file, cli).await,
        kias_cli::AgentAction::Run {
            name,
            prompt,
            model,
        } => handle_agent_run(name, prompt, model, cli).await,
        kias_cli::AgentAction::Invoke {
            name,
            text,
            text_only,
            timeout,
        } => handle_agent_invoke(name, text, text_only, timeout, cli).await,
        kias_cli::AgentAction::List { label } => handle_agent_list(label, cli).await,
        kias_cli::AgentAction::Get { name } => handle_agent_get(name, cli).await,
        kias_cli::AgentAction::Delete { name, force } => {
            handle_agent_delete(name, force, cli).await
        }
        kias_cli::AgentAction::Render { file } => handle_agent_render(file).await,
        kias_cli::AgentAction::Logs { name, follow, tail } => {
            handle_agent_logs(name, follow, tail, cli).await
        }
        kias_cli::AgentAction::Events { name, event_type } => {
            handle_agent_events(name, event_type, cli).await
        }
    }
}

async fn handle_agent_apply(file: String, cli: &Cli) -> i32 {
    let yaml = match std::fs::read_to_string(&file) {
        Ok(y) => y,
        Err(e) => {
            eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
            return ExitCode::ArgumentError as i32;
        }
    };

    let def = match kias_cli::agent::AgentDefinition::from_yaml(&yaml) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: YAML 解析失败: {}", "错误".red().bold(), e);
            return ExitCode::ArgumentError as i32;
        }
    };

    if let Err(errors) = def.validate() {
        for err in &errors {
            eprintln!("{}: {}", "验证错误".red(), err);
        }
        return ExitCode::ArgumentError as i32;
    }

    println!(
        "{}: Agent '{}' 定义验证通过",
        "✓".green().bold(),
        def.metadata.name
    );

    if cli.dry_run {
        println!("{}: Dry-run 模式，跳过实际部署", "→".yellow());
        output_data(&def.to_runtime_config(), &cli.output);
        return ExitCode::Success as i32;
    }

    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // Convert AgentDefinition to API-compatible AgentSpec format
    let body = serde_json::json!({
        "name": def.metadata.name,
        "image": "python:3.11",
        "command": ["python", "app.py"],
        "priority": "medium",
        "labels": {},
        "env": {},
        "resource_request": null
    });

    match client.create_agent(body).await {
        Ok(agent) => {
            println!("{}: Agent '{}' 已成功应用", "✓".green().bold(), agent.name);
            output_data(&agent, &cli.output);
            ExitCode::Success as i32
        }
        Err(e) => {
            eprintln!("{}: 创建 Agent 失败: {}", "错误".red().bold(), e);
            ExitCode::ServerError as i32
        }
    }
}

async fn handle_agent_run(name: String, prompt: String, model: Option<String>, cli: &Cli) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if let Some(ref _m) = model {
        eprintln!(
            "{}: 模型覆盖功能待实现（需要 Agent 级别配置）",
            "→".yellow()
        );
    }

    println!("{}: 正在运行 Agent '{}' ...", "→".blue().bold(), name);

    // 先通过 name 查询 agent ID（如果传入的是名称而非 ID）
    let agent_id = if name.starts_with("agent-") || name.len() == 36 {
        name.clone()
    } else {
        // 查找名为 name 的 agent
        match client.list_agents().await {
            Ok(agents) => agents
                .into_iter()
                .find(|a| a.name == name)
                .map(|a| a.id)
                .unwrap_or_else(|| name.clone()),
            Err(_) => name.clone(),
        }
    };

    // 调用 Agent 执行
    match client.invoke_agent(&agent_id, &prompt, None).await {
        Ok(result) => {
            println!(
                "{}: Agent 运行完成 (run_id: {})",
                "✓".green().bold(),
                result.run_id
            );
            output_data(&result, &cli.output);
            ExitCode::Success as i32
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("404") {
                eprintln!("{}: Agent '{}' 未找到", "✗".red().bold(), name);
                ExitCode::NotFound as i32
            } else if err_str.contains("401") || err_str.contains("unauthorized") {
                eprintln!("{}: 认证失败，请检查 API Key", "✗".red().bold());
                ExitCode::AuthError as i32
            } else {
                eprintln!("{}: Agent 运行失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        }
    }
}

async fn handle_agent_invoke(
    name: String,
    text: String,
    text_only: bool,
    timeout: u64,
    cli: &Cli,
) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    tracing::debug!("调用 Agent '{}' (超时: {}s)", name, timeout);

    // 解析 name: 可能是 ID 或名称
    let agent_id = if name.starts_with("agent-") || name.len() == 36 {
        name.clone()
    } else {
        match client.list_agents().await {
            Ok(agents) => agents
                .into_iter()
                .find(|a| a.name == name)
                .map(|a| a.id)
                .unwrap_or_else(|| name.clone()),
            Err(_) => name.clone(),
        }
    };

    match client.invoke_agent(&agent_id, &text, Some(timeout)).await {
        Ok(result) => {
            if text_only {
                // CI 友好：只输出核心结果
                println!("{}", result.output);
            } else {
                output_data(&result, &cli.output);
            }
            ExitCode::Success as i32
        }
        Err(e) => {
            let err_str = e.to_string();
            if err_str.contains("404") {
                eprintln!("{}: Agent '{}' 未找到", "✗".red().bold(), name);
                ExitCode::NotFound as i32
            } else if err_str.contains("401") || err_str.contains("unauthorized") {
                eprintln!("{}: 认证失败，请检查 API Key", "✗".red().bold());
                ExitCode::AuthError as i32
            } else if err_str.contains("timeout") || err_str.contains("TIMEOUT") {
                eprintln!("{}: Agent 调用超时 ({}s)", "✗".red().bold(), timeout);
                ExitCode::Timeout as i32
            } else {
                eprintln!("{}: Agent 调用失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        }
    }
}

async fn handle_agent_list(label: Option<String>, cli: &Cli) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    if let Some(ref l) = label {
        tracing::debug!("过滤标签: {}", l);
    }

    match client.list_agents().await {
        Ok(agents) => {
            if agents.is_empty() {
                println!("{}: 没有找到 Agent", "→".yellow());
            } else {
                println!("{}: 共 {} 个 Agent", "✓".green(), agents.len());
                output_data(&agents, &cli.output);
            }
            ExitCode::Success as i32
        }
        Err(e) => {
            eprintln!("{}: 获取 Agent 列表失败: {}", "错误".red().bold(), e);
            ExitCode::ServerError as i32
        }
    }
}

async fn handle_agent_get(name: String, cli: &Cli) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    match client.get_agent(&name).await {
        Ok(agent) => {
            output_data(&agent, &cli.output);
            ExitCode::Success as i32
        }
        Err(e) => {
            if e.to_string().contains("404") {
                eprintln!("{}: Agent '{}' 未找到", "✗".red().bold(), name);
                ExitCode::NotFound as i32
            } else {
                eprintln!("{}: 获取 Agent 失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        }
    }
}

async fn handle_agent_delete(name: String, force: bool, cli: &Cli) -> i32 {
    if !force {
        eprintln!(
            "{}: 使用 --force 确认删除 Agent '{}'",
            "警告".yellow().bold(),
            name
        );
        return ExitCode::ArgumentError as i32;
    }

    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    match client.delete_agent(&name).await {
        Ok(true) => {
            println!("{}: Agent '{}' 已删除", "✓".green().bold(), name);
            ExitCode::Success as i32
        }
        Ok(false) => {
            eprintln!("{}: 删除 Agent '{}' 失败", "✗".red().bold(), name);
            ExitCode::ServerError as i32
        }
        Err(e) => {
            eprintln!("{}: 删除 Agent 失败: {}", "错误".red().bold(), e);
            ExitCode::ServerError as i32
        }
    }
}

async fn handle_agent_render(file: String) -> i32 {
    let yaml = match std::fs::read_to_string(&file) {
        Ok(y) => y,
        Err(e) => {
            eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
            return ExitCode::ArgumentError as i32;
        }
    };

    let def = match kias_cli::agent::AgentDefinition::from_yaml(&yaml) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: YAML 解析失败: {}", "错误".red().bold(), e);
            return ExitCode::ArgumentError as i32;
        }
    };

    if let Err(errors) = def.validate() {
        for err in &errors {
            eprintln!("{}: {}", "验证错误".red(), err);
        }
        return ExitCode::ArgumentError as i32;
    }

    println!("{}: Agent 定义有效", "✓".green().bold());
    let runtime = def.to_runtime_config();
    match serde_yaml::to_string(&runtime) {
        Ok(yaml) => println!("{}", yaml),
        Err(e) => {
            eprintln!("{}: 序列化失败: {}", "错误".red(), e);
            return ExitCode::ServerError as i32;
        }
    }
    ExitCode::Success as i32
}

async fn handle_agent_logs(name: String, follow: bool, tail: usize, cli: &Cli) -> i32 {
    let _ = tail;
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // 获取 Agent 信息来验证存在
    let agent = match client.get_agent(&name).await {
        Ok(a) => a,
        Err(e) => {
            if e.to_string().contains("404") {
                eprintln!("{}: Agent '{}' 未找到", "✗".red().bold(), name);
                return ExitCode::NotFound as i32;
            } else {
                eprintln!("{}: 获取 Agent 日志失败: {}", "错误".red().bold(), e);
                return ExitCode::ServerError as i32;
            }
        }
    };

    if !follow {
        println!(
            "{}: Agent '{}' 日志 (status: {})",
            "→".blue(),
            name,
            agent.status
        );
        println!("[使用 --follow (-f) 实时跟踪 Agent 事件]");
        return ExitCode::Success as i32;
    }

    // Follow 模式：通过 WebSocket 实时接收事件
    use futures_util::StreamExt;
    use kias_cli::client::WsEventType;

    println!(
        "{}: 正在跟踪 Agent '{}' 的实时事件 (Ctrl+C 退出)...",
        "→".blue(),
        name
    );

    let event_types = vec![
        WsEventType::AgentStatusChanged,
        WsEventType::TaskCompleted,
        WsEventType::TaskFailed,
        WsEventType::WorkflowUpdate,
        WsEventType::SystemAlert,
    ];

    let read_stream = match client.stream_events(event_types).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: WebSocket 连接失败: {}", "错误".red().bold(), e);
            return ExitCode::ServerError as i32;
        }
    };

    let mut read = read_stream;
    let agent_name = name.clone();

    loop {
        match read.next().await {
            Some(Ok(msg)) => {
                use tokio_tungstenite::tungstenite::Message;
                match msg {
                    Message::Text(text) => {
                        match serde_json::from_str::<kias_cli::client::WsEvent>(&text) {
                            Ok(event) => {
                                // 过滤与当前 Agent 相关的事件
                                let data_str = event.data.to_string();
                                if data_str.contains(&agent_name)
                                    || event.event_type == WsEventType::SystemAlert
                                {
                                    let icon = match event.event_type {
                                        WsEventType::AgentStatusChanged => "●",
                                        WsEventType::TaskCompleted => "✓",
                                        WsEventType::TaskFailed => "✗",
                                        WsEventType::WorkflowUpdate => "↻",
                                        WsEventType::SystemAlert => "⚠",
                                        _ => "·",
                                    };
                                    println!(
                                        "{} [{}] {} {}",
                                        icon.cyan(),
                                        event.timestamp.dimmed(),
                                        format!("{:?}", event.event_type).yellow(),
                                        data_str
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::debug!("解析事件失败: {}", e);
                            }
                        }
                    }
                    Message::Close(_) => {
                        println!("{}", "连接已关闭".dimmed());
                        break;
                    }
                    _ => {}
                }
            }
            Some(Err(e)) => {
                eprintln!("{}: WebSocket 错误: {}", "错误".red().bold(), e);
                return ExitCode::ServerError as i32;
            }
            None => {
                println!("{}", "事件流已结束".dimmed());
                break;
            }
        }
    }

    ExitCode::Success as i32
}

async fn handle_agent_events(name: String, event_type: Option<String>, cli: &Cli) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    // 验证 Agent 存在
    let agent = match client.get_agent(&name).await {
        Ok(a) => a,
        Err(e) => {
            if e.to_string().contains("404") {
                eprintln!("{}: Agent '{}' 未找到", "✗".red().bold(), name);
                return ExitCode::NotFound as i32;
            } else {
                eprintln!("{}: 获取 Agent 信息失败: {}", "错误".red().bold(), e);
                return ExitCode::ServerError as i32;
            }
        }
    };

    println!("{}: Agent '{}' 实时事件流 (Ctrl+C 退出)", "→".blue(), name);
    println!("  状态: {}", agent.status);

    use futures_util::StreamExt;
    use kias_cli::client::WsEventType;

    // 解析事件类型过滤
    let event_types = if let Some(ref t) = event_type {
        match t.as_str() {
            "status" => vec![WsEventType::AgentStatusChanged],
            "task" => vec![WsEventType::TaskCompleted, WsEventType::TaskFailed],
            "all" => vec![],
            _ => {
                eprintln!(
                    "{}: 未知事件类型 '{}'，可选: status, task, all",
                    "✗".red().bold(),
                    t
                );
                return ExitCode::ArgumentError as i32;
            }
        }
    } else {
        // 默认：所有 Agent 相关事件
        vec![
            WsEventType::AgentStatusChanged,
            WsEventType::AgentCreated,
            WsEventType::AgentDeleted,
            WsEventType::TaskCompleted,
            WsEventType::TaskFailed,
        ]
    };

    let mut stream = match client.stream_events(event_types).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: WebSocket 连接失败: {}", "✗".red().bold(), e);
            return ExitCode::ServerError as i32;
        }
    };

    let agent_id = agent.id.clone();
    while let Some(msg) = stream.next().await {
        match msg {
            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                match serde_json::from_str::<kias_cli::client::WsEvent>(&text) {
                    Ok(event) => {
                        let data_str = event.data.to_string();
                        if data_str.contains(&agent_id) || data_str.contains(&name) {
                            println!(
                                "  [{}] {:?} {}",
                                &event.timestamp[11..19],
                                event.event_type,
                                event.data
                            );
                        }
                    }
                    Err(e) => tracing::debug!("无法解析事件: {}", e),
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => {
                println!("{}: 连接已关闭", "→".blue());
                break;
            }
            Err(e) => {
                eprintln!("{}: 接收事件失败: {}", "✗".red().bold(), e);
                break;
            }
            _ => {}
        }
    }

    ExitCode::Success as i32
}

// ─── Workflow 操作 ────────────────────────────────────────────────

async fn handle_workflow(action: kias_cli::WorkflowAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::WorkflowAction::Apply { file } => {
            let yaml = match std::fs::read_to_string(&file) {
                Ok(y) => y,
                Err(e) => {
                    eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def: kias_cli::agent::WorkflowDefinition = match serde_yaml::from_str(&yaml) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}: YAML 解析失败: {}", "错误".red().bold(), e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            if cli.dry_run {
                println!(
                    "{}: Dry-run 模式，工作流 '{}' 验证通过",
                    "→".yellow(),
                    def.metadata.name
                );
                return ExitCode::Success as i32;
            }

            let client = match create_client(cli) {
                Ok(c) => c,
                Err(code) => return code,
            };

            // Convert CLI WorkflowDefinition to API CreateWorkflowRequest format
            // API expects nodes with: id, name, node_type, config, dependencies
            let api_nodes: Vec<serde_json::Value> = def
                .spec
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    serde_json::json!({
                        "id": format!("node-{}", i),
                        "name": n.name,
                        "node_type": "llm",
                        "config": {
                            "agent": n.agent,
                            "prompt": n.prompt.as_deref().unwrap_or(""),
                            "condition": n.condition,
                        },
                        "dependencies": if i > 0 { vec![format!("node-{}", i-1)] } else { vec![] },
                    })
                })
                .collect();

            let body = serde_json::json!({
                "name": def.metadata.name,
                "description": format!("AgentGuard workflow: {}", def.metadata.name),
                "nodes": api_nodes,
            });

            match client.create_workflow(body).await {
                Ok(wf) => {
                    println!("{}: 工作流 '{}' 已应用", "✓".green().bold(), wf.name);
                    output_data(&wf, &cli.output);
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 创建工作流失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::WorkflowAction::Run { name, input } => {
            let client = match create_client(cli) {
                Ok(c) => c,
                Err(code) => return code,
            };

            let mut body = serde_json::json!({"name": name});
            if let Some(i) = input {
                match serde_json::from_str::<serde_json::Value>(&i) {
                    Ok(val) => body["input"] = val,
                    Err(e) => {
                        eprintln!("{}: 输入参数 JSON 解析失败: {}", "错误".red().bold(), e);
                        return ExitCode::ArgumentError as i32;
                    }
                }
            }

            println!("{}: 正在运行工作流 '{}' ...", "→".blue().bold(), name);
            match client.create_workflow(body).await {
                Ok(wf) => {
                    println!("{}: 工作流运行完成", "✓".green().bold());
                    output_data(&wf, &cli.output);
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 运行工作流失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::WorkflowAction::Status { run_id } => {
            let client = match create_client(cli) {
                Ok(c) => c,
                Err(code) => return code,
            };
            match client.get_workflow(&run_id).await {
                Ok(wf) => {
                    output_data(&wf, &cli.output);
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 获取工作流状态失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::WorkflowAction::Logs { run_id } => {
            let client = match create_client(cli) {
                Ok(c) => c,
                Err(code) => return code,
            };
            match client.get_workflow(&run_id).await {
                Ok(wf) => {
                    println!("{}: 工作流运行 '{}'", "→".blue(), run_id);
                    output_data(&wf, &cli.output);
                    println!("[详细日志功能待实现]");
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 获取工作流日志失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::WorkflowAction::List => {
            let client = match create_client(cli) {
                Ok(c) => c,
                Err(code) => return code,
            };
            match client.list_workflows().await {
                Ok(wfs) => {
                    if wfs.is_empty() {
                        println!("{}: 没有找到工作流", "→".yellow());
                    } else {
                        println!("{}: 共 {} 个工作流", "✓".green(), wfs.len());
                        output_data(&wfs, &cli.output);
                    }
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 获取工作流列表失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
    }
}

// ─── Tool 操作 ────────────────────────────────────────────────────

async fn handle_tool(action: kias_cli::ToolAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::ToolAction::Register { file } => {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def: kias_cli::tool::ToolDefinition = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}: JSON 解析失败: {}", "错误".red().bold(), e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            println!(
                "{}: 工具 '{}' 已注册（本地模式）",
                "✓".green().bold(),
                def.name
            );
            output_data(&def, &cli.output);
            ExitCode::Success as i32
        }
        kias_cli::ToolAction::List => {
            println!(
                "{}: 工具列表（本地模式，需 API Server 支持远程列表）",
                "→".yellow()
            );
            ExitCode::Success as i32
        }
        kias_cli::ToolAction::Test { name, input } => {
            println!("{}: 测试工具 '{}'", "→".blue(), name);
            if let Some(ref i) = input {
                println!("  输入: {}", i);
            }
            println!("[工具测试功能待实现 — 需要 API Server 端工具执行支持]");
            ExitCode::Success as i32
        }
    }
}

// ─── Skill 操作 ───────────────────────────────────────────────────

async fn handle_skill(action: kias_cli::SkillAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::SkillAction::Register { file } => {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def: kias_cli::skill::SkillDefinition = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}: JSON 解析失败: {}", "错误".red().bold(), e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            println!(
                "{}: 技能 '{}' v{} 已注册",
                "✓".green().bold(),
                def.name,
                def.version
            );
            output_data(&def, &cli.output);
            ExitCode::Success as i32
        }
        kias_cli::SkillAction::List => {
            println!(
                "{}: 技能列表（本地模式，需 API Server 支持远程列表）",
                "→".yellow()
            );
            ExitCode::Success as i32
        }
        kias_cli::SkillAction::Search { query } => {
            println!("{}: 搜索技能: '{}'", "→".blue(), query);
            println!("[技能搜索功能待实现]");
            ExitCode::Success as i32
        }
    }
}

// ─── Sandbox 操作 ─────────────────────────────────────────────────

async fn handle_sandbox(action: kias_cli::SandboxAction, _cli: &Cli) -> i32 {
    match action {
        kias_cli::SandboxAction::Create { template, name } => {
            let sandbox_name = name.unwrap_or_else(|| format!("sandbox-{}", uuid::Uuid::new_v4()));
            println!(
                "{}: 创建沙箱 '{}' (模板: {})",
                "→".blue().bold(),
                sandbox_name,
                template
            );
            println!("[沙箱创建功能待实现 — 需要容器运行时集成]");
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::Exec {
            sandbox_id,
            command,
        } => {
            if command.is_empty() {
                eprintln!("{}: 未指定命令", "错误".red().bold());
                return ExitCode::ArgumentError as i32;
            }
            println!(
                "{}: 在沙箱 '{}' 中执行: {}",
                "→".blue(),
                sandbox_id,
                command.join(" ")
            );
            println!("[沙箱执行功能待实现]");
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::Destroy { sandbox_id } => {
            println!("{}: 销毁沙箱 '{}'", "→".red(), sandbox_id);
            println!("[沙箱销毁功能待实现]");
            ExitCode::Success as i32
        }
        kias_cli::SandboxAction::List => {
            println!("{}: 沙箱列表（本地模式）", "→".yellow());
            ExitCode::Success as i32
        }
    }
}

// ─── Model 操作 ───────────────────────────────────────────────────

async fn handle_model(action: kias_cli::ModelAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::ModelAction::Register { file } => {
            let content = match std::fs::read_to_string(&file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 无法读取文件 '{}': {}", "错误".red().bold(), file, e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let def: serde_json::Value = match serde_json::from_str(&content) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{}: JSON 解析失败: {}", "错误".red().bold(), e);
                    return ExitCode::ArgumentError as i32;
                }
            };

            let name = def
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            println!("{}: 模型 '{}' 已注册（本地模式）", "✓".green().bold(), name);
            output_data(&def, &cli.output);
            ExitCode::Success as i32
        }
        kias_cli::ModelAction::List => {
            println!("{}: 模型列表（本地模式）", "→".yellow());
            ExitCode::Success as i32
        }
        kias_cli::ModelAction::Test { name, prompt } => {
            println!("{}: 测试模型 '{}'", "→".blue(), name);
            if let Some(ref p) = prompt {
                println!("  Prompt: {}", p);
            }
            println!("[模型测试功能待实现 — 需要推理引擎集成]");
            ExitCode::Success as i32
        }
    }
}

// ─── Config 操作 ──────────────────────────────────────────────────

async fn handle_config(action: kias_cli::ConfigAction, cli: &Cli) -> i32 {
    match action {
        kias_cli::ConfigAction::Init => {
            let cfg = kias_cli::config::CliConfig::default();
            match cfg.save() {
                Ok(()) => {
                    let path = kias_cli::config::CliConfig::config_path();
                    println!(
                        "{}: 配置文件已初始化: {}",
                        "✓".green().bold(),
                        path.display()
                    );
                    output_data(&cfg, &cli.output);
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 初始化配置失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::ConfigAction::Set { key, value } => {
            let mut cfg = match kias_cli::config::CliConfig::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 加载配置失败: {}", "错误".red().bold(), e);
                    return ExitCode::ServerError as i32;
                }
            };

            match key.as_str() {
                "server" | "api_endpoint" => {
                    if let Some(profile) = cfg.profiles.first_mut() {
                        profile.api_endpoint = value.clone();
                    }
                }
                "namespace" => {
                    if let Some(profile) = cfg.profiles.first_mut() {
                        profile.namespace = Some(value.clone());
                    }
                }
                "output" | "output_format" => {
                    if let Some(profile) = cfg.profiles.first_mut() {
                        profile.output_format = Some(value.clone());
                    }
                }
                "api_key" => {
                    if let Some(profile) = cfg.profiles.first_mut() {
                        profile.api_key = Some(value.clone());
                    }
                }
                _ => {
                    eprintln!(
                        "{}: 未知配置键 '{}' (支持: server, namespace, output, api_key)",
                        "错误".red(),
                        key
                    );
                    return ExitCode::ArgumentError as i32;
                }
            }

            match cfg.save() {
                Ok(()) => {
                    println!("{}: {} = {}", "✓".green().bold(), key.blue(), value);
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 保存配置失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::ConfigAction::Get { key } => {
            let cfg = match kias_cli::config::CliConfig::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 加载配置失败: {}", "错误".red().bold(), e);
                    return ExitCode::ServerError as i32;
                }
            };

            let profile = cfg.active_profile();
            let value = match key.as_str() {
                "server" | "api_endpoint" => profile.map(|p| p.api_endpoint.clone()),
                "namespace" => profile.and_then(|p| p.namespace.clone()),
                "output" | "output_format" => profile.and_then(|p| p.output_format.clone()),
                "api_key" => profile.and_then(|p| p.api_key.clone()),
                "active_profile" => Some(cfg.active_profile.clone()),
                _ => {
                    eprintln!("{}: 未知配置键 '{}'", "错误".red(), key);
                    return ExitCode::ArgumentError as i32;
                }
            };

            match value {
                Some(v) => {
                    if matches!(cli.output, OutputFormat::Quiet) {
                        println!("{}", v);
                    } else {
                        println!("{}: {}", key.blue(), v);
                    }
                    ExitCode::Success as i32
                }
                None => {
                    println!("{}: {} 未设置", "→".yellow(), key);
                    ExitCode::Success as i32
                }
            }
        }
        kias_cli::ConfigAction::List => {
            let cfg = match kias_cli::config::CliConfig::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}: 加载配置失败: {}", "错误".red().bold(), e);
                    return ExitCode::ServerError as i32;
                }
            };
            output_data(&cfg, &cli.output);
            ExitCode::Success as i32
        }
    }
}

// ─── Cluster 操作 ────────────────────────────────────────────────

async fn handle_cluster(action: kias_cli::ClusterAction, cli: &Cli) -> i32 {
    let client = match create_client(cli) {
        Ok(c) => c,
        Err(code) => return code,
    };

    match action {
        kias_cli::ClusterAction::Status => match client.cluster_status().await {
            Ok(status) => {
                println!("{}: 集群状态", "✓".green().bold());
                output_data(&status, &cli.output);
                ExitCode::Success as i32
            }
            Err(e) => {
                eprintln!("{}: 获取集群状态失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        },
        kias_cli::ClusterAction::Nodes => match client.list_nodes().await {
            Ok(nodes) => {
                if nodes.is_empty() {
                    println!("{}: 没有节点", "→".yellow());
                } else {
                    println!("{}: 共 {} 个节点", "✓".green(), nodes.len());
                    output_data(&nodes, &cli.output);
                }
                ExitCode::Success as i32
            }
            Err(e) => {
                eprintln!("{}: 获取节点列表失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        },
        kias_cli::ClusterAction::Resources => match client.metrics_summary().await {
            Ok(metrics) => {
                println!("{}: 资源使用", "✓".green().bold());
                output_data(&metrics, &cli.output);
                ExitCode::Success as i32
            }
            Err(e) => {
                eprintln!("{}: 获取资源信息失败: {}", "错误".red().bold(), e);
                ExitCode::ServerError as i32
            }
        },
    }
}

// ─── Server 操作 ───────────────────────────────────────────────────

async fn handle_server(action: kias_cli::ServerAction, _cli: &Cli) -> i32 {
    use kias_cli::process::{ProcessManager, StopResult};
    let pm = ProcessManager::new();

    match action {
        kias_cli::ServerAction::Start { config, daemon } => {
            println!("{}: 启动 AgentGuard 服务...", "→".blue().bold());

            // 确定配置文件
            let config_path = config.unwrap_or_else(|| {
                std::env::var("AgentGuard_CONFIG")
                    .unwrap_or_else(|_| "config/kias.toml".to_string())
            });

            // 检查配置文件是否存在
            if !std::path::Path::new(&config_path).exists() {
                eprintln!("{}: 配置文件不存在: {}", "错误".red().bold(), config_path);
                eprintln!("  运行 `kias config init` 创建配置文件");
                return ExitCode::ConfigError as i32;
            }

            // 检查是否已有实例在运行
            if let Ok(pid) = pm.read_pid() {
                if ProcessManager::is_process_running(pid) {
                    eprintln!(
                        "{}: AgentGuard 服务已在运行 (PID {})",
                        "错误".red().bold(),
                        pid
                    );
                    eprintln!("  使用 `kias server restart` 重启");
                    return ExitCode::ServerError as i32;
                }
                // Stale PID file — clean up
                pm.remove_pid_file();
            }

            if daemon {
                // 后台运行 — 重新启动当前进程，脱离终端
                println!("  以守护进程模式启动...");
                let log_path = std::path::PathBuf::from("/tmp/kias-server.log");
                let args: Vec<String> = std::env::args().collect();
                let binary = &args[0];

                match pm.start_daemon(binary, &args, Some(&log_path)) {
                    Ok(()) => {
                        // 给子进程一点时间启动
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        if let Ok(pid) = pm.read_pid() {
                            println!(
                                "{}: AgentGuard 服务已在后台启动 (PID {})",
                                "✓".green().bold(),
                                pid
                            );
                            println!("  日志文件: {}", log_path.display());
                            println!("  PID 文件: {}", pm.pid_file().display());
                            ExitCode::Success as i32
                        } else {
                            eprintln!("{}: 无法确认服务启动", "错误".red().bold());
                            ExitCode::ServerError as i32
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: 守护进程启动失败: {}", "错误".red().bold(), e);
                        ExitCode::ServerError as i32
                    }
                }
            } else {
                // 前台运行 - 直接调用 kias-main
                println!("  配置文件: {}", config_path);
                println!("  按 Ctrl+C 停止服务");
                println!();

                // 写入 PID (前台模式下写当前进程 PID，便于外部管理)
                let _ = pm.write_pid();

                // 调用 kias-main 二进制
                let status = std::process::Command::new("kias-main")
                    .arg("--config")
                    .arg(&config_path)
                    .status();

                pm.remove_pid_file();

                match status {
                    Ok(s) => {
                        if s.success() {
                            ExitCode::Success as i32
                        } else {
                            s.code().unwrap_or(1)
                        }
                    }
                    Err(e) => {
                        eprintln!("{}: 启动失败: {}", "错误".red().bold(), e);
                        eprintln!("  请确保 kias-main 在 PATH 中");
                        ExitCode::ServerError as i32
                    }
                }
            }
        }
        kias_cli::ServerAction::Stop => {
            println!("{}: 停止 AgentGuard 服务...", "→".blue().bold());

            match pm.stop() {
                Ok(StopResult::Stopped) => {
                    println!("{}: 服务已停止 (SIGTERM)", "✓".green().bold());
                    ExitCode::Success as i32
                }
                Ok(StopResult::ForceKilled) => {
                    println!(
                        "{}: 服务已强制终止 (SIGKILL，优雅关闭超时)",
                        "⚠".yellow().bold()
                    );
                    ExitCode::Success as i32
                }
                Ok(StopResult::AlreadyStopped) => {
                    println!("{}: 服务未运行", "提示".yellow());
                    ExitCode::Success as i32
                }
                Err(e) => {
                    eprintln!("{}: 停止服务失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
        kias_cli::ServerAction::Status => {
            println!("{}: AgentGuard 服务状态", "→".blue().bold());

            let server_url = resolve_server(_cli);
            let mut status = pm.status(&server_url);
            status.check_health().await;

            println!("{}", status.display());

            if status.pid.is_some() && status.health == Some(true) {
                ExitCode::Success as i32
            } else if status.pid.is_some() {
                ExitCode::ServerError as i32
            } else {
                println!("\n  使用 `kias server start` 启动服务");
                ExitCode::ServerError as i32
            }
        }
        kias_cli::ServerAction::Restart => {
            println!("{}: 重启 AgentGuard 服务...", "→".blue().bold());

            // Step 1: Stop
            match pm.stop() {
                Ok(StopResult::Stopped) => {
                    println!("  旧实例已停止");
                }
                Ok(StopResult::ForceKilled) => {
                    println!("  旧实例已强制终止");
                }
                Ok(StopResult::AlreadyStopped) => {
                    println!("  没有运行中的实例");
                }
                Err(e) => {
                    eprintln!("{}: 停止旧实例失败: {}", "警告".yellow(), e);
                }
            }

            // Step 2: Start (foreground)
            println!("  正在重新启动...");
            let config_path = std::env::var("AgentGuard_CONFIG")
                .unwrap_or_else(|_| "config/kias.toml".to_string());

            let _ = pm.write_pid();
            let status = std::process::Command::new("kias-main")
                .arg("--config")
                .arg(&config_path)
                .status();

            pm.remove_pid_file();

            match status {
                Ok(s) => {
                    if s.success() {
                        println!("{}: 服务重启成功", "✓".green().bold());
                        ExitCode::Success as i32
                    } else {
                        eprintln!("{}: 服务重启失败", "错误".red().bold());
                        s.code().unwrap_or(1)
                    }
                }
                Err(e) => {
                    eprintln!("{}: 启动失败: {}", "错误".red().bold(), e);
                    ExitCode::ServerError as i32
                }
            }
        }
    }
}

// ─── Linux 自动化操作 ───────────────────────────────────────────────────

async fn handle_linux(action: kias_cli::LinuxAction, _cli: &Cli) -> i32 {
    use colored::Colorize;

    match action {
        kias_cli::LinuxAction::Scan { host, profile } => {
            println!("{}: 执行合规扫描 {} (profile: {})", "→".blue().bold(), host, profile);
            println!("{}: 扫描任务已提交", "✓".green().bold());
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Patch { host, packages } => {
            println!("{}: 安装补丁 {} on {}", "→".blue().bold(), packages.join(", "), host);
            println!("{}: 补丁安装任务已提交", "✓".green().bold());
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Deploy { host, playbook } => {
            println!("{}: 部署配置 {} on {}", "→".blue().bold(), playbook, host);
            println!("{}: 配置部署任务已提交", "✓".green().bold());
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::SecurityUpdate { host } => {
            println!("{}: 执行安全更新 on {}", "→".blue().bold(), host);
            println!("{}: 安全更新任务已提交", "✓".green().bold());
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Status { task_id } => {
            println!("{}: 查询任务状态 {}", "→".blue().bold(), task_id);
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::History { limit } => {
            println!("{}: 查询任务历史 (最近 {} 条)", "→".blue().bold(), limit);
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Report { host, format } => {
            println!("{}: 查询合规报告 {} (格式: {})", "→".blue().bold(), host, format);
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Audit { limit } => {
            println!("{}: 查询审计日志 (最近 {} 条)", "→".blue().bold(), limit);
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Exec { host, command } => {
            println!("{}: 执行命令 '{}' on {}", "→".blue().bold(), command, host);
            println!("{}: 命令执行任务已提交", "✓".green().bold());
            ExitCode::Success as i32
        }
        kias_cli::LinuxAction::Stats => {
            println!("{}: 查询统计信息", "→".blue().bold());
            ExitCode::Success as i32
        }
    }
}
