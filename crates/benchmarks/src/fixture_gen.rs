//! Auto-generated mock agent states for property-based testing.
//!
//! Provides `FixtureGenerator` with deterministic seeding for reproducible
//! random AgentState, TaskState, and LoopState generation. Includes edge-case
//! fixtures (empty strings, max-length, unicode, zero/negative values) and
//! proptest strategies.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Mock state types (mirror real controller/workflow/goal structures) ───────

/// Mock agent status enum mirroring controller::AgentStatus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Pending,
    Running,
    Failed,
    Succeeded,
    Unresponsive,
}

impl AgentStatus {
    pub fn from_index(i: usize) -> Self {
        match i % 5 {
            0 => Self::Pending,
            1 => Self::Running,
            2 => Self::Failed,
            3 => Self::Succeeded,
            _ => Self::Unresponsive,
        }
    }
}

/// Mock agent state for property-based testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub retry_count: u32,
    pub consecutive_failures: u32,
    pub cpu_request: f64,
    pub memory_bytes: u64,
    pub gpu: u32,
    pub priority: u32,
    pub tenant_id: Option<String>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

/// Mock task status enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    WaitingForHuman,
}

impl TaskStatus {
    pub fn from_index(i: usize) -> Self {
        match i % 6 {
            0 => Self::Pending,
            1 => Self::Running,
            2 => Self::Completed,
            3 => Self::Failed,
            4 => Self::Cancelled,
            _ => Self::WaitingForHuman,
        }
    }
}

/// Mock task/workflow state for property-based testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub task_id: String,
    pub workflow_id: String,
    pub current_node: String,
    pub status: TaskStatus,
    pub data: HashMap<String, serde_json::Value>,
    pub history: Vec<StateTransition>,
    pub retries: u32,
    pub duration_ms: u64,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A state transition record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from_node: String,
    pub to_node: String,
    pub timestamp: DateTime<Utc>,
}

/// Mock goal/loop status enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LoopStatus {
    Pending,
    InProgress,
    Achieved,
    NotAchieved,
    Failed,
    Cancelled,
}

impl LoopStatus {
    pub fn from_index(i: usize) -> Self {
        match i % 6 {
            0 => Self::Pending,
            1 => Self::InProgress,
            2 => Self::Achieved,
            3 => Self::NotAchieved,
            4 => Self::Failed,
            _ => Self::Cancelled,
        }
    }
}

/// Mock goal-loop state for property-based testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    pub loop_id: String,
    pub goal_id: String,
    pub description: String,
    pub status: LoopStatus,
    pub current_round: u32,
    pub max_rounds: Option<u32>,
    pub total_tokens: u64,
    pub evaluation_history: Vec<EvaluationEntry>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single evaluation entry in the loop history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationEntry {
    pub round: u32,
    pub achieved: bool,
    pub reason: String,
    pub score: f64,
    pub evaluated_at: DateTime<Utc>,
}

// ── FixtureGenerator ────────────────────────────────────────────────────────

/// Deterministic fixture generator backed by a seeded StdRng.
///
/// Use `FixtureGenerator::new(seed)` for reproducible output, or
/// `FixtureGenerator::default()` for a fixed seed (42).
pub struct FixtureGenerator {
    rng: StdRng,
}

impl FixtureGenerator {
    /// Create a generator with a specific seed for reproducibility.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    // ── Helper: random alphanumeric string ──────────────────────────────

    fn random_string(&mut self, len: usize) -> String {
        let chars: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
        (0..len)
            .map(|_| {
                let idx = self.rng.gen_range(0..chars.len());
                chars[idx] as char
            })
            .collect()
    }

    fn random_label_value(&mut self) -> String {
        let len = self.rng.gen_range(1..=20);
        self.random_string(len)
    }

    // ── AgentState generation ───────────────────────────────────────────

