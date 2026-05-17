//! # Kanban Dispatcher
//!
//! Auto-scheduling engine for the Kanban board with three components:
//! - **Scanner**: Auto-advances tasks through columns
//! - **Allocator**: Assigns Ready tasks to available agents
//! - **Reclaimer**: Detects and recovers stale InProgress tasks

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

use super::kanban::{KanbanBoard, KanbanColumn};

/// Agent availability status
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStatus {
    /// Agent is idle and can accept tasks
    Idle,
    /// Agent is busy with a task
    Busy(String),
    /// Agent is offline/unavailable
    Offline,
}

/// Agent registration for the dispatcher
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub id: String,
    pub capabilities: Vec<String>,
    pub status: AgentStatus,
    pub max_tasks: usize,
    pub current_tasks: usize,
}

/// Dispatcher configuration
#[derive(Debug, Clone)]
pub struct DispatcherConfig {
    /// How often to run the scan cycle
    pub scan_interval: Duration,
    /// How long before a task is considered stale
    pub stale_threshold: Duration,
    /// Maximum retries before circuit breaker
    pub max_retries: u32,
    /// Enable auto-advance from Triage/Todo
    pub auto_advance: bool,
    /// Enable auto-claim by allocator
    pub auto_claim: bool,
}

impl Default for DispatcherConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(60),
            stale_threshold: Duration::from_secs(3600),
            max_retries: 3,
            auto_advance: true,
            auto_claim: true,
        }
    }
}

/// Dispatcher events for observability
#[derive(Debug, Clone)]
pub enum DispatcherEvent {
    AutoAdvanced {
        task_id: String,
        from: KanbanColumn,
        to: KanbanColumn,
    },
    Allocated {
        task_id: String,
        agent_id: String,
    },
    StaleDetected {
        task_id: String,
        column: KanbanColumn,
        duration: Duration,
    },
    Reclaimed {
        task_id: String,
        reason: String,
    },
    CircuitBroken {
        task_id: String,
        failure_count: u32,
    },
}

/// The Dispatcher — auto-scheduling engine
pub struct Dispatcher {
    config: DispatcherConfig,
    agents: HashMap<String, AgentInfo>,
    events: Vec<DispatcherEvent>,
}

impl Dispatcher {
    pub fn new(config: DispatcherConfig) -> Self {
        Self {
            config,
            agents: HashMap::new(),
            events: Vec::new(),
        }
    }

    pub fn register_agent(&mut self, agent: AgentInfo) {
        info!(agent_id = %agent.id, "Registered agent");
        self.agents.insert(agent.id.clone(), agent);
    }

    pub fn unregister_agent(&mut self, agent_id: &str) {
        self.agents.remove(agent_id);
        info!(agent_id, "Unregistered agent");
    }

    pub fn set_agent_status(&mut self, agent_id: &str, status: AgentStatus) {
        if let Some(agent) = self.agents.get_mut(agent_id) {
            agent.status = status;
        }
    }

    pub fn events(&self) -> &[DispatcherEvent] {
        &self.events
    }

    pub fn drain_events(&mut self) -> Vec<DispatcherEvent> {
        std::mem::take(&mut self.events)
    }

    // ─── Scanner ───

    /// Auto-advance: Triage→Todo, Todo→Ready (if deps met)
    pub fn scan(&mut self, board: &mut KanbanBoard) -> Vec<DispatcherEvent> {
        let mut events = Vec::new();
        if !self.config.auto_advance {
            return events;
        }

        // Triage → Todo
        let triage_ids: Vec<String> = board
            .tasks
            .iter()
            .filter(|t| t.column == KanbanColumn::Triage)
            .map(|t| t.id.clone())
            .collect();

        for task_id in triage_ids {
            if board
                .move_to(&task_id, KanbanColumn::Todo, "dispatcher:scan", None)
                .is_ok()
            {
                let ev = DispatcherEvent::AutoAdvanced {
                    task_id: task_id.clone(),
                    from: KanbanColumn::Triage,
                    to: KanbanColumn::Todo,
                };
                events.push(ev.clone());
                self.events.push(ev);
            }
        }

        // Todo → Ready (deps met)
        let advanceable: Vec<String> = board
            .auto_advanceable()
            .iter()
            .map(|t| t.id.clone())
            .collect();

        for task_id in advanceable {
            if board
                .move_to(
                    &task_id,
                    KanbanColumn::Ready,
                    "dispatcher:auto-advance",
                    None,
                )
                .is_ok()
            {
                let ev = DispatcherEvent::AutoAdvanced {
                    task_id: task_id.clone(),
                    from: KanbanColumn::Todo,
                    to: KanbanColumn::Ready,
                };
                events.push(ev.clone());
                self.events.push(ev);
            }
        }

        events
    }

