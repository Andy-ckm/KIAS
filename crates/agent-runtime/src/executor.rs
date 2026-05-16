//! Agent 执行器 — Codex 风格的 Agentic Loop
//!
//! 核心循环:
//! 1. User → System Prompt + User Message
//! 2. LLM → Tool Calls
//! 3. Execute Tools → Observations
//! 4. Loop until done

use crate::context::AgentContext;
use crate::types::*;
use llm_engine::{ChatMessage, ChatRequest, LlmProvider, MessageRole};
use std::sync::Arc;
use tokio::sync::RwLock;
use tool_executor::ToolRegistry;

/// Agent 执行器
pub struct AgentExecutor {
    provider: Arc<dyn LlmProvider>,
    tool_registry: Arc<ToolRegistry>,
    config: AgentConfig,
    context: Arc<RwLock<AgentContext>>,
    cost_tracker: Arc<llm_engine::CostTracker>,
}

impl AgentExecutor {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tool_registry: Arc<ToolRegistry>,
        config: AgentConfig,
        context: Arc<RwLock<AgentContext>>,
        cost_tracker: Arc<llm_engine::CostTracker>,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            config,
            context,
            cost_tracker,
        }
    }

    /// 执行 Agent — 核心 Agentic Loop
    pub async fn execute(&self, prompt: &str) -> AgentResult {
        let start_time = std::time::Instant::now();
        let mut iterations = 0;
        let mut total_tokens = 0u64;
        let mut total_cost = 0.0f64;
        let mut tool_calls_history = Vec::new();
        let mut messages = Vec::new();

        // 构建系统提示
        let context = self.context.read().await;
        let system_prompt = context.get_system_prompt(&self.config.system_prompt);
        drop(context);

        // 添加系统消息
        messages.push(ChatMessage {
            role: MessageRole::System,
            content: system_prompt,
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });

        // 添加用户消息
        messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.to_string(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });

        // 准备工具定义
        let tools = self.prepare_tools();

        // Agentic Loop
        loop {
            iterations += 1;

            // 检查迭代次数限制
            if iterations > self.config.max_iterations {
                return AgentResult {
                    success: false,
                    output: "Maximum iterations reached".to_string(),
                    iterations,
                    tokens_used: total_tokens,
                    cost: total_cost,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    tool_calls: tool_calls_history,
                    error: Some("Max iterations exceeded".to_string()),
                };
            }

            // 调用 LLM
            let request = ChatRequest {
                model: self.config.model.clone(),
                messages: messages.clone(),
                temperature: Some(self.config.temperature),
                max_tokens: Some(self.config.max_tokens),
                tools: Some(tools.clone()),
                stream: Some(false),
            };

            let response = match self.provider.chat(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    return AgentResult {
                        success: false,
                        output: format!("LLM error: {}", e),
                        iterations,
                        tokens_used: total_tokens,
                        cost: total_cost,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        tool_calls: tool_calls_history,
                        error: Some(e.to_string()),
                    };
                }
            };

            // 更新 token 统计
            if let Some(usage) = &response.usage {
                total_tokens += usage.total_tokens;
                let cost = self
                    .cost_tracker
                    .record_usage(&self.config.model, usage)
                    .await;
                total_cost += cost;
            }

            // 获取 assistant 响应
            let choice = match response.choices.first() {
                Some(c) => c,
                None => {
                    return AgentResult {
                        success: false,
                        output: "No response from LLM".to_string(),
                        iterations,
                        tokens_used: total_tokens,
                        cost: total_cost,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        tool_calls: tool_calls_history,
                        error: Some("Empty LLM response".to_string()),
                    };
                }
            };

            let assistant_message = &choice.message;

            // 添加 assistant 消息到历史
            messages.push(assistant_message.clone());

            // 检查是否有工具调用
            if let Some(tool_calls) = &assistant_message.tool_calls {
                if tool_calls.is_empty() {
                    // 没有工具调用，完成
                    return AgentResult {
                        success: true,
                        output: assistant_message.content.clone(),
                        iterations,
                        tokens_used: total_tokens,
                        cost: total_cost,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                        tool_calls: tool_calls_history,
                        error: None,
                    };
                }

                // 执行工具调用
                for tc in tool_calls {
                    let tool_start = std::time::Instant::now();
                    let name = tc.function.name.clone();
                    let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null);

                    // 执行工具
                    let result = self.tool_registry.execute(&name, args.clone()).await;

                    let record = ToolCallRecord {
                        name: name.clone(),
                        arguments: args,
                        result: result.output.clone(),
                        success: result.success,
                        duration_ms: tool_start.elapsed().as_millis() as u64,
                    };
                    tool_calls_history.push(record);

                    // 添加工具结果到消息历史
                    messages.push(ChatMessage {
                        role: MessageRole::Tool,
                        content: result.output,
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                    });
                }
            } else {
                // 没有工具调用，完成
                return AgentResult {
                    success: true,
                    output: assistant_message.content.clone(),
                    iterations,
                    tokens_used: total_tokens,
                    cost: total_cost,
                    duration_ms: start_time.elapsed().as_millis() as u64,
                    tool_calls: tool_calls_history,
                    error: None,
                };
            }
        }
    }

    /// 准备工具定义
    fn prepare_tools(&self) -> Vec<llm_engine::ToolDefinition> {
        let tool_infos = self.tool_registry.list();
        let mut tools = Vec::new();

        for info in tool_infos {
            // 只包含配置中允许的工具
            if self.config.tools.contains(&info.name) {
                tools.push(llm_engine::ToolDefinition {
                    tool_type: "function".to_string(),
                    function: llm_engine::FunctionDefinition {
                        name: info.name,
                        description: info.description,
                        parameters: info.parameters,
                    },
                });
            }
        }

        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use llm_engine::types::Choice;
    use llm_engine::{
        ChatMessage, ChatRequest, ChatResponse, FunctionCall, LlmError, MessageRole, TokenUsage,
        ToolCall,
    };
    use std::sync::atomic::{AtomicU32, Ordering};
    use tool_executor::builtin::{Tool, ToolResult};

    // ── Mock LlmProvider ──────────────────────────────────────────────

    /// Response configuration for the mock provider.
    enum MockResponse {
        /// Return a text-only response (no tool calls).
        Text(String),
        /// Return tool calls on the first N invocations, then text.
        ToolCallsThenText {
            tool_calls: Vec<ToolCall>,
            rounds: u32,
            final_text: String,
        },
        /// Always return an error.
        Error(String),
        /// Return a response with empty choices.
        Empty,
    }

    struct MockProvider {
        response: MockResponse,
        call_count: AtomicU32,
    }

    impl MockProvider {
        fn new(response: MockResponse) -> Self {
            Self {
                response,
                call_count: AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        fn models(&self) -> Vec<String> {
            vec!["mock-model".into()]
        }

        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, LlmError> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match &self.response {
                MockResponse::Text(text) => Ok(make_response(text.clone(), None)),
                MockResponse::ToolCallsThenText {
                    tool_calls,
                    rounds,
                    final_text,
                } => {
                    if count < *rounds {
                        Ok(make_response(String::new(), Some(tool_calls.clone())))
                    } else {
                        Ok(make_response(final_text.clone(), None))
                    }
                }
                MockResponse::Error(msg) => Err(LlmError::Provider(msg.clone())),
                MockResponse::Empty => Ok(ChatResponse {
                    id: "empty".into(),
                    model: "mock-model".into(),
                    choices: vec![],
                    usage: None,
                }),
            }
        }

        async fn chat_stream(
            &self,
            _request: ChatRequest,
        ) -> Result<Vec<llm_engine::StreamChunk>, LlmError> {
            Ok(vec![])
        }

        fn supports_tools(&self) -> bool {
            true
        }
        fn supports_streaming(&self) -> bool {
            true
        }
    }

    fn make_response(text: String, tool_calls: Option<Vec<ToolCall>>) -> ChatResponse {
        ChatResponse {
            id: "mock-resp".into(),
            model: "mock-model".into(),
            choices: vec![Choice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: text,
                    name: None,
                    tool_calls,
                    tool_call_id: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
        }
    }

    // ── Mock Tool ─────────────────────────────────────────────────────

    struct MockTool {
        tool_name: String,
        output: String,
    }

    impl MockTool {
        fn new(name: &str, output: &str) -> Self {
            Self {
                tool_name: name.to_string(),
                output: output.to_string(),
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.tool_name
        }
        fn description(&self) -> &str {
            "mock tool for testing"
        }
        fn parameters(&self) -> serde_json::Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _params: serde_json::Value) -> ToolResult {
            ToolResult {
                success: true,
                output: self.output.clone(),
                error: None,
                metadata: None,
            }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────

    fn test_config() -> AgentConfig {
        AgentConfig {
            name: "test-agent".into(),
            model: "mock-model".into(),
            system_prompt: "You are a test agent.".into(),
            max_iterations: 5,
            max_tokens: 1024,
            temperature: 0.7,
            tools: vec!["echo".into()],
            sandbox: SandboxConfig::default(),
        }
    }

    fn make_executor(
        provider: MockProvider,
        tools: Vec<Box<dyn Tool>>,
        config: AgentConfig,
    ) -> AgentExecutor {
        let mut registry = ToolRegistry::new();
        for tool in tools {
            registry.register(tool);
        }
        AgentExecutor::new(
            Arc::new(provider),
            Arc::new(registry),
            config,
            Arc::new(RwLock::new(AgentContext::new("test", "."))),
            Arc::new(llm_engine::CostTracker::default()),
        )
    }

    fn make_tool_call(id: &str, name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_execute_text_only_no_tools() {
        let provider = MockProvider::new(MockResponse::Text("Hello, world!".into()));
        let executor = make_executor(provider, vec![], test_config());

        let result = executor.execute("Say hello").await;
        assert!(result.success);
        assert_eq!(result.output, "Hello, world!");
        assert_eq!(result.iterations, 1);
        assert!(result.error.is_none());
        assert!(result.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn test_execute_with_tool_call() {
        let tc = make_tool_call("call_1", "echo", r#"{"msg":"hi"}"#);
        let provider = MockProvider::new(MockResponse::ToolCallsThenText {
            tool_calls: vec![tc],
            rounds: 1,
            final_text: "Done!".into(),
        });
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(MockTool::new("echo", "echoed"))];
        let config = AgentConfig {
            tools: vec!["echo".into()],
            ..test_config()
        };
        let executor = make_executor(provider, tools, config);

        let result = executor.execute("echo hi").await;
        assert!(result.success);
        assert_eq!(result.output, "Done!");
        assert_eq!(result.iterations, 2);
        assert_eq!(result.tool_calls.len(), 1);
        assert_eq!(result.tool_calls[0].name, "echo");
        assert!(result.tool_calls[0].success);
        assert_eq!(result.tool_calls[0].result, "echoed");
    }

    #[tokio::test]
    async fn test_execute_max_iterations_exceeded() {
        let tc = make_tool_call("call_1", "echo", "{}");
        let provider = MockProvider::new(MockResponse::ToolCallsThenText {
            tool_calls: vec![tc],
            rounds: 100,
            final_text: "should not reach".into(),
        });
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(MockTool::new("echo", "out"))];
        let config = AgentConfig {
            max_iterations: 3,
            tools: vec!["echo".into()],
            ..test_config()
        };
        let executor = make_executor(provider, tools, config);

        let result = executor.execute("loop forever").await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("Max iterations"));
        assert_eq!(result.iterations, 4);
    }

    #[tokio::test]
    async fn test_execute_llm_error() {
        let provider = MockProvider::new(MockResponse::Error("API key invalid".into()));
        let executor = make_executor(provider, vec![], test_config());

        let result = executor.execute("do something").await;
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("API key invalid"));
        assert_eq!(result.iterations, 1);
    }

    #[tokio::test]
    async fn test_execute_empty_choices() {
        let provider = MockProvider::new(MockResponse::Empty);
        let executor = make_executor(provider, vec![], test_config());

        let result = executor.execute("hello").await;
        assert!(!result.success);
        assert!(result
            .error
            .as_ref()
            .unwrap()
            .contains("Empty LLM response"));
    }

    #[tokio::test]
    async fn test_execute_multiple_tool_calls_in_one_turn() {
        let tcs = vec![
            make_tool_call("c1", "echo", r#"{"a":1}"#),
            make_tool_call("c2", "echo", r#"{"a":2}"#),
        ];
        let provider = MockProvider::new(MockResponse::ToolCallsThenText {
            tool_calls: tcs,
            rounds: 1,
            final_text: "Both done".into(),
        });
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(MockTool::new("echo", "ok"))];
        let config = AgentConfig {
            tools: vec!["echo".into()],
            ..test_config()
        };
        let executor = make_executor(provider, tools, config);

        let result = executor.execute("call two tools").await;
        assert!(result.success);
        assert_eq!(result.tool_calls.len(), 2);
        assert_eq!(result.tool_calls[0].name, "echo");
        assert_eq!(result.tool_calls[1].name, "echo");
    }

    #[tokio::test]
    async fn test_execute_tokens_tracked() {
        let provider = MockProvider::new(MockResponse::Text("ok".into()));
        let executor = make_executor(provider, vec![], test_config());

        let result = executor.execute("count tokens").await;
        assert!(result.success);
        assert_eq!(result.tokens_used, 150); // from mock TokenUsage
                                             // cost is 0.0 because "mock-model" has no pricing in CostTracker
        assert_eq!(result.cost, 0.0);
    }

    #[tokio::test]
    async fn test_prepare_tools_filters_by_config() {
        let provider = MockProvider::new(MockResponse::Text("ok".into()));
        let tools: Vec<Box<dyn Tool>> = vec![
            Box::new(MockTool::new("echo", "out")),
            Box::new(MockTool::new("file_read", "content")),
        ];
        let config = AgentConfig {
            tools: vec!["echo".into()],
            ..test_config()
        };
        let executor = make_executor(provider, tools, config);

        let prepared = executor.prepare_tools();
        assert_eq!(prepared.len(), 1);
        assert_eq!(prepared[0].function.name, "echo");
    }

    #[tokio::test]
    async fn test_execute_duration_ms_populated() {
        let provider = MockProvider::new(MockResponse::Text("fast".into()));
        let executor = make_executor(provider, vec![], test_config());

        let result = executor.execute("quick").await;
        assert!(result.success);
        let _ = result.duration_ms;
    }

    #[tokio::test]
    async fn test_execute_two_iterations_with_tool() {
        let tc = make_tool_call("c1", "echo", r#"{}"#);
        let provider = MockProvider::new(MockResponse::ToolCallsThenText {
            tool_calls: vec![tc],
            rounds: 1,
            final_text: "after tool".into(),
        });
        let tools: Vec<Box<dyn Tool>> = vec![Box::new(MockTool::new("echo", "result"))];
        let config = AgentConfig {
            tools: vec!["echo".into()],
            ..test_config()
        };
        let executor = make_executor(provider, tools, config);

        let result = executor.execute("use tool then respond").await;
        assert!(result.success);
        assert_eq!(result.iterations, 2);
        assert_eq!(result.tool_calls.len(), 1);
    }
}