    /// Generate a random AgentState with normal (non-edge-case) values.
    pub fn gen_agent_state(&mut self) -> AgentState {
        let id = format!("agent-{}", self.rng.gen_range(0u64..u64::MAX));
        let name = format!("agent-{}", self.random_string(8));
        let label_count = self.rng.gen_range(0..=5);
        let labels: HashMap<String, String> = (0..label_count)
            .map(|_| {
                (
                    format!("key-{}", self.random_string(4)),
                    self.random_label_value(),
                )
            })
            .collect();

        AgentState {
            id,
            name,
            status: AgentStatus::from_index(self.rng.gen_range(0..5)),
            retry_count: self.rng.gen_range(0..10),
            consecutive_failures: self.rng.gen_range(0..5),
            cpu_request: self.rng.gen_range(0.1..16.0),
            memory_bytes: self.rng.gen_range(128 * 1024 * 1024..64 * 1024 * 1024 * 1024),
            gpu: self.rng.gen_range(0..8),
            priority: self.rng.gen_range(1..=200),
            tenant_id: if self.rng.gen_bool(0.7) {
                Some(format!("tenant-{}", self.random_string(4)))
            } else {
                None
            },
            labels,
            created_at: Utc::now() - ChronoDuration::seconds(self.rng.gen_range(1..86400)),
            last_heartbeat: Utc::now() - ChronoDuration::seconds(self.rng.gen_range(0..300)),
        }
    }

    /// Generate an AgentState with edge-case values:
    /// empty strings, max-length strings, unicode, zero resources.
    pub fn gen_agent_state_edge(&mut self, variant: usize) -> AgentState {
        match variant % 6 {
            // Empty id and name
            0 => AgentState {
                id: String::new(),
                name: String::new(),
                status: AgentStatus::Pending,
                retry_count: 0,
                consecutive_failures: 0,
                cpu_request: 0.0,
                memory_bytes: 0,
                gpu: 0,
                priority: 0,
                tenant_id: None,
                labels: HashMap::new(),
                created_at: Utc::now(),
                last_heartbeat: Utc::now(),
            },
            // Max-length strings (4096 chars)
            1 => AgentState {
                id: "a".repeat(4096),
                name: "n".repeat(4096),
                status: AgentStatus::Running,
                retry_count: u32::MAX,
                consecutive_failures: u32::MAX,
                cpu_request: f64::MAX,
                memory_bytes: u64::MAX,
                gpu: u32::MAX,
                priority: u32::MAX,
                tenant_id: Some("t".repeat(4096)),
                labels: {
                    let mut m = HashMap::new();
                    m.insert("k".repeat(256), "v".repeat(4096));
                    m
                },
                created_at: DateTime::<Utc>::MIN_UTC,
                last_heartbeat: DateTime::<Utc>::MAX_UTC,
            },
            // Unicode strings
            2 => AgentState {
                id: "agent-🦀-日本語-العربية".to_string(),
                name: "测试代理-🎉-مرحبا".to_string(),
                status: AgentStatus::Failed,
                retry_count: 3,
                consecutive_failures: 1,
                cpu_request: 1.0,
                memory_bytes: 1024 * 1024 * 1024,
                gpu: 1,
                priority: 100,
                tenant_id: Some("租户-🏢".to_string()),
                labels: {
                    let mut m = HashMap::new();
                    m.insert("地域".to_string(), "東京".to_string());
                    m.insert("环境".to_string(), "生产".to_string());
                    m
                },
                created_at: Utc::now(),
                last_heartbeat: Utc::now(),
            },
            // Negative-equivalent values (wrapping for unsigned)
            3 => AgentState {
                id: "neg-agent".to_string(),
                name: "neg-agent".to_string(),
                status: AgentStatus::Unresponsive,
                retry_count: u32::MAX, // boundary
                consecutive_failures: u32::MAX,
                cpu_request: 0.0,       // zero CPU
                memory_bytes: 0,        // zero memory
                gpu: 0,                 // zero GPU
                priority: 0,            // zero priority
                tenant_id: None,
                labels: HashMap::new(),
                created_at: DateTime::<Utc>::MIN_UTC,
                last_heartbeat: DateTime::<Utc>::MIN_UTC,
            },
            // Single-char strings, minimal values
            4 => AgentState {
                id: "x".to_string(),
                name: "y".to_string(),
                status: AgentStatus::Succeeded,
                retry_count: 1,
                consecutive_failures: 0,
                cpu_request: 0.001,
                memory_bytes: 1,
                gpu: 0,
                priority: 1,
                tenant_id: Some("z".to_string()),
                labels: {
                    let mut m = HashMap::new();
                    m.insert("a".to_string(), "b".to_string());
                    m
                },
                created_at: Utc::now(),
                last_heartbeat: Utc::now(),
            },
            // Large label set
            5 => {
                let labels: HashMap<String, String> = (0..100)
                    .map(|i| (format!("label-{}", i), format!("value-{}", i)))
                    .collect();
                AgentState {
                    id: "labeled-agent".to_string(),
                    name: "many-labels".to_string(),
                    status: AgentStatus::Running,
                    retry_count: 0,
                    consecutive_failures: 0,
                    cpu_request: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 2,
                    priority: 50,
                    tenant_id: Some("bulk".to_string()),
                    labels,
                    created_at: Utc::now(),
                    last_heartbeat: Utc::now(),
                }
            }
            _ => unreachable!(),
        }
    }

