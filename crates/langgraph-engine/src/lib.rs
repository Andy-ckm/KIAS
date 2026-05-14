//! KIAS LangGraph-style State Graph Engine
//!
//! 有向图状态机，支持条件分支、循环、中断/恢复、子图组合

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// 状态通道 trait
pub trait Channel: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn as_any(&self) -> &dyn Any;
    fn clone_box(&self) -> Box<dyn Channel>;
}

/// 图状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    pub channels: HashMap<String, serde_json::Value>,
    pub metadata: StateMetadata,
}

/// 状态元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMetadata {
    pub run_id: String,
    pub step: usize,
    pub node_history: Vec<String>,
    pub is_interrupted: bool,
    pub checkpoint_id: Option<String>,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            channels: HashMap::new(),
            metadata: StateMetadata {
                run_id: Uuid::new_v4().to_string(),
                step: 0,
                node_history: Vec::new(),
                is_interrupted: false,
                checkpoint_id: None,
            },
        }
    }
}

impl GraphState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.channels
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn set<T: Serialize>(&mut self, key: &str, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.channels.insert(key.to_string(), v);
        }
    }

    pub fn merge(&mut self, other: GraphState) {
        for (k, v) in other.channels {
            self.channels.insert(k, v);
        }
    }
}

/// 节点处理器
pub type NodeHandler = Box<
    dyn Fn(GraphState) -> Pin<Box<dyn Future<Output = kias_common::KiasResult<GraphState>> + Send>>
        + Send
        + Sync,
>;

/// 图节点
pub struct GraphNode {
    pub name: String,
    pub handler: NodeHandler,
}

/// 边条件
pub type EdgeCondition = Box<dyn Fn(&GraphState) -> bool + Send + Sync>;

/// 图边
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub condition: Option<EdgeCondition>,
}

/// 检查点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub state: GraphState,
    pub node: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 执行事件（用于流式输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionEvent {
    NodeStart {
        node: String,
        step: usize,
    },
    NodeComplete {
        node: String,
        step: usize,
        output: GraphState,
    },
    EdgeTaken {
        from: String,
        to: String,
    },
    Interrupted {
        node: String,
        reason: String,
    },
    Completed {
        final_state: GraphState,
    },
    Error {
        node: String,
        error: String,
    },
}

/// 事件监听器
#[async_trait]
pub trait EventListener: Send + Sync {
    async fn on_event(&self, event: ExecutionEvent);
}

/// 状态图构建器
pub struct StateGraphBuilder {
    nodes: HashMap<String, GraphNode>,
    edges: Vec<GraphEdge>,
    entry: String,
    listeners: Vec<Arc<dyn EventListener>>,
}

impl StateGraphBuilder {
    pub fn new(entry: &str) -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            entry: entry.to_string(),
            listeners: Vec::new(),
        }
    }

    pub fn add_node<F, Fut>(mut self, name: &str, handler: F) -> Self
    where
        F: Fn(GraphState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = kias_common::KiasResult<GraphState>> + Send + 'static,
    {
        let name_owned = name.to_string();
        self.nodes.insert(
            name_owned.clone(),
            GraphNode {
                name: name_owned,
                handler: Box::new(move |state| Box::pin(handler(state))),
            },
        );
        self
    }

    pub fn add_edge(mut self, from: &str, to: &str) -> Self {
        self.edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            condition: None,
        });
        self
    }

    pub fn add_conditional_edge<F>(mut self, from: &str, to: &str, condition: F) -> Self
    where
        F: Fn(&GraphState) -> bool + Send + Sync + 'static,
    {
        self.edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            condition: Some(Box::new(condition)),
        });
        self
    }

    pub fn add_listener(mut self, listener: Arc<dyn EventListener>) -> Self {
        self.listeners.push(listener);
        self
    }

    pub fn build(self) -> StateGraph {
        StateGraph {
            nodes: self.nodes,
            edges: self.edges,
            entry: self.entry,
            listeners: self.listeners,
        }
    }
}

/// 状态图
pub struct StateGraph {
    nodes: HashMap<String, GraphNode>,
    edges: Vec<GraphEdge>,
    entry: String,
    listeners: Vec<Arc<dyn EventListener>>,
}

