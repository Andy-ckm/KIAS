//! # KIAS LangGraph-style State Graph Engine
//!
//! A directed graph state machine inspired by [LangGraph](https://langchain-ai.github.io/langgraph/),
//! supporting conditional branching, loops, interrupt/resume, subgraph composition,
//! persistent checkpoints, streaming events, and parallel fan-out execution.
//!
//! ## Architecture
//!
//! ```text
//! StateGraphBuilder ──build()──▶ StateGraph ──execute()──▶ GraphState
//!       │                           │
//!       ├── add_node()              ├── CheckpointStore (persistence)
//!       ├── add_edge()              └── ExecutionStream (events)
//!       ├── add_conditional_edge()
//!       ├── add_router()
//!       ├── add_fan_out()
//!       ├── with_checkpoint_store()
//!       └── with_stream()
//! ```

pub mod checkpoint;
pub mod graph;
pub mod state;
pub mod stream;
pub mod validation;

// Re-exports for convenience
pub use checkpoint::{Checkpoint, CheckpointStore, InMemoryCheckpointStore};
pub use graph::{EdgeCondition, GraphNode, NodeHandler, RouterFn, StateGraph, StateGraphBuilder};
pub use state::{GraphState, GraphStateSnapshot, StateMetadata};
pub use stream::{EventCollector, ExecutionEvent, ExecutionStream};
pub use validation::{ValidationError, ValidationErrorKind};