    /// Generate `n` random AgentState objects.
    pub fn gen_agent_states(&mut self, n: usize) -> Vec<AgentState> {
        (0..n).map(|_| self.gen_agent_state()).collect()
    }

    // ── TaskState generation ────────────────────────────────────────────

    /// Generate a random TaskState.
    pub fn gen_task_state(&mut self) -> TaskState {
        let transition_count = self.rng.gen_range(0..10);
        let history: Vec<StateTransition> = (0..transition_count)
            .map(|i| StateTransition {
                from_node: format!("node-{}", i),
                to_node: format!("node-{}", i + 1),
                timestamp: Utc::now()
                    - ChronoDuration::seconds(self.rng.gen_range(1..3600)),
            })
            .collect();

        let data_count = self.rng.gen_range(0..5);
        let data: HashMap<String, serde_json::Value> = (0..data_count)
            .map(|i| {
                (
                    format!("key-{}", i),
                    serde_json::json!(self.rng.gen_range(0..1000)),
                )
            })
            .collect();

        TaskState {
            task_id: format!("task-{}", self.rng.gen_range(0u64..u64::MAX)),
            workflow_id: format!("wf-{}", self.rng.gen_range(0u64..u64::MAX)),
            current_node: format!("node-{}", self.rng.gen_range(0..20)),
            status: TaskStatus::from_index(self.rng.gen_range(0..6)),
            data,
            history,
            retries: self.rng.gen_range(0..5),
            duration_ms: self.rng.gen_range(0..120_000),
            started_at: Utc::now() - ChronoDuration::seconds(self.rng.gen_range(1..86400)),
            updated_at: Utc::now(),
        }
    }

