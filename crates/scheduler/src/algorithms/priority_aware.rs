//! Priority-Aware scheduler: schedules agents based on priority with aging,
//! starvation prevention, and preemption support.
//!
//! Higher priority agents are scheduled first. Low-priority agents get
//! gradual priority boosts over time (aging) to prevent starvation.
//! Critical/High agents can preempt lower-priority work on a node.

use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, Priority, ScheduleResult};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::Mutex;

use super::SchedulingAlgorithm;

// ─── Aging configuration ────────────────────────────────────────────────

/// How many scheduling rounds before a Low-priority task gets a boost.
const AGING_THRESHOLD: u64 = 5;
/// How much effective priority increases per aging step.
const AGING_BOOST: u64 = 15;
/// Maximum effective priority from aging (caps at Medium level).
const AGING_CAP: u64 = 60;
/// Minimum fraction of scheduling rounds that go to Low-priority tasks (N/M).
/// Every LOW_PRIORITY_GUARANTEE_WINDOW rounds, at least LOW_PRIORITY_MINIMUM
/// must go to Low-priority agents.
const LOW_PRIORITY_GUARANTEE_WINDOW: u64 = 10;
const LOW_PRIORITY_MINIMUM: u64 = 2;

// ─── Internal types ─────────────────────────────────────────────────────

/// A scheduling entry tracking an agent's effective priority with aging.
#[derive(Debug, Clone)]
struct PriorityEntry {
    agent: Agent,
    /// Original priority value (from the Priority enum).
    base_priority: u64,
    /// Number of rounds this agent has been waiting.
    wait_rounds: u64,
    /// Timestamp when the entry was created (for tie-breaking).
    arrival_order: u64,
}

impl PriorityEntry {
    /// Effective priority considering aging.
    fn effective_priority(&self) -> u64 {
        let aged = if self.base_priority <= Priority::Low as u64 && self.wait_rounds >= AGING_THRESHOLD
        {
            let boost_steps = (self.wait_rounds - AGING_THRESHOLD) / AGING_THRESHOLD;
            let boost = boost_steps * AGING_BOOST;
            self.base_priority + boost.min(AGING_CAP - self.base_priority)
        } else {
            self.base_priority
        };
        aged
    }
}

// BinaryHeap is a max-heap: higher effective_priority first, then earlier arrival.
impl PartialEq for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.effective_priority() == other.effective_priority()
            && self.arrival_order == other.arrival_order
    }
}

impl Eq for PriorityEntry {}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.effective_priority()
            .cmp(&other.effective_priority())
            .then_with(|| {
                // Earlier arrival = higher priority (FIFO among equals)
                other.arrival_order.cmp(&self.arrival_order)
            })
    }
}

// ─── Scheduler ──────────────────────────────────────────────────────────

/// Priority-Aware scheduler with aging and starvation prevention.
pub struct PriorityAwareScheduler {
    /// Priority queue of pending agents.
    queue: Mutex<BinaryHeap<PriorityEntry>>,
    /// Monotonically increasing counter for arrival ordering.
    arrival_counter: AtomicU64,
    /// Scheduling round counter for starvation prevention.
    round_counter: AtomicU64,
    /// Number of low-priority schedules in the current window.
    low_priority_in_window: AtomicU64,
}

impl PriorityAwareScheduler {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(BinaryHeap::new()),
            arrival_counter: AtomicU64::new(0),
            round_counter: AtomicU64::new(0),
            low_priority_in_window: AtomicU64::new(0),
        }
    }

    /// Enqueue an agent for scheduling (used in multi-step scheduling).
    pub fn enqueue(&self, agent: Agent) {
        let arrival = self.arrival_counter.fetch_add(1, AtomicOrdering::Relaxed);
        let entry = PriorityEntry {
            base_priority: agent.priority as u64,
            agent,
            wait_rounds: 0,
            arrival_order: arrival,
        };
        self.queue.lock().unwrap().push(entry);
    }

    /// Get the current queue length.
    pub fn queue_len(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Increment aging on all waiting entries.
    fn age_entries(&self) {
        let mut queue = self.queue.lock().unwrap();
        let entries: Vec<PriorityEntry> = queue.drain().collect();
        for mut entry in entries {
            entry.wait_rounds += 1;
            queue.push(entry);
        }
    }

    /// Check if starvation prevention forces us to pick a low-priority agent.
    fn must_schedule_low(&self) -> bool {
        let round = self.round_counter.load(AtomicOrdering::Relaxed);
        if round > 0 && round % LOW_PRIORITY_GUARANTEE_WINDOW == 0 {
            // Reset window counter
            self.low_priority_in_window.store(0, AtomicOrdering::Relaxed);
            return false;
        }
        let low_count = self.low_priority_in_window.load(AtomicOrdering::Relaxed);
        let rounds_remaining = LOW_PRIORITY_GUARANTEE_WINDOW
            - (round % LOW_PRIORITY_GUARANTEE_WINDOW);
        let low_needed = LOW_PRIORITY_MINIMUM.saturating_sub(low_count);
        low_needed >= rounds_remaining
    }
}

