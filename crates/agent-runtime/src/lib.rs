//! Agent Runtime — Codex 风格的 Agent 执行引擎
//!
//! 核心循环:
//! 1. User → System Prompt + User Message
//! 2. LLM → Tool Calls
//! 3. Execute Tools → Observations
//! 4. Loop until done

pub mod executor;
pub mod context;
pub mod types;

pub use executor::AgentExecutor;
pub use context::AgentContext;
pub use types::*;