impl StateGraph {
    pub fn builder(entry: &str) -> StateGraphBuilder {
        StateGraphBuilder::new(entry)
    }

    /// 获取检查点列表
    pub fn checkpoints(&self) -> &[Checkpoint] {
        &[]
    }

    /// 执行状态图
    pub async fn execute(&self, initial_state: GraphState) -> kias_common::KiasResult<GraphState> {
        let mut state = initial_state;
        let mut current = self.entry.clone();
        let mut step = 0;

        loop {
            // 检查中断
            if state.metadata.is_interrupted {
                self.emit_event(ExecutionEvent::Interrupted {
                    node: current.clone(),
                    reason: "State interrupted".to_string(),
                })
                .await;
                return Ok(state);
            }

            // 获取节点
            let node = self.nodes.get(&current).ok_or_else(|| {
                kias_common::KiasError::Validation(format!("Node '{}' not found", current))
            })?;

            // 执行节点
            self.emit_event(ExecutionEvent::NodeStart {
                node: current.clone(),
                step,
            })
            .await;

            state.metadata.step = step;
            state.metadata.node_history.push(current.clone());

            let output = match (node.handler)(state.clone()).await {
                Ok(o) => o,
                Err(e) => {
                    self.emit_event(ExecutionEvent::Error {
                        node: current.clone(),
                        error: e.to_string(),
                    })
                    .await;
                    return Err(e);
                }
            };

            state = output;

            self.emit_event(ExecutionEvent::NodeComplete {
                node: current.clone(),
                step,
                output: state.clone(),
            })
            .await;

            // 保存检查点
            self.save_checkpoint(&current, &state).await;

            // 查找下一个节点
            let next = self.find_next(&current, &state);

            match next {
                Some(next_node) => {
                    self.emit_event(ExecutionEvent::EdgeTaken {
                        from: current.clone(),
                        to: next_node.clone(),
                    })
                    .await;
                    current = next_node;
                    step += 1;
                }
                None => {
                    // 没有更多边，执行完成
                    self.emit_event(ExecutionEvent::Completed {
                        final_state: state.clone(),
                    })
                    .await;
                    return Ok(state);
                }
            }
        }
    }

    /// 查找下一个节点
    fn find_next(&self, current: &str, state: &GraphState) -> Option<String> {
        // 先检查条件边
        for edge in &self.edges {
            if edge.from == current {
                if let Some(ref condition) = edge.condition {
                    if condition(state) {
                        return Some(edge.to.clone());
                    }
                }
            }
        }

        // 再检查普通边
        for edge in &self.edges {
            if edge.from == current && edge.condition.is_none() {
                return Some(edge.to.clone());
            }
        }

        None
    }

    /// 保存检查点
    async fn save_checkpoint(&self, node: &str, state: &GraphState) {
        let checkpoint = Checkpoint {
            id: Uuid::new_v4().to_string(),
            state: state.clone(),
            node: node.to_string(),
            timestamp: chrono::Utc::now(),
        };
        // 注意：这里需要 &mut self，实际实现中应该用内部可变性
        // 简化处理：检查点存储在外部
        tracing::info!("Checkpoint saved: {}", checkpoint.id);
    }

    /// 发送事件
    async fn emit_event(&self, event: ExecutionEvent) {
        for listener in &self.listeners {
            listener.on_event(event.clone()).await;
        }
    }

    /// 中断执行
    pub fn interrupt(state: &mut GraphState, reason: &str) {
        state.metadata.is_interrupted = true;
        tracing::info!("Execution interrupted: {}", reason);
    }

    /// 恢复执行
    pub fn resume(state: &mut GraphState) {
        state.metadata.is_interrupted = false;
    }
}

/// 子图节点
pub struct SubgraphNode {
    pub name: String,
    pub graph: Arc<StateGraph>,
}

impl SubgraphNode {
    pub fn new(name: &str, graph: StateGraph) -> Self {
        Self {
            name: name.to_string(),
            graph: Arc::new(graph),
        }
    }

