pub mod agent_tool;
pub mod compaction;
pub mod crew;
pub mod delegation;
pub mod embedder;
pub mod engine;
pub mod inspiration;
pub mod memory;
pub mod owner;
pub mod semantic_matcher;
pub mod session;
pub mod skill_matcher;
pub mod soul;
pub mod state;
pub mod subagent;
pub mod swarm;
pub mod team;
pub mod verifier;
pub mod worker;
pub mod workspace;

pub use agent_tool::{
    AgentExecutor, AgentTool, AgentToolError, EchoAgentExecutor, FailingAgentExecutor,
    OrchestrationMode, TeamTool, ToolDescriptor, ToolRegistry, ToolResult,
};
pub use compaction::{
    extract_key_facts, CompactionConfig, CompactionResult, ContextCompactor, Message,
};
pub use crew::{Crew, CrewConfig, CrewResult, CrewStats, CrewTask, ProcessMode, TaskExecutor};
pub use delegation::{
    AgentId, CancelDelegation, DelegateRequest, DelegateResponse, DelegationId, DelegationMessage,
    DelegationPriority, DelegationRecord, DelegationResult, DelegationState, ProgressUpdate,
};
pub use embedder::{Embedder, HashingEmbedder};
pub use engine::TeamEngine;
pub use memory::{
    compaction::{
        estimate_messages_tokens, estimate_tokens, should_compact, CompactedHistory,
        CompactionManager, CompactionStrategy, HistorySnapshot, SlidingWindow, SummarizeOld,
    },
    ContextBuilder, EntityFact, EntityMemory, LongTermMemory, MemoryCategory, MemoryEntry,
    MemoryManager, MidTermEntry, MidTermMemory, ShortTermMemory,
};
pub use owner::Owner;
pub use semantic_matcher::{SemanticMatchResult, SemanticMatcherConfig, SemanticSkillMatcher};
pub use session::{Session, SessionConfig, SessionMessage, SessionMetadata};
pub use skill_matcher::{AgentProfile, MatchResult, MatcherConfig, SkillMatcher};
pub use soul::{Personality, SoulConfig, SoulLoader};
pub use state::{AgentRole, TaskStatus, TeamState};
pub use subagent::{
    DelegationMode, DelegationOutcome, SubAgentError, SubAgentExecutor, SubAgentRegistry,
    SubAgentRunner, SubAgentSpec, TaskHandle, TaskStatus as SubAgentTaskStatus,
};
pub use swarm::{SwarmOrchestrator, SwarmStrategy};
pub use team::Team;
pub use verifier::{QualityGate, RuleBasedVerifier, VerificationRule, Verifier};
pub use worker::{CodeWorker, LlmWorker, ResearchWorker, Worker};
pub use workspace::{SkillDef, Workspace, WorkspaceConfig};

// pub // mod task_planner; // TODO: fix compilation // TODO: fix compilation
pub mod adversarial_validation;
// pub // mod memory_governance; // TODO: fix compilation // TODO: fix compilation
// pub // mod self_eval_loop; // TODO: fix compilation // TODO: fix compilation
// pub // mod memory_enhanced; // TODO: fix compilation // TODO: fix compilation
// pub // mod memory_conflict; // TODO: fix compilation // TODO: fix compilation