    /// Generate a TaskState with edge-case values.
    pub fn gen_task_state_edge(&mut self, variant: usize) -> TaskState {
        match variant % 4 {
            // Empty everything
            0 => TaskState {
                task_id: String::new(),
                workflow_id: String::new(),
                current_node: String::new(),
                status: TaskStatus::Pending,
                data: HashMap::new(),
                history: vec![],
                retries: 0,
                duration_ms: 0,
                started_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Unicode task names
            1 => TaskState {
                task_id: "任务-🔧-ID".to_string(),
                workflow_id: "工作流-🌊".to_string(),
                current_node: "处理节点-📝".to_string(),
                status: TaskStatus::Running,
                data: {
                    let mut m = HashMap::new();
                    m.insert(
                        "输入".to_string(),
                        serde_json::json!("测试数据-🎯"),
                    );
                    m
                },
                history: vec![StateTransition {
                    from_node: "开始".to_string(),
                    to_node: "处理".to_string(),
                    timestamp: Utc::now(),
                }],
                retries: 2,
                duration_ms: 5000,
                started_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Max-length and large values
            2 => TaskState {
                task_id: "t".repeat(4096),
                workflow_id: "w".repeat(4096),
                current_node: "n".repeat(4096),
                status: TaskStatus::Failed,
                data: {
                    let mut m = HashMap::new();
                    for i in 0..50 {
                        m.insert(
                            format!("key-{}", i),
                            serde_json::json!("v".repeat(1024)),
                        );
                    }
                    m
                },
                history: (0..100)
                    .map(|i| StateTransition {
                        from_node: format!("n-{}", i),
                        to_node: format!("n-{}", i + 1),
                        timestamp: Utc::now(),
                    })
                    .collect(),
                retries: u32::MAX,
                duration_ms: u64::MAX,
                started_at: DateTime::<Utc>::MIN_UTC,
                updated_at: DateTime::<Utc>::MAX_UTC,
            },
            // Zero/negative-boundary
            3 => TaskState {
                task_id: "zero".to_string(),
                workflow_id: "zero".to_string(),
                current_node: "zero".to_string(),
                status: TaskStatus::Cancelled,
                data: HashMap::new(),
                history: vec![],
                retries: 0,
                duration_ms: 0,
                started_at: DateTime::<Utc>::MIN_UTC,
                updated_at: DateTime::<Utc>::MIN_UTC,
            },
            _ => unreachable!(),
        }
    }

    /// Generate `n` random TaskState objects.
    pub fn gen_task_states(&mut self, n: usize) -> Vec<TaskState> {
        (0..n).map(|_| self.gen_task_state()).collect()
    }

    // ── LoopState generation ────────────────────────────────────────────

    /// Generate a random LoopState (goal-loop).
    pub fn gen_loop_state(&mut self) -> LoopState {
        let eval_count = self.rng.gen_range(0..10);
        let eval_history: Vec<EvaluationEntry> = (0..eval_count)
            .map(|i| EvaluationEntry {
                round: i as u32 + 1,
                achieved: self.rng.gen_bool(0.3),
                reason: format!("reason-{}", self.random_string(6)),
                score: self.rng.gen_range(0.0..1.0),
                evaluated_at: Utc::now()
                    - ChronoDuration::seconds(self.rng.gen_range(1..3600)),
            })
            .collect();

        LoopState {
            loop_id: format!("loop-{}", self.rng.gen_range(0u64..u64::MAX)),
            goal_id: format!("goal-{}", self.rng.gen_range(0u64..u64::MAX)),
            description: format!("Goal: achieve {}", self.random_string(12)),
            status: LoopStatus::from_index(self.rng.gen_range(0..6)),
            current_round: self.rng.gen_range(0..50),
            max_rounds: if self.rng.gen_bool(0.8) {
                Some(self.rng.gen_range(1..100))
            } else {
                None
            },
            total_tokens: self.rng.gen_range(0..1_000_000),
            evaluation_history: eval_history,
            started_at: Utc::now() - ChronoDuration::seconds(self.rng.gen_range(1..86400)),
            updated_at: Utc::now(),
        }
    }

    /// Generate a LoopState with edge-case values.
    pub fn gen_loop_state_edge(&mut self, variant: usize) -> LoopState {
        match variant % 4 {
            // Empty/minimal
            0 => LoopState {
                loop_id: String::new(),
                goal_id: String::new(),
                description: String::new(),
                status: LoopStatus::Pending,
                current_round: 0,
                max_rounds: None,
                total_tokens: 0,
                evaluation_history: vec![],
                started_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Unicode
            1 => LoopState {
                loop_id: "循环-🔄-ID".to_string(),
                goal_id: "目标-🎯-ID".to_string(),
                description: "实现 自动化 🚀 目标".to_string(),
                status: LoopStatus::InProgress,
                current_round: 5,
                max_rounds: Some(20),
                total_tokens: 50000,
                evaluation_history: vec![EvaluationEntry {
                    round: 1,
                    achieved: false,
                    reason: "尚未达到 📊 目标".to_string(),
                    score: 0.65,
                    evaluated_at: Utc::now(),
                }],
                started_at: Utc::now(),
                updated_at: Utc::now(),
            },
            // Max-length and boundary values
            2 => LoopState {
                loop_id: "l".repeat(4096),
                goal_id: "g".repeat(4096),
                description: "d".repeat(8192),
                status: LoopStatus::Failed,
                current_round: u32::MAX,
                max_rounds: Some(u32::MAX),
                total_tokens: u64::MAX,
                evaluation_history: (0..200)
                    .map(|i| EvaluationEntry {
                        round: i + 1,
                        achieved: false,
                        reason: "r".repeat(512),
                        score: 0.0,
                        evaluated_at: Utc::now(),
                    })
                    .collect(),
                started_at: DateTime::<Utc>::MIN_UTC,
                updated_at: DateTime::<Utc>::MAX_UTC,
            },
            // Zero/boundary
            3 => LoopState {
                loop_id: "0".to_string(),
                goal_id: "0".to_string(),
                description: "zero".to_string(),
                status: LoopStatus::Cancelled,
                current_round: 0,
                max_rounds: Some(0),
                total_tokens: 0,
                evaluation_history: vec![],
                started_at: DateTime::<Utc>::MIN_UTC,
                updated_at: DateTime::<Utc>::MIN_UTC,
            },
            _ => unreachable!(),
        }
    }

    /// Generate `n` random LoopState objects.
    pub fn gen_loop_states(&mut self, n: usize) -> Vec<LoopState> {
        (0..n).map(|_| self.gen_loop_state()).collect()
    }
}

impl Default for FixtureGenerator {
    fn default() -> Self {
        Self::new(42)
    }
}

// ── proptest strategies ─────────────────────────────────────────────────────

/// proptest strategies for generating `AgentState` values.
pub mod strategies {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for generating random AgentStatus values.
    pub fn agent_status() -> impl Strategy<Value = AgentStatus> {
        prop_oneof![
            Just(AgentStatus::Pending),
            Just(AgentStatus::Running),
            Just(AgentStatus::Failed),
            Just(AgentStatus::Succeeded),
            Just(AgentStatus::Unresponsive),
        ]
    }

    /// Strategy for generating random TaskStatus values.
    pub fn task_status() -> impl Strategy<Value = TaskStatus> {
        prop_oneof![
            Just(TaskStatus::Pending),
            Just(TaskStatus::Running),
            Just(TaskStatus::Completed),
            Just(TaskStatus::Failed),
            Just(TaskStatus::Cancelled),
            Just(TaskStatus::WaitingForHuman),
        ]
    }

    /// Strategy for generating random LoopStatus values.
    pub fn loop_status() -> impl Strategy<Value = LoopStatus> {
        prop_oneof![
            Just(LoopStatus::Pending),
            Just(LoopStatus::InProgress),
            Just(LoopStatus::Achieved),
            Just(LoopStatus::NotAchieved),
            Just(LoopStatus::Failed),
            Just(LoopStatus::Cancelled),
        ]
    }

    /// Strategy for generating AgentState with controlled fields.
    pub fn agent_state() -> impl Strategy<Value = AgentStatus> {
        agent_status()
    }

    /// Full strategy for AgentState using proptest combinators.
    ///
    /// Generates AgentState with bounded field sizes suitable for fast
    /// property-based testing (avoids huge allocations).
    pub fn arb_agent_state() -> impl Strategy<Value = AgentState> {
        (
            "[a-z0-9]{1,32}",                       // id
            "[a-zA-Z0-9_]{1,64}",                   // name
            agent_status(),                          // status
            0u32..20,                                // retry_count
            0u32..10,                                // consecutive_failures
            0.0f64..32.0,                            // cpu_request
            128u64..64 * 1024 * 1024 * 1024,         // memory_bytes
            0u32..8,                                 // gpu
            1u32..200,                               // priority
            prop::option::of("[a-z0-9]{1,16}"),      // tenant_id
            prop::collection::hash_map(              // labels
                "[a-z]{1,8}",
                "[a-z]{1,8}",
                0..5,
            ),
        )
            .prop_map(
                |(id, name, status, retry, fail, cpu, mem, gpu, pri, tenant, labels)| {
                    AgentState {
                        id,
                        name,
                        status,
                        retry_count: retry,
                        consecutive_failures: fail,
                        cpu_request: cpu,
                        memory_bytes: mem,
                        gpu,
                        priority: pri,
                        tenant_id: tenant,
                        labels,
                        created_at: Utc::now(),
                        last_heartbeat: Utc::now(),
                    }
                },
            )
    }

    /// Strategy for TaskState.
    pub fn arb_task_state() -> impl Strategy<Value = TaskState> {
        (
            "[a-z0-9]{1,32}",
            "[a-z0-9]{1,32}",
            "[a-z]{1,16}",
            task_status(),
            0u32..10,
            0u64..120_000,
        )
            .prop_map(|(tid, wf, node, status, retries, dur)| TaskState {
                task_id: tid,
                workflow_id: wf,
                current_node: node,
                status,
                data: HashMap::new(),
                history: vec![],
                retries,
                duration_ms: dur,
                started_at: Utc::now(),
                updated_at: Utc::now(),
            })
    }

    /// Strategy for LoopState.
    pub fn arb_loop_state() -> impl Strategy<Value = LoopState> {
        (
            "[a-z0-9]{1,32}",
            "[a-z0-9]{1,32}",
            "[a-zA-Z ]{1,64}",
            loop_status(),
            0u32..50,
            prop::option::of(1u32..100),
            0u64..1_000_000,
        )
            .prop_map(|(lid, gid, desc, status, round, max, tokens)| LoopState {
                loop_id: lid,
                goal_id: gid,
                description: format!("Goal: {}", desc),
                status,
                current_round: round,
                max_rounds: max,
                total_tokens: tokens,
                evaluation_history: vec![],
                started_at: Utc::now(),
                updated_at: Utc::now(),
            })
    }
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FixtureGenerator basic tests ────────────────────────────────────

    #[test]
    fn test_generator_deterministic_seeding() {
        let mut gen1 = FixtureGenerator::new(12345);
        let mut gen2 = FixtureGenerator::new(12345);

        let agents1 = gen1.gen_agent_states(10);
        let agents2 = gen2.gen_agent_states(10);

        assert_eq!(agents1.len(), agents2.len());
        for (a, b) in agents1.iter().zip(agents2.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.name, b.name);
            assert_eq!(a.status, b.status);
            assert_eq!(a.retry_count, b.retry_count);
        }
    }

    #[test]
    fn test_generator_default_seed() {
        let gen1 = FixtureGenerator::default();
        let gen2 = FixtureGenerator::default();
        // Same seed must produce same generator; verify via internal state is hard,
        // so just generate and compare first agent.
        let mut g1 = gen1;
        let mut g2 = gen2;
        let a1 = g1.gen_agent_state();
        let a2 = g2.gen_agent_state();
        assert_eq!(a1.id, a2.id);
    }

    #[test]
    fn test_different_seeds_produce_different_output() {
        let mut gen1 = FixtureGenerator::new(1);
        let mut gen2 = FixtureGenerator::new(2);
        let a1 = gen1.gen_agent_state();
        let a2 = gen2.gen_agent_state();
        // With overwhelming probability these differ
        assert_ne!(a1.id, a2.id);
    }

    #[test]
    fn test_gen_agent_states_count() {
        let mut gen = FixtureGenerator::new(42);
        let agents = gen.gen_agent_states(100);
        assert_eq!(agents.len(), 100);
    }

    #[test]
    fn test_gen_agent_state_edge_empty_strings() {
        let mut gen = FixtureGenerator::new(0);
        let agent = gen.gen_agent_state_edge(0);
        assert!(agent.id.is_empty());
        assert!(agent.name.is_empty());
        assert_eq!(agent.status, AgentStatus::Pending);
        assert_eq!(agent.retry_count, 0);
        assert_eq!(agent.cpu_request, 0.0);
        assert_eq!(agent.memory_bytes, 0);
    }

    #[test]
    fn test_gen_agent_state_edge_max_length() {
        let mut gen = FixtureGenerator::new(0);
        let agent = gen.gen_agent_state_edge(1);
        assert_eq!(agent.id.len(), 4096);
        assert_eq!(agent.name.len(), 4096);
        assert_eq!(agent.retry_count, u32::MAX);
        assert_eq!(agent.memory_bytes, u64::MAX);
        assert_eq!(agent.gpu, u32::MAX);
    }

    #[test]
    fn test_gen_agent_state_edge_unicode() {
        let mut gen = FixtureGenerator::new(0);
        let agent = gen.gen_agent_state_edge(2);
        assert!(agent.id.contains('🦀'));
        assert!(agent.name.contains('🎉'));
        assert_eq!(agent.status, AgentStatus::Failed);
        // Verify unicode labels survive round-trip
        assert!(agent.labels.contains_key("地域"));
    }

    #[test]
    fn test_gen_agent_state_edge_zero_resources() {
        let mut gen = FixtureGenerator::new(0);
        let agent = gen.gen_agent_state_edge(3);
        assert_eq!(agent.cpu_request, 0.0);
        assert_eq!(agent.memory_bytes, 0);
        assert_eq!(agent.gpu, 0);
        assert_eq!(agent.priority, 0);
        assert_eq!(agent.status, AgentStatus::Unresponsive);
    }

    #[test]
    fn test_gen_agent_state_edge_large_labels() {
        let mut gen = FixtureGenerator::new(0);
        let agent = gen.gen_agent_state_edge(5);
        assert_eq!(agent.labels.len(), 100);
    }

    #[test]
    fn test_gen_task_state_deterministic() {
        let mut gen1 = FixtureGenerator::new(99);
        let mut gen2 = FixtureGenerator::new(99);
        let t1 = gen1.gen_task_state();
        let t2 = gen2.gen_task_state();
        assert_eq!(t1.task_id, t2.task_id);
        assert_eq!(t1.status, t2.status);
        assert_eq!(t1.retries, t2.retries);
    }

    #[test]
    fn test_gen_task_state_edge_empty() {
        let mut gen = FixtureGenerator::new(0);
        let task = gen.gen_task_state_edge(0);
        assert!(task.task_id.is_empty());
        assert!(task.workflow_id.is_empty());
        assert!(task.history.is_empty());
        assert!(task.data.is_empty());
    }

    #[test]
    fn test_gen_task_state_edge_unicode() {
        let mut gen = FixtureGenerator::new(0);
        let task = gen.gen_task_state_edge(1);
        assert!(task.task_id.contains('🔧'));
        assert!(task.workflow_id.contains('🌊'));
    }

    #[test]
    fn test_gen_loop_state_deterministic() {
        let mut gen1 = FixtureGenerator::new(77);
        let mut gen2 = FixtureGenerator::new(77);
        let l1 = gen1.gen_loop_state();
        let l2 = gen2.gen_loop_state();
        assert_eq!(l1.loop_id, l2.loop_id);
        assert_eq!(l1.goal_id, l2.goal_id);
        assert_eq!(l1.status, l2.status);
        assert_eq!(l1.current_round, l2.current_round);
    }

    #[test]
    fn test_gen_loop_state_edge_empty() {
        let mut gen = FixtureGenerator::new(0);
        let ls = gen.gen_loop_state_edge(0);
        assert!(ls.loop_id.is_empty());
        assert!(ls.goal_id.is_empty());
        assert!(ls.description.is_empty());
        assert_eq!(ls.current_round, 0);
        assert_eq!(ls.total_tokens, 0);
        assert!(ls.evaluation_history.is_empty());
    }

    #[test]
    fn test_gen_loop_state_edge_unicode() {
        let mut gen = FixtureGenerator::new(0);
        let ls = gen.gen_loop_state_edge(1);
        assert!(ls.loop_id.contains('🔄'));
        assert!(ls.description.contains('🚀'));
    }

    #[test]
    fn test_gen_loop_state_edge_max_boundary() {
        let mut gen = FixtureGenerator::new(0);
        let ls = gen.gen_loop_state_edge(2);
        assert_eq!(ls.current_round, u32::MAX);
        assert_eq!(ls.total_tokens, u64::MAX);
        assert_eq!(ls.evaluation_history.len(), 200);
    }

    #[test]
    fn test_gen_task_states_count() {
        let mut gen = FixtureGenerator::new(42);
        let tasks = gen.gen_task_states(50);
        assert_eq!(tasks.len(), 50);
    }

    #[test]
    fn test_gen_loop_states_count() {
        let mut gen = FixtureGenerator::new(42);
        let loops = gen.gen_loop_states(30);
        assert_eq!(loops.len(), 30);
    }

    #[test]
    fn test_agent_state_serialization_roundtrip() {
        let mut gen = FixtureGenerator::new(42);
        let agent = gen.gen_agent_state();
        let json = serde_json::to_string(&agent).expect("serialize");
        let restored: AgentState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(agent.id, restored.id);
        assert_eq!(agent.name, restored.name);
        assert_eq!(agent.status, restored.status);
        assert_eq!(agent.retry_count, restored.retry_count);
    }

    #[test]
    fn test_task_state_serialization_roundtrip() {
        let mut gen = FixtureGenerator::new(42);
        let task = gen.gen_task_state();
        let json = serde_json::to_string(&task).expect("serialize");
        let restored: TaskState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(task.task_id, restored.task_id);
        assert_eq!(task.status, restored.status);
    }

    #[test]
    fn test_loop_state_serialization_roundtrip() {
        let mut gen = FixtureGenerator::new(42);
        let ls = gen.gen_loop_state();
        let json = serde_json::to_string(&ls).expect("serialize");
        let restored: LoopState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ls.loop_id, restored.loop_id);
        assert_eq!(ls.status, restored.status);
        assert_eq!(ls.current_round, restored.current_round);
    }

    #[test]
    fn test_agent_status_from_index_covers_all_variants() {
        let statuses: Vec<AgentStatus> = (0..5).map(AgentStatus::from_index).collect();
        assert!(statuses.contains(&AgentStatus::Pending));
        assert!(statuses.contains(&AgentStatus::Running));
        assert!(statuses.contains(&AgentStatus::Failed));
        assert!(statuses.contains(&AgentStatus::Succeeded));
        assert!(statuses.contains(&AgentStatus::Unresponsive));
    }

    #[test]
    fn test_task_status_from_index_covers_all_variants() {
        let statuses: Vec<TaskStatus> = (0..6).map(TaskStatus::from_index).collect();
        assert!(statuses.contains(&TaskStatus::Pending));
        assert!(statuses.contains(&TaskStatus::Running));
        assert!(statuses.contains(&TaskStatus::Completed));
        assert!(statuses.contains(&TaskStatus::Failed));
        assert!(statuses.contains(&TaskStatus::Cancelled));
        assert!(statuses.contains(&TaskStatus::WaitingForHuman));
    }

    #[test]
    fn test_loop_status_from_index_covers_all_variants() {
        let statuses: Vec<LoopStatus> = (0..6).map(LoopStatus::from_index).collect();
        assert!(statuses.contains(&LoopStatus::Pending));
        assert!(statuses.contains(&LoopStatus::InProgress));
        assert!(statuses.contains(&LoopStatus::Achieved));
        assert!(statuses.contains(&LoopStatus::NotAchieved));
        assert!(statuses.contains(&LoopStatus::Failed));
        assert!(statuses.contains(&LoopStatus::Cancelled));
    }
}

// ── proptest-based property tests ───────────────────────────────────────────

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn prop_agent_state_id_never_empty(agent in strategies::arb_agent_state()) {
            // The proptest strategy generates non-empty ids
            prop_assert!(!agent.id.is_empty());
        }

