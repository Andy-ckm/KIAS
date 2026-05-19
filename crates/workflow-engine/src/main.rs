use kias_workflow_engine::{
    Edge, ExecutorConfig, Node, NodeType, WorkflowEngine, WorkflowGraph, WorkflowState,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("AgentGuard Workflow Engine starting...");

    // Create a workflow graph with real node executors
    let mut graph = WorkflowGraph::new("example-workflow");

    // Shell executor node
    graph.add_node(
        Node::new("start", "Start", NodeType::Process).with_executor(ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["Workflow started".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: Some(10),
        }),
    );

    // LLM executor node
    graph.add_node(
        Node::new("analyze", "Analyze", NodeType::Process).with_executor(ExecutorConfig::Llm {
            model: "gpt-4".into(),
            prompt: "Analyze the workflow input and provide recommendations".into(),
            temperature: Some(0.7),
            max_tokens: Some(500),
        }),
    );

    // Condition node
    graph.add_node(
        Node::new("decide", "Decide", NodeType::Condition)
            .with_config(serde_json::json!({"condition_key": "recommendation"})),
    );

    // Branch nodes
    graph.add_node(
        Node::new("approve", "Approve", NodeType::Process).with_executor(ExecutorConfig::Shell {
            command: "echo".into(),
            args: vec!["Approved!".into()],
            env: HashMap::new(),
            working_dir: None,
            timeout_secs: None,
        }),
    );

    // Human review node
    graph.add_node(Node::new("review", "HumanReview", NodeType::HumanReview));

    // End node
    graph.add_node(Node::new("end", "End", NodeType::Process));

    // Wire up edges
    graph.add_edge(Edge::new("start", "analyze"));
    graph.add_edge(Edge::new("analyze", "decide"));
    graph.add_edge(
        Edge::new("decide", "approve")
            .with_condition("recommendation == \"approve\"", "Auto-approve"),
    );
    graph.add_edge(
        Edge::new("decide", "review")
            .with_condition("recommendation == \"review\"", "Needs review"),
    );
    graph.add_edge(Edge::new("approve", "end"));
    graph.add_edge(Edge::new("review", "end"));

    graph.set_entry("start");
    graph.add_exit_node("end");

    // Validate
    graph.validate().map_err(|e| anyhow::anyhow!(e))?;

    // Create initial state
    let mut initial_state = WorkflowState::new(&graph.id, &graph.entry_node);
    initial_state.set("input", "Some workflow input data");
    initial_state.set("recommendation", "approve");

    // Execute
    let engine = WorkflowEngine::new();
    let final_state = engine.execute(&graph, initial_state).await?;

    println!("Workflow completed with status: {:?}", final_state.status);
    println!(
        "Final state data keys: {:?}",
        final_state.data.keys().collect::<Vec<_>>()
    );

    Ok(())
}