    // ─── Allocator ───

    /// Assign Ready tasks to idle agents (capability matching)
    pub fn allocate(&mut self, board: &mut KanbanBoard) -> Vec<DispatcherEvent> {
        let mut events = Vec::new();
        if !self.config.auto_claim {
            return events;
        }

        let claimable: Vec<(String, Vec<String>)> = board
            .claimable_tasks()
            .iter()
            .map(|t| {
                (
                    t.id.clone(),
                    t.required_capabilities
                        .iter()
                        .map(|c| c.0.clone())
                        .collect(),
                )
            })
            .collect();

        for (task_id, required_caps) in claimable {
            let Some(agent_id) = self.find_agent(&required_caps) else {
                continue;
            };
            if board.claim(&task_id, &agent_id).is_ok() {
                if let Some(agent) = self.agents.get_mut(&agent_id) {
                    agent.status = AgentStatus::Busy(task_id.clone());
                    agent.current_tasks += 1;
                }
                let ev = DispatcherEvent::Allocated {
                    task_id: task_id.clone(),
                    agent_id: agent_id.clone(),
                };
                events.push(ev.clone());
                self.events.push(ev);
                info!(task_id, agent_id, "Task allocated");
            }
        }
        events
    }

    fn find_agent(&self, required_caps: &[String]) -> Option<String> {
        let mut candidates: Vec<(&AgentInfo, usize)> = self
            .agents
            .values()
            .filter(|a| a.status == AgentStatus::Idle && a.current_tasks < a.max_tasks)
            .filter(|a| {
                required_caps.is_empty() || required_caps.iter().all(|c| a.capabilities.contains(c))
            })
            .map(|a| {
                let score = a
                    .capabilities
                    .iter()
                    .filter(|c| required_caps.contains(c))
                    .count();
                (a, score)
            })
            .collect();
        candidates.sort_by_key(|b| std::cmp::Reverse(b.1));
        candidates.first().map(|(agent, _)| agent.id.clone())
    }

    // ─── Reclaimer ───

    /// Detect stale InProgress tasks and reclaim or circuit-break them.
    pub fn reclaim(&mut self, board: &mut KanbanBoard) -> Vec<DispatcherEvent> {
        let mut events = Vec::new();

        // Collect stale task info to avoid borrow conflicts
        let stale_info: Vec<(String, u32, Duration)> = board
            .stale_tasks(self.config.stale_threshold)
            .iter()
            .filter(|t| t.column == KanbanColumn::InProgress)
            .map(|t| {
                let dur = SystemTime::now()
                    .duration_since(t.column_entered_at)
                    .unwrap_or(Duration::ZERO);
                (t.id.clone(), t.failure_count, dur)
            })
            .collect();

        for (task_id, failure_count, duration) in stale_info {
            let ev = DispatcherEvent::StaleDetected {
                task_id: task_id.clone(),
                column: KanbanColumn::InProgress,
                duration,
            };
            events.push(ev.clone());
            self.events.push(ev);

            if failure_count >= self.config.max_retries {
                if board
                    .block(
                        &task_id,
                        "dispatcher:reclaimer",
                        &format!(
                            "Circuit breaker: {} failures, stale for {:?}",
                            failure_count, duration
                        ),
                    )
                    .is_ok()
                {
                    let ev = DispatcherEvent::CircuitBroken {
                        task_id: task_id.clone(),
                        failure_count,
                    };
                    events.push(ev.clone());
                    self.events.push(ev);
                    warn!(task_id, failures = failure_count, "Task circuit-broken");
                }
            } else {
                let _ = board.unassign(&task_id);
                if board
                    .move_to(
                        &task_id,
                        KanbanColumn::Ready,
                        "dispatcher:reclaimer",
                        Some(format!("Reclaimed after {:?} stale", duration)),
                    )
                    .is_ok()
                {
                    let ev = DispatcherEvent::Reclaimed {
                        task_id: task_id.clone(),
                        reason: format!("Stale for {:?}", duration),
                    };
                    events.push(ev.clone());
                    self.events.push(ev);
                    info!(task_id, "Task reclaimed");
                }
            }
        }
        events
    }

