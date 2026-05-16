//! Agent 执行器 — Codex 风格的 Agentic Loop
//!
//! 核心循环:
//! 1. User → System Prompt + User Message
//! 2. LLM → Tool Calls
//! 3. Execute Tools → Observations
//! 4. Loop until done

use std::sync::Arc;
use tokio::sync::RwLock;
use llm_engine::{LlmProvider, ChatRequest, ChatMessage, MessageRole, TokenUsage};
use tool_executor::ToolRegistry;
use crate::types::*;
use crate::context::AgentContext;

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
                let cost = self.cost_tracker.record_usage(&self.config.model, usage).await;
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