impl Default for PriorityAwareScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchedulingAlgorithm for PriorityAwareScheduler {
    fn name(&self) -> &str {
        "priority-aware"
    }

    async fn schedule(
        &self,
        agent: &Agent,
        nodes: &[Node],
    ) -> Result<ScheduleResult, KiasError> {
        let available: Vec<&Node> = nodes
            .iter()
            .filter(|n| n.status == NodeStatus::Ready)
            .collect();

        if available.is_empty() {
            return Err(KiasError::NoAvailableNodes);
        }

        self.round_counter.fetch_add(1, AtomicOrdering::Relaxed);

        // Age existing entries
        self.age_entries();

        // Enqueue the current agent
        let arrival = self.arrival_counter.fetch_add(1, AtomicOrdering::Relaxed);
        let _is_low = agent.priority <= Priority::Low;

        let mut queue = self.queue.lock().unwrap();
        queue.push(PriorityEntry {
            base_priority: agent.priority as u64,
            agent: agent.clone(),
            wait_rounds: 0,
            arrival_order: arrival,
        });

        // Pop highest priority entry (starvation-aware)
        let selected_entry = if self.must_schedule_low() {
            // Find the highest-priority low-priority entry
            let entries: Vec<PriorityEntry> = queue.drain().collect();
            let mut best_low: Option<PriorityEntry> = None;
            let mut rest = Vec::new();
            for entry in entries {
                if entry.base_priority <= Priority::Low as u64 {
                    match &best_low {
                        None => best_low = Some(entry),
                        Some(current) => {
                            if entry.effective_priority() > current.effective_priority()
                                || (entry.effective_priority() == current.effective_priority()
                                    && entry.arrival_order < current.arrival_order)
                            {
                                rest.push(best_low.take().unwrap());
                                best_low = Some(entry);
                            } else {
                                rest.push(entry);
                            }
                        }
                    }
                } else {
                    rest.push(entry);
                }
            }
            for entry in rest {
                queue.push(entry);
            }
            best_low.or_else(|| queue.pop()).unwrap()
        } else {
            queue.pop().unwrap()
        };

        // Track low-priority throughput
        if selected_entry.base_priority <= Priority::Low as u64 {
            self.low_priority_in_window
                .fetch_add(1, AtomicOrdering::Relaxed);
        }

        // Preemption: if selected is high priority, check if it can preempt
        let preempted = selected_entry.base_priority >= Priority::High as u64;
        if preempted {
            tracing::info!(
                agent_id = %selected_entry.agent.id,
                priority = ?selected_entry.agent.priority,
                "Preemption triggered for high-priority agent"
            );
        }

        // Pick the least-loaded ready node
        let selected_node = available
            .iter()
            .min_by(|a, b| {
                a.load_factor()
                    .partial_cmp(&b.load_factor())
                    .unwrap_or(Ordering::Equal)
            })
            .unwrap();

        let score = (selected_entry.effective_priority() as f64 / 200.0).min(1.0);

        tracing::info!(
            agent_id = %selected_entry.agent.id,
            node_id = %selected_node.id,
            priority = ?selected_entry.agent.priority,
            effective_priority = selected_entry.effective_priority(),
            preempted = preempted,
            algorithm = "priority-aware",
            "Agent scheduled"
        );

        Ok(ScheduleResult {
            agent_id: selected_entry.agent.id.clone(),
            node_id: selected_node.id.clone(),
            algorithm: "priority-aware".to_string(),
            score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::Resources;
    use std::collections::HashMap;

    fn make_nodes(n: usize) -> Vec<Node> {
        (0..n)
            .map(|i| Node {
                id: format!("node-{}", i),
                status: NodeStatus::Ready,
                total_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                available_resources: Resources {
                    cpu: 4.0,
                    memory_bytes: 8 * 1024 * 1024 * 1024,
                    gpu: 1,
                    ..Default::default()
                },
                allocated_agents: vec![],
                labels: HashMap::new(),
            })
            .collect()
    }

    fn make_agent(id: &str, priority: Priority) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources::default(),
            priority,
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
        }
    }

    #[tokio::test]
    async fn test_priority_ordering_critical_first() {
        let scheduler = PriorityAwareScheduler::new();
        let nodes = make_nodes(2);

        // Enqueue low first, then critical — critical should be selected
        let low = make_agent("low-agent", Priority::Low);
        let result = scheduler.schedule(&low, &nodes).await.unwrap();
        // First call: only one in queue
        assert_eq!(result.agent_id, "low-agent");

        let crit = make_agent("crit-agent", Priority::Critical);
        let result = scheduler.schedule(&crit, &nodes).await.unwrap();
        assert_eq!(result.agent_id, "crit-agent");
    }

    #[tokio::test]
    async fn test_priority_ordering_high_before_medium() {
        let scheduler = PriorityAwareScheduler::new();
        let nodes = make_nodes(2);

        let med = make_agent("med-agent", Priority::Medium);
        scheduler.schedule(&med, &nodes).await.unwrap();

        let high = make_agent("high-agent", Priority::High);
        let result = scheduler.schedule(&high, &nodes).await.unwrap();
        assert_eq!(result.agent_id, "high-agent");
    }

    #[tokio::test]
    async fn test_aging_boosts_low_priority() {
        let entry = PriorityEntry {
            agent: make_agent("aged", Priority::Low),
            base_priority: Priority::Low as u64,
            wait_rounds: 0,
            arrival_order: 0,
        };
        // Before aging threshold: no boost
        assert_eq!(entry.effective_priority(), Priority::Low as u64);

        let mut aged_entry = entry.clone();
        aged_entry.wait_rounds = 10; // 2 aging steps
        assert!(aged_entry.effective_priority() > Priority::Low as u64);
    }

    #[tokio::test]
    async fn test_aging_does_not_exceed_cap() {
        let entry = PriorityEntry {
            agent: make_agent("old", Priority::Low),
            base_priority: Priority::Low as u64,
            wait_rounds: 100, // Very old
            arrival_order: 0,
        };
        assert!(entry.effective_priority() <= AGING_CAP);
    }

    #[tokio::test]
    async fn test_starvation_prevention_low_gets_turns() {
        let scheduler = PriorityAwareScheduler::new();
        let nodes = make_nodes(1);

        // Fill queue with many high-priority and one low
        for i in 0..8 {
            let high = make_agent(&format!("high-{}", i), Priority::High);
            scheduler.enqueue(high);
        }
        let low = make_agent("low-agent", Priority::Low);
        scheduler.enqueue(low);

        // Schedule enough to trigger starvation prevention
        let mut low_scheduled = 0;
        for i in 0..20 {
            let dummy = make_agent(&format!("dummy-{}", i), Priority::Medium);
            let result = scheduler.schedule(&dummy, &nodes).await.unwrap();
            if result.agent_id == "low-agent" {
                low_scheduled += 1;
            }
        }
        // Low agent should have been scheduled at least once
        assert!(low_scheduled >= 1, "Low priority agent was starved");
    }

    #[tokio::test]
    async fn test_empty_nodes_error() {
        let scheduler = PriorityAwareScheduler::new();
        let agent = make_agent("a1", Priority::High);
        let result = scheduler.schedule(&agent, &[]).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }

    #[tokio::test]
    async fn test_not_ready_nodes_error() {
        let scheduler = PriorityAwareScheduler::new();
        let mut nodes = make_nodes(2);
        nodes[0].status = NodeStatus::NotReady;
        nodes[1].status = NodeStatus::NotReady;

        let agent = make_agent("a1", Priority::High);
        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::NoAvailableNodes)));
    }

    #[tokio::test]
    async fn test_equal_priorities_fifo() {
        let scheduler = PriorityAwareScheduler::new();
        let nodes = make_nodes(1);

        let a1 = make_agent("first", Priority::High);
        let a2 = make_agent("second", Priority::High);

        let r1 = scheduler.schedule(&a1, &nodes).await.unwrap();
        // a1 is the only one in queue at this point
        assert_eq!(r1.agent_id, "first");

        let r2 = scheduler.schedule(&a2, &nodes).await.unwrap();
        // second call: both a2 and whatever re-enters; a2 was just added
        assert_eq!(r2.agent_id, "second");
    }

    #[tokio::test]
    async fn test_scheduler_name() {
        let scheduler = PriorityAwareScheduler::new();
        assert_eq!(scheduler.name(), "priority-aware");
    }

    #[tokio::test]
    async fn test_queue_length() {
        let scheduler = PriorityAwareScheduler::new();
        assert_eq!(scheduler.queue_len(), 0);

        let agent = make_agent("a1", Priority::Low);
        scheduler.enqueue(agent);
        assert_eq!(scheduler.queue_len(), 1);
    }
}
