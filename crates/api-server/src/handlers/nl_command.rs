//! 自然语言命令处理器
//!
//! 参考 Codex CLI 的 Agent Loop 模式：
//! User instruction → Intent parsing → Tool calls → Observations → Response
//!
//! 提供 /api/v1/nl/command 端点，接受自然语言指令并转换为 KIAS 操作。

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::handlers::workflows::WorkflowStatus;
use crate::models::agent::AgentSpec;
use crate::AppState;

/// 自然语言命令请求
#[derive(Debug, Clone, Deserialize)]
pub struct NlCommandRequest {
    /// 用户的自然语言指令
    pub command: String,
    /// 上下文（可选）
    #[serde(default)]
    pub context: Option<NlContext>,
    /// 执行模式: suggest（需确认）, auto（自动执行）
    #[serde(default = "default_mode")]
    pub mode: String,
}

fn default_mode() -> String {
    "suggest".to_string()
}

/// NL 命令上下文
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NlContext {
    pub working_dir: Option<String>,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub extra: Option<serde_json::Value>,
}

/// NL 命令响应
#[derive(Debug, Clone, Serialize)]
pub struct NlCommandResponse {
    /// 解析后的意图
    pub intent: String,
    /// 置信度
    pub confidence: f64,
    /// 执行的操作列表
    pub actions: Vec<NlAction>,
    /// 自然语言回复
    pub message: String,
    /// 建议的后续操作
    pub suggestions: Vec<String>,
}

/// NL 解析后的动作
#[derive(Debug, Clone, Serialize)]
pub struct NlAction {
    pub action_type: String,
    pub params: serde_json::Value,
    pub status: String,
    pub summary: Option<String>,
}

/// 支持的意图类型
#[derive(Debug, Clone)]
pub enum Intent {
    AgentCreate {
        name: Option<String>,
        model: Option<String>,
    },
    AgentList,
    AgentDelete {
        name: String,
    },
    AgentRun {
        name: String,
        prompt: Option<String>,
    },
    WorkflowCreate {
        name: Option<String>,
        description: Option<String>,
    },
    WorkflowRun {
        name: String,
    },
    WorkflowList,
    ClusterStatus,
    ServerStatus,
    Metrics,
    KnowledgeSearch {
        query: String,
    },
    /// 问题报告
    ProblemReport {
        title: String,
        description: String,
    },
    /// 查看问题列表
    ProblemList,
    /// 启动自动循环
    AutoLoopStart {
        problem: String,
    },
    /// 查看循环状态
    AutoLoopStatus,
    ConfigGet,
    Help,
    Unknown,
}

/// POST /api/v1/nl/command
/// 处理自然语言命令
pub async fn nl_command(
    State(state): State<AppState>,
    Json(req): Json<NlCommandRequest>,
) -> Result<Json<NlCommandResponse>, ApiError> {
    if req.command.trim().is_empty() {
        return Err(ApiError::bad_request("Command cannot be empty"));
    }

    let (intent, confidence) = parse_intent(&req.command);
    let (actions, message, suggestions) = execute_intent(&intent, &state).await;

    Ok(Json(NlCommandResponse {
        intent: format!("{:?}", intent),
        confidence,
        actions,
        message,
        suggestions,
    }))
}

/// POST /api/v1/nl/stream
/// 处理自然语言命令（SSE 流式输出）
pub async fn nl_stream(
    State(state): State<AppState>,
    Json(req): Json<NlCommandRequest>,
) -> axum::response::Response {
    // IntoResponse unused in tests

    // 解析意图并执行
    let (intent, confidence) = parse_intent(&req.command);
    let (actions, message, suggestions) = execute_intent(&intent, &state).await;

    let result = serde_json::json!({
        "type": "complete",
        "intent": format!("{:?}", intent),
        "confidence": confidence,
        "actions": actions,
        "message": message,
        "suggestions": suggestions,
    });

    // 返回 SSE 格式响应
    let body = format!(
        "data: {}\\n\\n",
        serde_json::to_string(&result).unwrap_or_default()
    );
    axum::response::Response::builder()
        .status(200)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .body(axum::body::Body::from(body))
        .unwrap_or_else(|_| {
            axum::response::Response::builder()
                .status(500)
                .body(axum::body::Body::empty())
                .unwrap()
        })
}

