# Descheduler Design Document

> K8S-inspired agent rebalancing system for KIAS

## 1. Problem Statement

After initial scheduling, cluster nodes can become imbalanced due to:
- Node resource changes (scaling, failures)
- Agent resource consumption drift
- New affinity/anti-affinity constraints
- Priority class changes

The **Descheduler** periodically scans the cluster, detects imbalance, and generates eviction plans so the scheduler can reschedule agents to better nodes.

## 2. Architecture

```
┌─────────────────────────────────────────────────────┐
│                  DeschedulerEngine                   │
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────┐ │
│  │  LowNode     │  │  Remove      │  │  Remove    │ │
│  │  Utilization │  │  Duplicates  │  │  AntiAff   │ │
│  │  Strategy    │  │  Strategy    │  │  Violators │ │
│  └──────────────┘  └──────────────┘  └────────────┘ │
│                                                      │
│  ┌──────────────────────────────────────────────┐   │
│  │         EvictionPlan + PDB Guard             │   │
│  └──────────────────────────────────────────────┘   │
│                                                      │
│  ┌──────────────────────────────────────────────┐   │
│  │         DryRun / Execute mode                │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

## 3. Strategies

### 3.1 LowNodeUtilization

Detects nodes that are over/under-utilized compared to thresholds.

- **HighThreshold**: CPU/memory above which a node is "overloaded" (default: 80%)
- **LowThreshold**: CPU/memory below which a node is "underutilized" (default: 20%)
- Evicts agents from overloaded nodes so they can be rescheduled to underutilized ones

### 3.2 RemoveDuplicates

When multiple agents of the same type (same system_prompt_hash) are on the same node, evict duplicates to spread them across nodes for resilience.

### 3.3 RemoveAgentsViolatingAntiAffinity

Finds agents that violate their own anti-affinity rules (e.g., two agents with `avoid_agent_types` co-located) and evicts the lower-priority one.

## 4. Eviction Safety

### 4.1 Pod Disruption Budget (PDB) Equivalent

```rust
struct AgentDisruptionBudget {
    agent_type: String,
    min_available: usize,   // At least N must remain running
    max_unavailable: usize, // At most N can be evicted
}
```

### 4.2 EvictionPlan

```rust
struct EvictionPlan {
    evictions: Vec<Eviction>,
    dry_run: bool,
    timestamp: DateTime<Utc>,
}

struct Eviction {
    agent_id: String,
    source_node: String,
    reason: EvictionReason,
    priority: Priority,
}
```

## 5. Configuration

```rust
struct DeschedulerConfig {
    strategies: Vec<StrategyConfig>,
    dry_run: bool,
    max_evictions_per_cycle: usize,
    node_utilization_thresholds: UtilizationThresholds,
}
```

## 6. Integration Points

- **Input**: Cluster snapshot (nodes + agents) from Controller
- **Output**: EvictionPlan consumed by Controller's reconciler
- **Scheduler**: After eviction, Scheduler reschedules the agents
- **Monitoring**: Each cycle reports metrics (evictions, imbalance score)

## 7. Quality Gates

- Zero clippy warnings
- All strategies have ≥ 5 tests
- PDB constraints are never violated
- Dry-run mode produces identical plans without side effects
