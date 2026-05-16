//! 流式输出处理

use serde::{Deserialize, Serialize};

/// 流式事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// 文本增量
    #[serde(rename = "text")]
    Text { content: String },

    /// 工具调用开始
    #[serde(rename = "tool_call_start")]
    ToolCallStart { id: String, name: String },

    /// 工具调用参数增量
    #[serde(rename = "tool_call_delta")]
    ToolCallDelta { id: String, arguments: String },

    /// 工具调用完成
    #[serde(rename = "tool_call_end")]
    ToolCallEnd { id: String, result: String },

    /// 完成
    #[serde(rename = "done")]
    Done {
        finish_reason: String,
        usage: Option<crate::types::TokenUsage>,
    },

    /// 错误
    #[serde(rename = "error")]
    Error { message: String },
}

/// 流式处理器
pub struct StreamProcessor {
    _events: Vec<StreamEvent>,
    current_tool_calls: std::collections::HashMap<String, ToolCallState>,
}

/// 工具调用状态
struct ToolCallState {
    id: String,
    name: String,
    arguments: String,
}

impl Default for StreamProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamProcessor {
    pub fn new() -> Self {
        Self {
            _events: Vec::new(),
            current_tool_calls: std::collections::HashMap::new(),
        }
    }

    /// 处理流式块
    pub fn process_chunk(&mut self, chunk: &crate::types::StreamChunk) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        for choice in &chunk.choices {
            // 处理文本内容
            if let Some(content) = &choice.delta.content {
                if !content.is_empty() {
                    events.push(StreamEvent::Text {
                        content: content.clone(),
                    });
                }
            }

            // 处理工具调用
            if let Some(tool_calls) = &choice.delta.tool_calls {
                for tc in tool_calls {
                    let id = tc
                        .id
                        .clone()
                        .unwrap_or_else(|| format!("call_{}", tc.index));

                    if let Some(func) = &tc.function {
                        if let Some(name) = &func.name {
                            // 新工具调用开始
                            self.current_tool_calls.insert(
                                id.clone(),
                                ToolCallState {
                                    id: id.clone(),
                                    name: name.clone(),
                                    arguments: String::new(),
                                },
                            );
                            events.push(StreamEvent::ToolCallStart {
                                id: id.clone(),
                                name: name.clone(),
                            });
                        }

                        if let Some(args) = &func.arguments {
                            // 工具调用参数增量
                            if let Some(state) = self.current_tool_calls.get_mut(&id) {
                                state.arguments.push_str(args);
                                events.push(StreamEvent::ToolCallDelta {
                                    id: id.clone(),
                                    arguments: args.clone(),
                                });
                            }
                        }
                    }
                }
            }

            // 处理完成
            if let Some(finish_reason) = &choice.finish_reason {
                events.push(StreamEvent::Done {
                    finish_reason: finish_reason.clone(),
                    usage: None,
                });
            }
        }

        events
    }

    /// 获取所有工具调用结果
    pub fn get_tool_calls(&self) -> Vec<ToolCallResult> {
        self.current_tool_calls
            .values()
            .map(|state| ToolCallResult {
                id: state.id.clone(),
                name: state.name.clone(),
                arguments: serde_json::from_str(&state.arguments)
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect()
    }
}

/// 工具调用结果
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}