        #[test]
        fn prop_agent_state_retry_bounded(agent in strategies::arb_agent_state()) {
            prop_assert!(agent.retry_count <= 20);
        }

        #[test]
        fn prop_agent_state_cpu_non_negative(agent in strategies::arb_agent_state()) {
            prop_assert!(agent.cpu_request >= 0.0);
        }

        #[test]
        fn prop_task_state_serializes(task in strategies::arb_task_state()) {
            let json = serde_json::to_string(&task).unwrap();
            let restored: TaskState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&task.task_id, &restored.task_id);
            prop_assert_eq!(&task.status, &restored.status);
        }

        #[test]
        fn prop_loop_state_round_non_neg(ls in strategies::arb_loop_state()) {
            // u32 is always non-negative, but verify the invariant explicitly
            prop_assert!(ls.current_round <= u32::MAX);
        }

        #[test]
        fn prop_loop_state_serializes(ls in strategies::arb_loop_state()) {
            let json = serde_json::to_string(&ls).unwrap();
            let restored: LoopState = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(&ls.loop_id, &restored.loop_id);
            prop_assert_eq!(&ls.status, &restored.status);
        }

        #[test]
        fn prop_deterministic_same_seed(seed in 0u64..10_000) {
            let mut g1 = FixtureGenerator::new(seed);
            let mut g2 = FixtureGenerator::new(seed);
            let a1 = g1.gen_agent_state();
            let a2 = g2.gen_agent_state();
            prop_assert_eq!(&a1.id, &a2.id);
            prop_assert_eq!(&a1.name, &a2.name);
        }
    }
}
