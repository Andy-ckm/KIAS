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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        StreamChoice, StreamChunk, StreamDelta, StreamFunctionCall, StreamToolCall,
    };

    fn make_chunk(choices: Vec<StreamChoice>) -> StreamChunk {
        StreamChunk {
            id: "chatcmpl-123".to_string(),
            model: "gpt-4".to_string(),
            choices,
        }
    }

    fn text_choice(content: &str) -> StreamChoice {
        StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: Some(content.to_string()),
                tool_calls: None,
            },
            finish_reason: None,
        }
    }

    fn done_choice(reason: &str) -> StreamChoice {
        StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: None,
            },
            finish_reason: Some(reason.to_string()),
        }
    }

    fn tool_call_choice(id: &str, name: &str, args: Option<&str>) -> StreamChoice {
        StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![StreamToolCall {
                    index: 0,
                    id: Some(id.to_string()),
                    function: Some(StreamFunctionCall {
                        name: Some(name.to_string()),
                        arguments: args.map(|a| a.to_string()),
                    }),
                }]),
            },
            finish_reason: None,
        }
    }

    fn tool_call_delta_choice(id: &str, args: &str) -> StreamChoice {
        StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![StreamToolCall {
                    index: 0,
                    id: Some(id.to_string()),
                    function: Some(StreamFunctionCall {
                        name: None,
                        arguments: Some(args.to_string()),
                    }),
                }]),
            },
            finish_reason: None,
        }
    }

    #[test]
    fn test_new_processor_is_empty() {
        let proc = StreamProcessor::new();
        assert!(proc.get_tool_calls().is_empty());
    }

    #[test]
    fn test_default_processor() {
        let proc = StreamProcessor::default();
        assert!(proc.get_tool_calls().is_empty());
    }

    #[test]
    fn test_process_text_chunk() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![text_choice("Hello")]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Text { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected Text event"),
        }
    }

    #[test]
    fn test_process_empty_content_ignored() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![text_choice("")]);
        let events = proc.process_chunk(&chunk);
        assert!(events.is_empty());
    }

    #[test]
    fn test_process_done_chunk() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![done_choice("stop")]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Done {
                finish_reason,
                usage,
            } => {
                assert_eq!(finish_reason, "stop");
                assert!(usage.is_none());
            }
            _ => panic!("Expected Done event"),
        }
    }

    #[test]
    fn test_process_tool_call_start() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![tool_call_choice(
            "call_1",
            "search",
            Some(r#"{"q":"test"}"#),
        )]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 2); // ToolCallStart + ToolCallDelta
        match &events[0] {
            StreamEvent::ToolCallStart { id, name } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "search");
            }
            _ => panic!("Expected ToolCallStart"),
        }
        match &events[1] {
            StreamEvent::ToolCallDelta { id, arguments } => {
                assert_eq!(id, "call_1");
                assert_eq!(arguments, r#"{"q":"test"}"#);
            }
            _ => panic!("Expected ToolCallDelta"),
        }
    }

    #[test]
    fn test_process_tool_call_name_only_no_delta() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![tool_call_choice("call_1", "search", None)]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 1); // Only ToolCallStart, no delta
        match &events[0] {
            StreamEvent::ToolCallStart { .. } => {}
            _ => panic!("Expected ToolCallStart"),
        }
    }

    #[test]
    fn test_debug_accumulation() {
        let mut proc = StreamProcessor::new();

        // Chunk 1: start tool call with partial args
        let chunk1 = make_chunk(vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![StreamToolCall {
                    index: 0,
                    id: Some("call_1".to_string()),
                    function: Some(StreamFunctionCall {
                        name: Some("search".to_string()),
                        arguments: Some("{\"q".to_string()),
                    }),
                }]),
            },
            finish_reason: None,
        }]);
        let events1 = proc.process_chunk(&chunk1);
        eprintln!("events1: {:?}", events1);
        eprintln!("tool_calls after chunk1: {:?}", proc.get_tool_calls());

        // Chunk 2: more args
        let chunk2 = make_chunk(vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![StreamToolCall {
                    index: 0,
                    id: Some("call_1".to_string()),
                    function: Some(StreamFunctionCall {
                        name: None,
                        arguments: Some(":\"test\"}".to_string()),
                    }),
                }]),
            },
            finish_reason: None,
        }]);
        let events2 = proc.process_chunk(&chunk2);
        eprintln!("events2: {:?}", events2);

        let tool_calls = proc.get_tool_calls();
        eprintln!("final tool_calls: {:?}", tool_calls);
        assert_eq!(tool_calls.len(), 1);
    }

    #[test]
    fn test_process_tool_call_accumulation() {
        let mut proc = StreamProcessor::new();
        // First chunk: tool call start with complete JSON args
        let chunk1 = make_chunk(vec![tool_call_choice(
            "call_1",
            "search",
            Some(r#"{"q":"test"}"#),
        )]);
        let events1 = proc.process_chunk(&chunk1);
        assert_eq!(events1.len(), 2); // ToolCallStart + ToolCallDelta
                                      // Check tool calls are accumulated
        let tool_calls = proc.get_tool_calls();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].name, "search");
        assert_eq!(tool_calls[0].arguments, serde_json::json!({"q": "test"}));
        // Second chunk: another tool call
        let chunk2 = make_chunk(vec![tool_call_choice(
            "call_2",
            "calculate",
            Some(r#"{"x":42}"#),
        )]);
        proc.process_chunk(&chunk2);
        let tool_calls = proc.get_tool_calls();
        assert_eq!(tool_calls.len(), 2);
    }

    #[test]
    fn test_process_tool_call_invalid_json() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![tool_call_choice("call_1", "search", Some("not json"))]);
        proc.process_chunk(&chunk);
        let tool_calls = proc.get_tool_calls();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].arguments, serde_json::Value::Null);
    }

    #[test]
    fn test_process_multiple_choices() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![text_choice("Hello"), text_choice("World")]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_process_mixed_events() {
        let mut proc = StreamProcessor::new();
        let chunk = make_chunk(vec![
            text_choice("Sure, "),
            tool_call_choice("call_1", "calc", Some(r#"{"x":1}"#)),
            done_choice("stop"),
        ]);
        let events = proc.process_chunk(&chunk);
        // Text + ToolCallStart + ToolCallDelta + Done = 4
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_tool_call_without_id_gets_generated_id() {
        let mut proc = StreamProcessor::new();
        let choice = StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: None,
                content: None,
                tool_calls: Some(vec![StreamToolCall {
                    index: 0,
                    id: None,
                    function: Some(StreamFunctionCall {
                        name: Some("test".to_string()),
                        arguments: None,
                    }),
                }]),
            },
            finish_reason: None,
        };
        let chunk = make_chunk(vec![choice]);
        let events = proc.process_chunk(&chunk);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolCallStart { id, name } => {
                assert_eq!(id, "call_0");
                assert_eq!(name, "test");
            }
            _ => panic!("Expected ToolCallStart"),
        }
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::Text {
            content: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""content":"hello""#));
    }

    #[test]
    fn test_stream_event_done_serialization() {
        let event = StreamEvent::Done {
            finish_reason: "stop".to_string(),
            usage: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""type":"done""#));
    }
}
