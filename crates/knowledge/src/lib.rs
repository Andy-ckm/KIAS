pub mod agentic_rag;
pub mod approval;
pub mod context_manager;
pub mod entity_extractor;
pub mod graph;
pub mod graphrag;
pub mod inspiration_stream;
pub mod memory;
pub mod memory_layers;
pub mod quality_pipeline;
pub mod retriever;
pub mod vector;

pub use context_manager::{
    CompressionLevel, CompressionResult, ContextManager, ContextManagerConfig, ContextMessage,
    ContextStats, MessageRole, MultiSessionContextManager,
};

pub use graph::{Edge, KnowledgeGraph, KnowledgeNode, NodeType};
pub use graphrag::{GraphRAGEngine, HybridQuery, RetrievalResult, RetrievalStrategy};
pub use memory::{AgentMemoryStore, Importance, MemoryEntry, MemoryType};
pub use retriever::{HybridRetriever, KeywordRetriever, MatchType, Retriever, ScoredNode};
pub use vector::{
    cosine_distance, cosine_similarity, l2_distance, EmbeddingEngine, LocalEmbeddingEngine,
    SiliconFlowEmbeddingEngine, VectorRetriever, VectorStore, VectorStoreStats, BGE_M3_DIMENSION,
};
