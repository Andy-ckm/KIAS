//! Auto-scaling — load-driven agent pool scaling.
//!
//! Architecture reference:
//! - Kubernetes Horizontal Pod Autoscaler (HPA) algorithm
//! - AWS Application Auto Scaling target-tracking policy
//! - Paper: "Borg, Omega, and Kubernetes" (ACM Queue, 2016)
//!
//! Pattern: Target-tracking with stabilization window to prevent flapping.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Scaling policy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScaleConfig {
    pub min_replicas: u32,
    pub max_replicas: u32,
    /// Target CPU utilization percentage (0-100).
    pub target_cpu_utilization: f64,
    /// Target memory utilization percentage (0-100).
    pub target_memory_utilization: f64,
    /// Target queue depth per agent (pending tasks per agent).
    pub target_queue_depth: f64,
    /// Cooldown period after a scaling event (ms).
    pub scale_cooldown_ms: u64,
    /// Evaluation interval (ms).
    pub evaluation_interval_ms: u64,
    /// Number of consecutive evaluations before scaling (prevents flapping).
    pub stabilization_window: u32,
    pub scale_up_step: u32,
    pub scale_down_step: u32,
}

impl Default for AutoScaleConfig {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            max_replicas: 100,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            target_queue_depth: 10.0,
            scale_cooldown_ms: 60_000,
            evaluation_interval_ms: 15_000,
            stabilization_window: 3,
            scale_up_step: 2,
            scale_down_step: 1,
        }
    }
}

/// Current metrics snapshot for an agent pool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub current_replicas: u32,
    pub avg_cpu_utilization: f64,
    pub avg_memory_utilization: f64,
    pub pending_tasks: u32,
    pub active_tasks: u32,
    pub avg_response_time_ms: f64,
    pub error_rate: f64,
    pub timestamp_ms: u64,
}

impl Default for PoolMetrics {
    fn default() -> Self {
        Self {
            current_replicas: 1,
            avg_cpu_utilization: 0.0,
            avg_memory_utilization: 0.0,
            pending_tasks: 0,
            active_tasks: 0,
            avg_response_time_ms: 0.0,
            error_rate: 0.0,
            timestamp_ms: now_ms(),
        }
    }
}

/// Scaling decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScalingDecision {
    NoChange,
    ScaleUp(u32),
    ScaleDown(u32),
}

/// Auto-scaler for agent pools.
pub struct AutoScaler {
    config: AutoScaleConfig,
    pools: std::sync::Mutex<HashMap<String, AutoScaleState>>,
}