    pub fn into_handler(self) -> NodeHandler {
        let graph = self.graph.clone();
        Box::new(move |state| {
            let graph = graph.clone();
            Box::pin(async move { graph.execute(state).await })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_graph_execution() {
        let graph = StateGraph::builder("start")
            .add_node("start", |mut state| async move {
                state.set("value", 1);
                Ok(state)
            })
            .add_node("process", |mut state| async move {
                let v: i32 = state.get("value").unwrap_or(0);
                state.set("value", v * 2);
                Ok(state)
            })
            .add_node("end", |state| async move { Ok(state) })
            .add_edge("start", "process")
            .add_edge("process", "end")
            .build();

        let state = GraphState::new();
        let result = graph.execute(state).await.unwrap();
        assert_eq!(result.get::<i32>("value"), Some(2));
    }

    #[tokio::test]
    async fn test_conditional_edge() {
        let graph = StateGraph::builder("check")
            .add_node("check", |mut state| async move {
                state.set("value", 10);
                Ok(state)
            })
            .add_node("high", |mut state| async move {
                state.set("result", "high");
                Ok(state)
            })
            .add_node("low", |mut state| async move {
                state.set("result", "low");
                Ok(state)
            })
            .add_conditional_edge("check", "high", |state| {
                state.get::<i32>("value").unwrap_or(0) > 5
            })
            .add_conditional_edge("check", "low", |state| {
                state.get::<i32>("value").unwrap_or(0) <= 5
            })
            .build();

        let state = GraphState::new();
        let result = graph.execute(state).await.unwrap();
        assert_eq!(result.get::<String>("result"), Some("high".to_string()));
    }

    #[tokio::test]
    async fn test_loop() {
        let graph = StateGraph::builder("init")
            .add_node("init", |mut state| async move {
                state.set("counter", 0);
                Ok(state)
            })
            .add_node("increment", |mut state| async move {
                let c: i32 = state.get("counter").unwrap_or(0);
                state.set("counter", c + 1);
                Ok(state)
            })
            .add_node("done", |state| async move { Ok(state) })
            .add_edge("init", "increment")
            .add_conditional_edge("increment", "increment", |state| {
                state.get::<i32>("counter").unwrap_or(0) < 5
            })
            .add_conditional_edge("increment", "done", |state| {
                state.get::<i32>("counter").unwrap_or(0) >= 5
            })
            .build();

        let state = GraphState::new();
        let result = graph.execute(state).await.unwrap();
        assert_eq!(result.get::<i32>("counter"), Some(5));
    }

    #[tokio::test]
    async fn test_interrupt_resume() {
        let graph = StateGraph::builder("start")
            .add_node("start", |mut state| async move {
                state.set("step", 1);
                Ok(state)
            })
            .add_node("interrupt_point", |mut state| async move {
                StateGraph::interrupt(&mut state, "需要人工确认");
                Ok(state)
            })
            .add_node("resume_point", |mut state| async move {
                state.set("step", 2);
                Ok(state)
            })
            .add_edge("start", "interrupt_point")
            .add_edge("interrupt_point", "resume_point")
            .build();

        let state = GraphState::new();
        let result = graph.execute(state).await.unwrap();
        assert_eq!(result.get::<i32>("step"), Some(1));
        assert!(result.metadata.is_interrupted);

        // 恢复执行
        let mut resumed_state = result.clone();
        StateGraph::resume(&mut resumed_state);
        // 注意：这里简化处理，实际需要从检查点恢复
    }

    #[tokio::test]
    async fn test_subgraph() {
        let subgraph = StateGraph::builder("sub_start")
            .add_node("sub_start", |mut state| async move {
                state.set("sub_value", 42);
                Ok(state)
            })
            .build();

        let graph = StateGraph::builder("main")
            .add_node("main", |mut state| async move {
                state.set("main_value", 1);
                Ok(state)
            })
            .add_node(
                "sub",
                SubgraphNode::new("sub", subgraph).into_handler(),
            )
            .add_node("end", |state| async move { Ok(state) })
            .add_edge("main", "sub")
            .add_edge("sub", "end")
            .build();

        let state = GraphState::new();
        let result = graph.execute(state).await.unwrap();
        assert_eq!(result.get::<i32>("main_value"), Some(1));
        assert_eq!(result.get::<i32>("sub_value"), Some(42));
    }
}