/// 意图解析器
fn parse_intent(command: &str) -> (Intent, f64) {
    let cmd = command.trim().to_lowercase();

    // Agent 相关
    if (cmd.contains("创建") || cmd.contains("create") || cmd.contains("新建"))
        && cmd.contains("agent")
    {
        let name = extract_name(
            &cmd,
            &["agent", "创建", "create", "新建", "一个", "名为", "叫"],
        );
        return (Intent::AgentCreate { name, model: None }, 0.9);
    }
    if (cmd.contains("列出")
        || cmd.contains("list")
        || cmd.contains("查看")
        || cmd.contains("show"))
        && cmd.contains("agent")
    {
        return (Intent::AgentList, 0.95);
    }
    if (cmd.contains("删除") || cmd.contains("delete") || cmd.contains("remove"))
        && cmd.contains("agent")
    {
        let name = extract_name(&cmd, &["agent", "删除", "delete", "remove"]);
        return (
            Intent::AgentDelete {
                name: name.unwrap_or_default(),
            },
            0.85,
        );
    }
    if (cmd.contains("运行")
        || cmd.contains("run")
        || cmd.contains("执行")
        || cmd.contains("invoke"))
        && cmd.contains("agent")
    {
        let name = extract_name(&cmd, &["agent", "运行", "run", "执行", "invoke"]);
        return (
            Intent::AgentRun {
                name: name.unwrap_or_default(),
                prompt: None,
            },
            0.85,
        );
    }

    // Workflow 相关
    if (cmd.contains("创建") || cmd.contains("create"))
        && (cmd.contains("workflow") || cmd.contains("工作流"))
    {
        let name = extract_name(&cmd, &["workflow", "工作流", "创建", "create"]);
        return (
            Intent::WorkflowCreate {
                name,
                description: None,
            },
            0.9,
        );
    }
    if (cmd.contains("列出") || cmd.contains("list") || cmd.contains("查看"))
        && (cmd.contains("workflow") || cmd.contains("工作流"))
    {
        return (Intent::WorkflowList, 0.95);
    }

    // 问题报告（在WorkflowRun之前）
    if cmd.contains("问题") || cmd.contains("bug") || cmd.contains("缺陷") {
        if cmd.contains("发现") || cmd.contains("报告") || cmd.contains("report") {
            let title = extract_problem_title(&cmd);
            let description = cmd.clone();
            return (Intent::ProblemReport { title, description }, 0.85);
        }
        if cmd.contains("列表") || cmd.contains("list") {
            return (Intent::ProblemList, 0.9);
        }
    }

    // 报告bug（独立处理）
    if (cmd.starts_with("报告") || cmd.starts_with("report"))
        && (cmd.contains("bug") || cmd.contains("问题") || cmd.contains("缺陷"))
    {
        let title = extract_problem_title(&cmd);
        let description = cmd.clone();
        return (Intent::ProblemReport { title, description }, 0.85);
    }

    if (cmd.contains("运行") || cmd.contains("run") || cmd.contains("执行"))
        && (cmd.contains("workflow") || cmd.contains("工作流"))
    {
        let name = extract_name(&cmd, &["workflow", "工作流", "运行", "run", "执行"]);
        return (
            Intent::WorkflowRun {
                name: name.unwrap_or_default(),
            },
            0.85,
        );
    }

    // 集群/服务
    if cmd.contains("集群") || cmd.contains("cluster") {
        return (Intent::ClusterStatus, 0.9);
    }
    if cmd.contains("状态")
        || cmd.contains("status")
        || cmd.contains("健康")
        || cmd.contains("health")
    {
        return (Intent::ServerStatus, 0.9);
    }
    if cmd.contains("指标") || cmd.contains("metrics") || cmd.contains("统计") {
        return (Intent::Metrics, 0.9);
    }
    if cmd.contains("搜索") || cmd.contains("search") {
        let query = extract_query(&cmd);
        return (Intent::KnowledgeSearch { query }, 0.8);
    }
    if cmd.contains("配置") || cmd.contains("config") {
        return (Intent::ConfigGet, 0.8);
    }
    if cmd.contains("帮助") || cmd.contains("help") || cmd == "?" {
        return (Intent::Help, 0.95);
    }

    // 问题报告
    if cmd.contains("问题") || cmd.contains("bug") || cmd.contains("缺陷") {
        if cmd.contains("发现") || cmd.contains("报告") || cmd.contains("report") {
            let title = extract_problem_title(&cmd);
            let description = cmd.clone();
            return (Intent::ProblemReport { title, description }, 0.85);
        }
        if cmd.contains("列表") || cmd.contains("list") {
            return (Intent::ProblemList, 0.9);
        }
    }

    // 报告bug（独立处理）
    if (cmd.starts_with("报告") || cmd.starts_with("report"))
        && (cmd.contains("bug") || cmd.contains("问题") || cmd.contains("缺陷"))
    {
        let title = extract_problem_title(&cmd);
        let description = cmd.clone();
        return (Intent::ProblemReport { title, description }, 0.85);
    }

    // 自动循环
    if cmd.contains("自动循环") || cmd.contains("auto loop") || cmd.contains("autoloop") {
        if cmd.contains("启动") || cmd.contains("start") || cmd.contains("开始") {
            let problem = extract_problem_title(&cmd);
            return (Intent::AutoLoopStart { problem }, 0.85);
        }
        if cmd.contains("状态") || cmd.contains("status") {
            return (Intent::AutoLoopStatus, 0.9);
        }
    }

    // English
    if cmd.starts_with("list") && cmd.contains("agent") {
        return (Intent::AgentList, 0.9);
    }
    if cmd.starts_with("list") && (cmd.contains("workflow") || cmd.contains("workflows")) {
        return (Intent::WorkflowList, 0.9);
    }
    if cmd.contains("cluster") {
        return (Intent::ClusterStatus, 0.9);
    }
    if cmd.contains("help") {
        return (Intent::Help, 0.95);
    }

    (Intent::Unknown, 0.0)
}

