use kias_knowledge::{KnowledgeGraph, HybridRetriever, Retriever};
use kias_knowledge::graph::{KnowledgeNode, NodeType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Starting KIAS Knowledge Service");
    
    let mut graph = KnowledgeGraph::new();
    
    // 添加测试节点
    graph.add_node(KnowledgeNode {
        id: "node-1".to_string(),
        content: "KIAS is a Kubernetes-inspired Agent System".to_string(),
        node_type: NodeType::Document,
        metadata: Default::default(),
    });
    
    let retriever = HybridRetriever::new(graph);
    let results = retriever.retrieve("KIAS", 10).await?;
    
    println!("Retrieved {} knowledge nodes", results.len());
    
    tracing::info!("KIAS Knowledge Service finished");
    Ok(())
}