struct AutoScaleState {
    config: AutoScaleConfig,
    current_replicas: u32,
    last_scale_event_ms: Option<u64>,
    consecutive_scale_up: u32,
    consecutive_scale_down: u32,
    events: Vec<ScalingEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingEvent {
    pub timestamp_ms: u64,
    pub decision: ScalingDecision,
    pub reason: String,
    pub before_replicas: u32,
    pub after_replicas: u32,
}

impl AutoScaler {
    pub fn new(config: AutoScaleConfig) -> Self {
        Self {
            config,
            pools: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Register a new agent pool for auto-scaling.
    pub fn register_pool(
        &self,
        pool_id: String,
        initial_replicas: u32,
        pool_config: Option<AutoScaleConfig>,
    ) -> Result<(), String> {
        let mut pools = self.pools.lock().map_err(|e| e.to_string())?;
        pools.insert(
            pool_id,
            AutoScaleState {
                config: pool_config.unwrap_or_else(|| self.config.clone()),
                current_replicas: initial_replicas,
                last_scale_event_ms: None,
                consecutive_scale_up: 0,
                consecutive_scale_down: 0,
                events: Vec::new(),
            },
        );
        Ok(())
    }

    /// Evaluate metrics and return a scaling decision.
    pub fn evaluate(&self, pool_id: &str, metrics: PoolMetrics) -> Result<ScalingDecision, String> {
        let mut pools = self.pools.lock().map_err(|e| e.to_string())?;
        let state = pools
            .get_mut(pool_id)
            .ok_or_else(|| format!("Pool '{}' not registered", pool_id))?;

        let now = now_ms();

        // Check cooldown
        if let Some(last_scale) = state.last_scale_event_ms {
            if now - last_scale < state.config.scale_cooldown_ms {
                return Ok(ScalingDecision::NoChange);
            }
        }

        let mut desired_replicas = metrics.current_replicas;

        // CPU-based scaling
        if metrics.avg_cpu_utilization > state.config.target_cpu_utilization {
            let scale_factor = metrics.avg_cpu_utilization / state.config.target_cpu_utilization;
            desired_replicas = (metrics.current_replicas as f64 * scale_factor).ceil() as u32;
        }

        // Memory-based scaling
        if metrics.avg_memory_utilization > state.config.target_memory_utilization {
            let scale_factor =
                metrics.avg_memory_utilization / state.config.target_memory_utilization;
            let mem_desired = (metrics.current_replicas as f64 * scale_factor).ceil() as u32;
            desired_replicas = desired_replicas.max(mem_desired);
        }

        // Queue-depth-based scaling
        if state.config.target_queue_depth > 0.0 && metrics.current_replicas > 0 {
            let tasks_per_agent = metrics.pending_tasks as f64 / metrics.current_replicas as f64;
            if tasks_per_agent > state.config.target_queue_depth {
                let q_desired =
                    (metrics.pending_tasks as f64 / state.config.target_queue_depth).ceil() as u32;
                desired_replicas = desired_replicas.max(q_desired);
            }
        }

        let decision = if desired_replicas > metrics.current_replicas {
            let delta =
                (desired_replicas - metrics.current_replicas).min(state.config.scale_up_step);
            ScalingDecision::ScaleUp(delta)
        } else if desired_replicas < metrics.current_replicas {
            // Only scale down if below 50% of target
            let below_target = metrics.avg_cpu_utilization
                < state.config.target_cpu_utilization * 0.5
                && metrics.avg_memory_utilization < state.config.target_memory_utilization * 0.5;
            if below_target && metrics.current_replicas > state.config.min_replicas {
                let excess = metrics.current_replicas - state.config.min_replicas;
                let delta = excess.min(state.config.scale_down_step);
                ScalingDecision::ScaleDown(delta)
            } else {
                ScalingDecision::NoChange
            }
        } else {
            // No scale change detected from metrics
            // Check if under-utilized and should scale down anyway
            let under_utilized = metrics.avg_cpu_utilization
                < state.config.target_cpu_utilization * 0.5
                && metrics.avg_memory_utilization < state.config.target_memory_utilization * 0.5;
            if under_utilized && metrics.current_replicas > state.config.min_replicas {
                let excess = metrics.current_replicas - state.config.min_replicas;
                let delta = excess.min(state.config.scale_down_step);
                ScalingDecision::ScaleDown(delta)
            } else {
                ScalingDecision::NoChange
            }
        };

        // Stabilization window check
        match &decision {
            ScalingDecision::ScaleUp(_) => {
                state.consecutive_scale_up += 1;
                state.consecutive_scale_down = 0;
                if state.consecutive_scale_up < state.config.stabilization_window {
                    return Ok(ScalingDecision::NoChange);
                }
            }
            ScalingDecision::ScaleDown(_) => {
                state.consecutive_scale_down += 1;
                state.consecutive_scale_up = 0;
                if state.consecutive_scale_down < state.config.stabilization_window {
                    return Ok(ScalingDecision::NoChange);
                }
            }
            ScalingDecision::NoChange => {
                state.consecutive_scale_up = 0;
                state.consecutive_scale_down = 0;
            }
        }

        // Apply
        let new_replicas = match &decision {
            ScalingDecision::ScaleUp(n) => {
                (state.current_replicas + n).min(state.config.max_replicas)
            }
            ScalingDecision::ScaleDown(n) => {
                (state.current_replicas - n).max(state.config.min_replicas)
            }
            ScalingDecision::NoChange => state.current_replicas,
        };

        if new_replicas != state.current_replicas {
            state.events.push(ScalingEvent {
                timestamp_ms: now,
                decision: decision.clone(),
                reason: format!(
                    "cpu={:.1}% mem={:.1}% pending={}",
                    metrics.avg_cpu_utilization,
                    metrics.avg_memory_utilization,
                    metrics.pending_tasks
                ),
                before_replicas: state.current_replicas,
                after_replicas: new_replicas,
            });
            state.current_replicas = new_replicas;
            state.last_scale_event_ms = Some(now);
        }

        Ok(decision)
    }

    pub fn get_replicas(&self, pool_id: &str) -> Option<u32> {
        let pools = self.pools.lock().ok()?;
        pools.get(pool_id).map(|s| s.current_replicas)
    }

    pub fn set_replicas(&self, pool_id: &str, replicas: u32) -> Result<(), String> {
        let mut pools = self.pools.lock().map_err(|e| e.to_string())?;
        let state = pools
            .get_mut(pool_id)
            .ok_or_else(|| format!("Pool '{}' not registered", pool_id))?;
        state.current_replicas = replicas
            .max(state.config.min_replicas)
            .min(state.config.max_replicas);
        Ok(())
    }

    pub fn get_events(&self, pool_id: &str, limit: usize) -> Vec<ScalingEvent> {
        let pools = self.pools.lock().unwrap_or_else(|e| e.into_inner());
        pools
            .get(pool_id)
            .map(|s| {
                let start = s.events.len().saturating_sub(limit);
                s.events[start..].to_vec()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(replicas: u32, cpu: f64, mem: f64, pending: u32) -> PoolMetrics {
        PoolMetrics {
            current_replicas: replicas,
            avg_cpu_utilization: cpu,
            avg_memory_utilization: mem,
            pending_tasks: pending,
            ..Default::default()
        }
    }

    #[test]
    fn test_register_and_get() {
        let scaler = AutoScaler::new(AutoScaleConfig::default());
        scaler.register_pool("p1".to_string(), 3, None).unwrap();
        assert_eq!(scaler.get_replicas("p1"), Some(3));
    }

    #[test]
    fn test_no_change_when_balanced() {
        let scaler = AutoScaler::new(AutoScaleConfig {
            stabilization_window: 1,
            ..Default::default()
        });
        scaler.register_pool("p1".to_string(), 3, None).unwrap();
        let decision = scaler
            .evaluate("p1", make_metrics(3, 50.0, 50.0, 10))
            .unwrap();
        assert_eq!(decision, ScalingDecision::NoChange);
    }

    #[test]
    fn test_scale_down_when_underutilized() {
        let config = AutoScaleConfig {
            stabilization_window: 1,
            min_replicas: 1,
            scale_cooldown_ms: 0,
            ..Default::default()
        };
        let scaler = AutoScaler::new(config);
        scaler.register_pool("p1".to_string(), 5, None).unwrap();
        for _ in 0..3 {
            // metrics.current_replicas should be 4 to trigger scale-down decision
            // (desired_replicas < metrics.current_replicas = 4 < 5 triggers scale-down)
            scaler
                .evaluate("p1", make_metrics(4, 10.0, 10.0, 0))
                .unwrap();
        }
        assert!(scaler.get_replicas("p1").unwrap() < 5);
    }

    #[test]
    fn test_manual_set() {
        let scaler = AutoScaler::new(AutoScaleConfig::default());
        scaler.register_pool("p1".to_string(), 3, None).unwrap();
        scaler.set_replicas("p1", 10).unwrap();
        assert_eq!(scaler.get_replicas("p1"), Some(10));
    }
}