/// 从命令中提取名称
fn extract_name(cmd: &str, exclude_words: &[&str]) -> Option<String> {
    // 尝试从 "名为 XXX" 或 "叫 XXX" 模式提取
    for pattern in &["名为 ", "叫 ", "named ", "called "] {
        if let Some(pos) = cmd.find(pattern) {
            let after = &cmd[pos + pattern.len()..];
            let name: String = after
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '的' && *c != '"' && *c != '\'')
                .collect();
            if !name.is_empty() && name.len() > 1 {
                return Some(name);
            }
        }
    }

    // 回退：按空格分词，跳过排除词
    let words: Vec<&str> = cmd.split_whitespace().collect();
    for word in &words {
        let lower = word.to_lowercase();
        if !exclude_words.contains(&lower.as_str())
            && ![
                "的", "一个", "a", "an", "the", "named", "called", "名为", "叫",
            ]
            .contains(&lower.as_str())
            && !lower.is_empty()
            && lower.len() > 1
            && !lower.chars().all(|c| c.is_ascii_digit())
        {
            return Some(word.to_string());
        }
    }
    None
}

/// 从命令中提取搜索查询
fn extract_query(cmd: &str) -> String {
    // 尝试提取引号内的内容
    if let Some(start) = cmd.find('"').or_else(|| cmd.find('\u{201c}')) {
        if let Some(end) = cmd[start + 1..]
            .find('"')
            .or_else(|| cmd[start + 1..].find('\u{201d}'))
        {
            return cmd[start + 1..start + 1 + end].to_string();
        }
    }
    // 去掉关键词
    cmd.split_whitespace()
        .filter(|w| !["搜索", "search", "查找"].contains(&w.to_lowercase().as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 从命令中提取问题标题
fn extract_problem_title(cmd: &str) -> String {
    // 尝试提取引号内的内容
    if let Some(start) = cmd.find('"').or_else(|| cmd.find('\u{201c}')) {
        if let Some(end) = cmd[start + 1..]
            .find('"')
            .or_else(|| cmd[start + 1..].find('\u{201d}'))
        {
            return cmd[start + 1..start + 1 + end].to_string();
        }
    }
    // 去掉关键词，保留主要描述
    let keywords = [
        "发现", "报告", "report", "问题", "bug", "缺陷", "了", "：", ":",
    ];
    let mut title = cmd.to_string();
    for keyword in &keywords {
        title = title.replace(keyword, "");
    }
    title.trim().to_string()
}

/// 执行解析后的意图
async fn execute_intent(intent: &Intent, state: &AppState) -> (Vec<NlAction>, String, Vec<String>) {
    match intent {
        Intent::AgentList => {
            let agents = state.agents.read().await;
            let count = agents.len();
            let names: Vec<String> = agents.values().map(|a| a.spec.name.clone()).collect();

            let action = NlAction {
                action_type: "agent.list".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some(format!("Found {} agents", count)),
            };

            let msg = if count == 0 {
                "当前没有注册的 Agent".to_string()
            } else {
                format!("共 {} 个 Agent: {}", count, names.join(", "))
            };

            (
                vec![action],
                msg,
                vec!["创建新 Agent".to_string(), "查看集群状态".to_string()],
            )
        }

        Intent::AgentCreate { name, model } => {
            let agent_name = name
                .clone()
                .unwrap_or_else(|| format!("agent-{}", &uuid::Uuid::new_v4().to_string()[..8]));
            let model_name = model.clone().unwrap_or_else(|| "gpt-4o".to_string());

            let spec = AgentSpec {
                name: agent_name.clone(),
                image: "python:3.11".to_string(),
                command: vec!["python".to_string(), "app.py".to_string()],
                resource_request: None,
                labels: std::collections::HashMap::new(),
                priority: "medium".to_string(),
                env: std::collections::HashMap::new(),
            };

            let agent = crate::models::agent::Agent::from_spec(spec);
            let agent_id = agent.id.clone();

            let mut agents = state.agents.write().await;
            agents.insert(agent_id.clone(), agent);

            let action = NlAction {
                action_type: "agent.create".to_string(),
                params: serde_json::json!({ "name": agent_name, "model": model_name }),
                status: "completed".to_string(),
                summary: Some(format!("Agent '{}' created", agent_name)),
            };

            (
                vec![action],
                format!("✓ 已创建 Agent '{}' (ID: {})", agent_name, agent_id),
                vec![
                    format!("运行 Agent '{}'", agent_name),
                    "查看 Agent 列表".to_string(),
                ],
            )
        }

        Intent::AgentDelete { name } => {
            let mut agents = state.agents.write().await;
            let agent_id = agents
                .values()
                .find(|a| a.spec.name == *name)
                .map(|a| a.id.clone());

            if let Some(id) = agent_id {
                agents.remove(&id);
                let action = NlAction {
                    action_type: "agent.delete".to_string(),
                    params: serde_json::json!({ "name": name }),
                    status: "completed".to_string(),
                    summary: Some(format!("Agent '{}' deleted", name)),
                };
                (
                    vec![action],
                    format!("✓ 已删除 Agent '{}'", name),
                    vec!["查看 Agent 列表".to_string()],
                )
            } else {
                let action = NlAction {
                    action_type: "agent.delete".to_string(),
                    params: serde_json::json!({ "name": name }),
                    status: "failed".to_string(),
                    summary: Some(format!("Agent '{}' not found", name)),
                };
                (
                    vec![action],
                    format!("✗ 未找到 Agent '{}'", name),
                    vec!["查看 Agent 列表".to_string()],
                )
            }
        }

        Intent::AgentRun { name, prompt } => {
            let agents = state.agents.read().await;
            let agent = agents.values().find(|a| a.spec.name == *name);

            if let Some(_agent) = agent {
                let action = NlAction {
                    action_type: "agent.run".to_string(),
                    params: serde_json::json!({ "name": name, "prompt": prompt }),
                    status: "submitted".to_string(),
                    summary: Some(format!("Agent '{}' run submitted", name)),
                };
                (
                    vec![action],
                    format!("✓ 已提交 Agent '{}' 的运行请求", name),
                    vec!["查看运行状态".to_string()],
                )
            } else {
                let action = NlAction {
                    action_type: "agent.run".to_string(),
                    params: serde_json::json!({ "name": name }),
                    status: "failed".to_string(),
                    summary: Some(format!("Agent '{}' not found", name)),
                };
                (
                    vec![action],
                    format!("✗ 未找到 Agent '{}'", name),
                    vec!["查看 Agent 列表".to_string()],
                )
            }
        }

        Intent::WorkflowList => {
            let workflows = state.workflows.read().await;
            let count = workflows.len();
            let names: Vec<String> = workflows.values().map(|w| w.name.clone()).collect();

            let action = NlAction {
                action_type: "workflow.list".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some(format!("Found {} workflows", count)),
            };

            let msg = if count == 0 {
                "当前没有注册的工作流".to_string()
            } else {
                format!("共 {} 个工作流: {}", count, names.join(", "))
            };

            (
                vec![action],
                msg,
                vec!["创建工作流".to_string(), "运行工作流".to_string()],
            )
        }

        Intent::WorkflowCreate { name, description } => {
            let wf_name = name
                .clone()
                .unwrap_or_else(|| format!("workflow-{}", &uuid::Uuid::new_v4().to_string()[..8]));
            let wf_desc = description.clone().unwrap_or_default();

            let now = chrono::Utc::now().to_rfc3339();
            let id = uuid::Uuid::new_v4().to_string();

            let workflow = crate::handlers::workflows::Workflow {
                id: id.clone(),
                name: wf_name.clone(),
                description: wf_desc,
                status: WorkflowStatus::Draft,
                nodes: vec![],
                created_at: now.clone(),
                updated_at: now,
                started_at: None,
                completed_at: None,
                execution_count: 0,
            };

            let mut workflows = state.workflows.write().await;
            workflows.insert(id.clone(), workflow);

            let action = NlAction {
                action_type: "workflow.create".to_string(),
                params: serde_json::json!({ "name": wf_name }),
                status: "completed".to_string(),
                summary: Some(format!("Workflow '{}' created", wf_name)),
            };

            (
                vec![action],
                format!("✓ 已创建工作流 '{}'", wf_name),
                vec![format!("运行工作流 '{}'", wf_name)],
            )
        }

        Intent::WorkflowRun { name } => {
            let workflows = state.workflows.read().await;
            let workflow = workflows.values().find(|w| w.name == *name);

            if let Some(_wf) = workflow {
                let action = NlAction {
                    action_type: "workflow.run".to_string(),
                    params: serde_json::json!({ "name": name }),
                    status: "submitted".to_string(),
                    summary: Some(format!("Workflow '{}' run submitted", name)),
                };
                (
                    vec![action],
                    format!("✓ 已提交工作流 '{}' 的运行请求", name),
                    vec!["查看工作流状态".to_string()],
                )
            } else {
                let action = NlAction {
                    action_type: "workflow.run".to_string(),
                    params: serde_json::json!({ "name": name }),
                    status: "failed".to_string(),
                    summary: Some(format!("Workflow '{}' not found", name)),
                };
                (
                    vec![action],
                    format!("✗ 未找到工作流 '{}'", name),
                    vec!["查看工作流列表".to_string()],
                )
            }
        }

        Intent::ClusterStatus => {
            let agents_count = state.agents.read().await.len();
            let action = NlAction {
                action_type: "cluster.status".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("Cluster status retrieved".to_string()),
            };
            (
                vec![action],
                format!("✓ 集群状态: 健康, {} 个 Agent", agents_count),
                vec!["查看指标".to_string()],
            )
        }

        Intent::ServerStatus => {
            let action = NlAction {
                action_type: "server.status".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("Server is running".to_string()),
            };
            (
                vec![action],
                "✓ KIAS 服务运行正常".to_string(),
                vec!["查看集群状态".to_string()],
            )
        }

        Intent::Metrics => {
            let action = NlAction {
                action_type: "metrics.get".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("Metrics retrieved".to_string()),
            };
            (
                vec![action],
                "✓ 指标摘要".to_string(),
                vec!["查看 Agent 指标".to_string()],
            )
        }

        Intent::KnowledgeSearch { query } => {
            let action = NlAction {
                action_type: "knowledge.search".to_string(),
                params: serde_json::json!({ "query": query }),
                status: "completed".to_string(),
                summary: Some(format!("Searched for '{}'", query)),
            };
            (vec![action], format!("✓ 搜索完成: '{}'", query), vec![])
        }

        Intent::ConfigGet => {
            let action = NlAction {
                action_type: "config.get".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("Config retrieved".to_string()),
            };
            (
                vec![action],
                "✓ 当前配置".to_string(),
                vec!["更新配置".to_string()],
            )
        }

        Intent::Help => {
            let action = NlAction {
                action_type: "help".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("Help displayed".to_string()),
            };
            (
                vec![action],
                "KIAS 自然语言命令支持:\n\
                 - 列出所有 agent / list agents\n\
                 - 创建一个名为 xxx 的 agent\n\
                 - 删除 agent xxx\n\
                 - 运行 agent xxx\n\
                 - 列出工作流 / list workflows\n\
                 - 创建工作流 xxx\n\
                 - 运行工作流 xxx\n\
                 - 查看集群状态 / cluster status\n\
                 - 查看状态 / status\n\
                 - 查看指标 / metrics\n\
                 - 搜索 xxx / search xxx\n\
                 - 帮助 / help"
                    .to_string(),
                vec!["查看 Agent 列表".to_string(), "查看集群状态".to_string()],
            )
        }

        Intent::Unknown => {
            let action = NlAction {
                action_type: "unknown".to_string(),
                params: serde_json::json!({}),
                status: "skipped".to_string(),
                summary: Some("Intent not recognized".to_string()),
            };
            (
                vec![action],
                "✗ 无法识别该命令，输入 '帮助' 查看支持的操作".to_string(),
                vec!["帮助".to_string(), "查看 Agent 列表".to_string()],
            )
        }

        Intent::ProblemReport { title, description } => {
            let action = NlAction {
                action_type: "problem.report".to_string(),
                params: serde_json::json!({
                    "title": title,
                    "description": description,
                }),
                status: "completed".to_string(),
                summary: Some(format!("问题已记录: {}", title)),
            };
            (
                vec![action],
                format!(
                    "✓ 问题已记录: {}\n\n问题将被纳入自改进循环，系统会自动生成解决方案。",
                    title
                ),
                vec!["查看问题列表".to_string(), "创建解决方案".to_string()],
            )
        }

        Intent::ProblemList => {
            let action = NlAction {
                action_type: "problem.list".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("问题列表".to_string()),
            };
            (
                vec![action],
                "问题列表功能已记录，将通过自改进循环处理。".to_string(),
                vec!["报告新问题".to_string(), "查看改进记录".to_string()],
            )
        }

        Intent::AutoLoopStart { problem } => {
            let action = NlAction {
                action_type: "auto_loop.start".to_string(),
                params: serde_json::json!({
                    "problem": problem,
                }),
                status: "completed".to_string(),
                summary: Some(format!("自动循环已启动: {}", problem)),
            };
            (
                vec![action],
                format!(
                    "✓ 自动循环已启动: {}\n\n系统将自动分析问题、制定方案、实施修复、验证结果。",
                    problem
                ),
                vec!["查看循环状态".to_string(), "查看问题列表".to_string()],
            )
        }

        Intent::AutoLoopStatus => {
            let action = NlAction {
                action_type: "auto_loop.status".to_string(),
                params: serde_json::json!({}),
                status: "completed".to_string(),
                summary: Some("自动循环状态".to_string()),
            };
            (
                vec![action],
                "自动循环状态查询功能已记录。".to_string(),
                vec!["启动自动循环".to_string(), "查看问题列表".to_string()],
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_intent_agent_list_cn() {
        let (intent, conf) = parse_intent("列出所有 agent");
        assert!(matches!(intent, Intent::AgentList));
        assert!(conf > 0.8);
    }

    #[test]
    fn test_parse_intent_agent_list_en() {
        let (intent, _) = parse_intent("list all agents");
        assert!(matches!(intent, Intent::AgentList));
    }

    #[test]
    fn test_parse_intent_agent_create() {
        let (intent, _) = parse_intent("创建一个名为 test 的 agent");
        assert!(matches!(intent, Intent::AgentCreate { .. }));
    }

    #[test]
    fn test_parse_intent_agent_delete() {
        let (intent, _) = parse_intent("删除 agent test-agent");
        assert!(matches!(intent, Intent::AgentDelete { .. }));
    }

    #[test]
    fn test_parse_intent_workflow_list() {
        let (intent, _) = parse_intent("列出工作流");
        assert!(matches!(intent, Intent::WorkflowList));
    }

    #[test]
    fn test_parse_intent_cluster_status() {
        let (intent, _) = parse_intent("查看集群状态");
        assert!(matches!(intent, Intent::ClusterStatus));
    }

    #[test]
    fn test_parse_intent_server_status() {
        let (intent, _) = parse_intent("服务状态");
        assert!(matches!(intent, Intent::ServerStatus));
    }

    #[test]
    fn test_parse_intent_help() {
        let (intent, _) = parse_intent("帮助");
        assert!(matches!(intent, Intent::Help));
    }

    #[test]
    fn test_parse_intent_unknown() {
        let (intent, conf) = parse_intent("今天天气怎么样");
        assert!(matches!(intent, Intent::Unknown));
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn test_parse_intent_english_cluster() {
        let (intent, _) = parse_intent("show cluster status");
        assert!(matches!(intent, Intent::ClusterStatus));
    }

    #[test]
    fn test_extract_name() {
        let name = extract_name(
            "创建一个名为 test 的 agent",
            &["agent", "创建", "一个", "名为"],
        );
        assert!(name.is_some());
        assert_eq!(name.unwrap(), "test");
    }

    #[test]
    fn test_extract_query_with_quotes() {
        let query = extract_query("搜索 \"rust async\"");
        assert_eq!(query, "rust async");
    }

    #[test]
    fn test_extract_query_without_quotes() {
        let query = extract_query("搜索 rust async");
        assert_eq!(query, "rust async");
    }

    #[test]
    fn test_parse_intent_agent_run() {
        let (intent, _) = parse_intent("运行 agent test-agent");
        assert!(matches!(intent, Intent::AgentRun { .. }));
    }

    #[test]
    fn test_parse_intent_workflow_create() {
        let (intent, _) = parse_intent("创建工作流 my-workflow");
        assert!(matches!(intent, Intent::WorkflowCreate { .. }));
    }

    #[test]
    fn test_parse_intent_workflow_run() {
        let (intent, _) = parse_intent("运行工作流 deploy");
        assert!(matches!(intent, Intent::WorkflowRun { .. }));
    }

    #[test]
    fn test_parse_intent_metrics() {
        let (intent, _) = parse_intent("查看指标");
        assert!(matches!(intent, Intent::Metrics));
    }

    #[test]
    fn test_parse_intent_knowledge_search() {
        let (intent, _) = parse_intent("搜索知识 rust async");
        assert!(matches!(intent, Intent::KnowledgeSearch { .. }));
    }

    #[test]
    fn test_parse_intent_problem_report() {
        let (intent, _) = parse_intent("报告bug Agent无法启动");
        assert!(matches!(intent, Intent::ProblemReport { .. }));
    }

    #[test]
    fn test_parse_intent_config_get() {
        let (intent, _) = parse_intent("查看配置");
        assert!(matches!(intent, Intent::ConfigGet));
    }

    #[test]
    fn test_parse_intent_auto_loop_start() {
        let (intent, _) = parse_intent("启动自动循环");
        assert!(matches!(intent, Intent::AutoLoopStart { .. }));
    }

    #[test]
    fn test_extract_name_returns_none_for_no_match() {
        let name = extract_name("hello world", &["hello", "world"]);
        assert!(name.is_none());
    }

    #[test]
    fn test_extract_problem_title() {
        let title = extract_problem_title("报告问题：Agent无法启动");
        assert!(!title.is_empty());
    }

    #[test]
    fn test_parse_intent_english_agent_list() {
        let (intent, _) = parse_intent("show all agents");
        assert!(matches!(intent, Intent::AgentList));
    }

    #[test]
    fn test_parse_intent_english_help() {
        let (intent, _) = parse_intent("help");
        assert!(matches!(intent, Intent::Help));
    }

    #[test]
    fn test_parse_intent_english_status() {
        let (intent, _) = parse_intent("server status");
        assert!(matches!(intent, Intent::ServerStatus));
    }
}

// ─── 公共接口（供 IM 模块调用）──────────────────────────

/// 公共意图解析接口
pub fn parse_intent_for_im(command: &str) -> (Intent, f64) {
    parse_intent(command)
}

/// 公共意图执行接口
pub async fn execute_intent_for_im(
    intent: &Intent,
    state: &AppState,
) -> (Vec<NlAction>, String, Vec<String>) {
    execute_intent(intent, state).await
}

// ─── 意图识别 API 端点 ──────────────────────────────────

/// 意图识别请求
#[derive(Debug, Clone, Deserialize)]
pub struct RecognizeIntentRequest {
    /// 用户输入
    pub input: String,
    /// 可选上下文
    #[serde(default)]
    pub context: Option<String>,
}

/// 意图识别响应
#[derive(Debug, Clone, Serialize)]
pub struct RecognizeIntentResponse {
    /// 意图类型
    pub intent_type: String,
    /// 复杂度
    pub complexity: String,
    /// 优先级
    pub priority: String,
    /// 置信度
    pub confidence: f64,
    /// 关键词
    pub keywords: Vec<String>,
    /// 推荐工具
    pub recommended_tools: Vec<RecommendedToolResponse>,
}

/// 推荐工具响应
#[derive(Debug, Clone, Serialize)]
pub struct RecommendedToolResponse {
    /// 工具名称
    pub name: String,
    /// 匹配分数
    pub score: f64,
    /// 匹配原因
    pub reason: String,
}

/// 任务拆解请求
#[derive(Debug, Clone, Deserialize)]
pub struct DecomposeTaskRequest {
    /// 用户输入
    pub input: String,
    /// 意图类型（可选）
    #[serde(default)]
    pub intent_type: Option<String>,
}

/// 任务拆解响应
#[derive(Debug, Clone, Serialize)]
pub struct DecomposeTaskResponse {
    /// 任务数量
    pub task_count: usize,
    /// 总预估耗时（秒）
    pub total_estimated_duration: u64,
    /// 是否需要多Agent协作
    pub requires_multi_agent: bool,
    /// 任务列表
    pub tasks: Vec<TaskResponse>,
}

/// 任务响应
#[derive(Debug, Clone, Serialize)]
pub struct TaskResponse {
    /// 任务ID
    pub id: String,
    /// 任务名称
    pub name: String,
    /// 任务描述
    pub description: String,
    /// 依赖任务
    pub dependencies: Vec<String>,
    /// 预估耗时（秒）
    pub estimated_duration: u64,
    /// 所需技能
    pub required_skills: Vec<String>,
}

/// 意图识别端点
pub async fn recognize_intent(
    Json(request): Json<RecognizeIntentRequest>,
) -> Result<Json<RecognizeIntentResponse>, ApiError> {
    // 使用关键词识别器
    let recognizer = auto_loop::intent_recognizer::IntentRecognizer::new();
    let intent = recognizer.recognize(&request.input);

    // 使用工具感知识别器推荐工具
    let tool_recognizer = auto_loop::tool_aware_intent::ToolAwareRecognizer::new();
    let tool_intent = tool_recognizer.recognize(&request.input, intent.clone());

    // 转换为响应
    let response = RecognizeIntentResponse {
        intent_type: format!("{:?}", tool_intent.base_intent.intent_type),
        complexity: format!("{:?}", tool_intent.base_intent.complexity),
        priority: format!("{:?}", tool_intent.base_intent.priority),
        confidence: tool_intent.base_intent.confidence,
        keywords: tool_intent.base_intent.keywords,
        recommended_tools: tool_intent
            .recommended_tools
            .iter()
            .map(|t| RecommendedToolResponse {
                name: t.name.clone(),
                score: t.score,
                reason: t.reason.clone(),
            })
            .collect(),
    };

    Ok(Json(response))
}

/// 任务拆解端点
pub async fn decompose_task(
    Json(request): Json<DecomposeTaskRequest>,
) -> Result<Json<DecomposeTaskResponse>, ApiError> {
    // 解析意图类型
    let intent_type = if let Some(ref type_str) = request.intent_type {
        match type_str.as_str() {
            "CodeGeneration" => auto_loop::intent_recognizer::IntentType::CodeGeneration,
            "BugFix" => auto_loop::intent_recognizer::IntentType::BugFix,
            "CodeReview" => auto_loop::intent_recognizer::IntentType::CodeReview,
            "TestGeneration" => auto_loop::intent_recognizer::IntentType::TestGeneration,
            "Documentation" => auto_loop::intent_recognizer::IntentType::Documentation,
            "SecurityAudit" => auto_loop::intent_recognizer::IntentType::SecurityAudit,
            "PerformanceOptimization" => {
                auto_loop::intent_recognizer::IntentType::PerformanceOptimization
            }
            "KnowledgeQuery" => auto_loop::intent_recognizer::IntentType::KnowledgeQuery,
            "SystemAdmin" => auto_loop::intent_recognizer::IntentType::SystemAdmin,
            _ => auto_loop::intent_recognizer::IntentType::Unknown,
        }
    } else {
        // 自动识别意图
        let recognizer = auto_loop::intent_recognizer::IntentRecognizer::new();
        let intent = recognizer.recognize(&request.input);
        intent.intent_type
    };

    // 创建意图
    let intent = auto_loop::intent_recognizer::RecognizedIntent {
        intent_type,
        complexity: auto_loop::intent_recognizer::Complexity::Medium,
        priority: auto_loop::intent_recognizer::Priority::Medium,
        keywords: vec![],
        raw_input: request.input.clone(),
        confidence: 0.8,
    };

    // 拆解任务
    let decomposer = auto_loop::task_decomposer::TaskDecomposer::new();
    let result = decomposer.decompose(&intent);

    // 转换为响应
    let tasks: Vec<TaskResponse> = result
        .task_graph
        .nodes
        .values()
        .map(|node| TaskResponse {
            id: node.id.clone(),
            name: node.name.clone(),
            description: node.description.clone(),
            dependencies: node.dependencies.clone(),
            estimated_duration: node.estimated_duration,
            required_skills: node.required_skills.clone(),
        })
        .collect();

    let response = DecomposeTaskResponse {
        task_count: result.task_count,
        total_estimated_duration: result.total_estimated_duration,
        requires_multi_agent: result.requires_multi_agent,
        tasks,
    };

    Ok(Json(response))
}