    // ─── Full cycle ───

    /// Run a full dispatcher cycle: scan → allocate → reclaim
    pub fn tick(&mut self, board: &mut KanbanBoard) -> Vec<DispatcherEvent> {
        let mut all = Vec::new();
        all.extend(self.scan(board));
        all.extend(self.allocate(board));
        all.extend(self.reclaim(board));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanban::{Capability, KanbanBoard, KanbanTask, Priority, WipLimit};
    use std::collections::HashMap;
    use std::time::SystemTime;

    fn make_task(id: &str, title: &str, column: KanbanColumn) -> KanbanTask {
        KanbanTask {
            id: id.to_string(),
            board_id: "b1".to_string(),
            title: title.to_string(),
            description: String::new(),
            column,
            priority: Priority::Medium,
            required_capabilities: vec![],
            assigned_to: None,
            created_at: SystemTime::now(),
            column_entered_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            history: vec![],
            tags: vec![],
            estimated_minutes: None,
            blocked_by: vec![],
            parents: vec![],
            children: vec![],
            workspace: None,
            failure_count: 0,
            block_reason: None,
            metadata: HashMap::new(),
        }
    }

    fn make_dispatcher() -> Dispatcher {
        Dispatcher::new(DispatcherConfig::default())
    }

    #[test]
    fn test_scan_triage_to_todo() {
        let mut d = make_dispatcher();
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Task", KanbanColumn::Triage))
            .unwrap();
        let events = d.scan(&mut board);
        // scan does Triage→Todo then Todo→Ready in one pass
        assert_eq!(events.len(), 2);
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);
    }

    #[test]
    fn test_scan_todo_to_ready() {
        let mut d = make_dispatcher();
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Task", KanbanColumn::Todo))
            .unwrap();
        let events = d.scan(&mut board);
        assert_eq!(events.len(), 1);
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);
    }

    #[test]
    fn test_scan_blocked_by_dependency() {
        let mut d = make_dispatcher();
        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Parent", KanbanColumn::InProgress))
            .unwrap();
        let mut t2 = make_task("t2", "Child", KanbanColumn::Todo);
        t2.blocked_by = vec!["t1".to_string()];
        board.add_task(t2).unwrap();

        d.scan(&mut board);
        assert_eq!(board.tasks[1].column, KanbanColumn::Todo);
    }

    #[test]
    fn test_allocate_to_idle_agent() {
        let mut d = make_dispatcher();
        d.register_agent(AgentInfo {
            id: "agent-1".to_string(),
            capabilities: vec!["rust".to_string()],
            status: AgentStatus::Idle,
            max_tasks: 3,
            current_tasks: 0,
        });

        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Task", KanbanColumn::Ready))
            .unwrap();

        let events = d.allocate(&mut board);
        assert_eq!(events.len(), 1);
        assert_eq!(board.tasks[0].column, KanbanColumn::InProgress);
        assert_eq!(board.tasks[0].assigned_to, Some("agent-1".to_string()));
    }

    #[test]
    fn test_allocate_skips_busy_agent() {
        let mut d = make_dispatcher();
        d.register_agent(AgentInfo {
            id: "agent-1".to_string(),
            capabilities: vec![],
            status: AgentStatus::Busy("other".to_string()),
            max_tasks: 3,
            current_tasks: 1,
        });

        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Task", KanbanColumn::Ready))
            .unwrap();

        let events = d.allocate(&mut board);
        assert!(events.is_empty());
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);
    }

    #[test]
    fn test_allocate_capability_matching() {
        let mut d = make_dispatcher();
        d.register_agent(AgentInfo {
            id: "agent-py".to_string(),
            capabilities: vec!["python".to_string()],
            status: AgentStatus::Idle,
            max_tasks: 3,
            current_tasks: 0,
        });
        d.register_agent(AgentInfo {
            id: "agent-rs".to_string(),
            capabilities: vec!["rust".to_string()],
            status: AgentStatus::Idle,
            max_tasks: 3,
            current_tasks: 0,
        });

        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Rust Task", KanbanColumn::Ready);
        t.required_capabilities = vec![Capability("rust".to_string())];
        board.add_task(t).unwrap();

        d.allocate(&mut board);
        assert_eq!(board.tasks[0].assigned_to, Some("agent-rs".to_string()));
    }

    #[test]
    fn test_reclaim_stale_task() {
        let mut d = make_dispatcher();
        d.config.stale_threshold = Duration::ZERO;

        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Stale", KanbanColumn::InProgress);
        t.column_entered_at = SystemTime::now() - Duration::from_secs(7200);
        t.assigned_to = Some("agent-1".to_string());
        board.add_task(t).unwrap();

        d.reclaim(&mut board);
        assert_eq!(board.tasks[0].column, KanbanColumn::Ready);
        assert!(board.tasks[0].assigned_to.is_none());
    }

    #[test]
    fn test_reclaim_circuit_breaker() {
        let mut d = make_dispatcher();
        d.config.stale_threshold = Duration::ZERO;
        d.config.max_retries = 3;

        let mut board = KanbanBoard::new("b1", "test");
        let mut t = make_task("t1", "Broken", KanbanColumn::InProgress);
        t.column_entered_at = SystemTime::now() - Duration::from_secs(7200);
        t.failure_count = 3;
        board.add_task(t).unwrap();

        let events = d.reclaim(&mut board);
        assert!(events
            .iter()
            .any(|e| matches!(e, DispatcherEvent::CircuitBroken { .. })));
        assert_eq!(board.tasks[0].column, KanbanColumn::Blocked);
    }

    #[test]
    fn test_full_tick() {
        let mut d = make_dispatcher();
        // Register multiple agents so all Ready tasks can be claimed
        for i in 0..5 {
            d.register_agent(AgentInfo {
                id: format!("a{}", i),
                capabilities: vec![],
                status: AgentStatus::Idle,
                max_tasks: 1,
                current_tasks: 0,
            });
        }

        let mut board = KanbanBoard::new("b1", "test");
        board
            .add_task(make_task("t1", "Triage", KanbanColumn::Triage))
            .unwrap();
        board
            .add_task(make_task("t2", "Todo", KanbanColumn::Todo))
            .unwrap();
        board
            .add_task(make_task("t3", "Ready", KanbanColumn::Ready))
            .unwrap();

        let events = d.tick(&mut board);
        // t1: Triage→Todo→Ready (2 events), t2: Todo→Ready (1 event), t3+ others claimed
        assert!(events.len() >= 4);

        // t3 should be claimed (was already Ready before tick)
        let t3 = board.tasks.iter().find(|t| t.id == "t3").unwrap();
        assert_eq!(t3.column, KanbanColumn::InProgress);
        assert!(t3.assigned_to.is_some());
    }

    #[test]
    fn test_register_unregister() {
        let mut d = make_dispatcher();
        d.register_agent(AgentInfo {
            id: "a1".to_string(),
            capabilities: vec![],
            status: AgentStatus::Idle,
            max_tasks: 1,
            current_tasks: 0,
        });
        assert_eq!(d.agents.len(), 1);
        d.unregister_agent("a1");
        assert!(d.agents.is_empty());
    }

    #[test]
    fn test_drain_events() {
        let mut d = make_dispatcher();
        d.events.push(DispatcherEvent::AutoAdvanced {
            task_id: "t1".to_string(),
            from: KanbanColumn::Triage,
            to: KanbanColumn::Todo,
        });
        let events = d.drain_events();
        assert_eq!(events.len(), 1);
        assert!(d.events.is_empty());
    }
}
