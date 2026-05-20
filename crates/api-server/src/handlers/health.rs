use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::time::Instant;

use crate::models::request::{ComponentHealth, HealthResponse};
use crate::AppState;

/// System uptime tracking
static START_TIME: once_cell::sync::Lazy<Instant> = once_cell::sync::Lazy::new(Instant::now);

/// GET /health
/// Liveness probe — always returns 200 if the server is up
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok"
    }))
}

/// Deep health check response
#[derive(Debug, Serialize)]
pub struct DeepHealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub components: Vec<ComponentHealth>,
    pub system: SystemInfo,
}

/// System resource information
#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_usage_percent: f64,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
    pub disk_usage_percent: f64,
    pub cpu_cores: usize,
    pub load_average: LoadAverage,
}

/// System load averages
#[derive(Debug, Serialize)]
pub struct LoadAverage {
    pub one_min: f64,
    pub five_min: f64,
    pub fifteen_min: f64,
}

/// GET /readyz
/// Readiness probe — checks that internal state is usable
pub async fn readiness(State(state): State<AppState>) -> Json<HealthResponse> {
    let mut components = vec![];

    // Check agents store
    let agents_healthy = {
        let _lock = state.agents.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "agents_store".to_string(),
        status: if agents_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
    });

    // Check nodes store
    let nodes_healthy = {
        let _lock = state.nodes.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "nodes_store".to_string(),
        status: if nodes_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
    });

    let overall = if agents_healthy && nodes_healthy {
        "healthy"
    } else {
        "degraded"
    };

    Json(HealthResponse {
        status: overall.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        components,
    })
}

/// GET /healthz/deep
/// Deep health check with system resource monitoring
pub async fn deep_health(State(state): State<AppState>) -> Json<DeepHealthResponse> {
    let mut components = vec![];
    let mut all_healthy = true;

    // Check agents store
    let agents_healthy = {
        let _lock = state.agents.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "agents_store".to_string(),
        status: if agents_healthy {
            "healthy".to_string()
        } else {
            all_healthy = false;
            "unhealthy".to_string()
        },
    });

    // Check nodes store
    let nodes_healthy = {
        let _lock = state.nodes.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "nodes_store".to_string(),
        status: if nodes_healthy {
            "healthy".to_string()
        } else {
            all_healthy = false;
            "unhealthy".to_string()
        },
    });

    // Check workflows store
    let workflows_healthy = {
        let _lock = state.workflows.try_read();
        _lock.is_ok()
    };
    components.push(ComponentHealth {
        name: "workflows_store".to_string(),
        status: if workflows_healthy {
            "healthy".to_string()
        } else {
            all_healthy = false;
            "unhealthy".to_string()
        },
    });

    // Check event bus (if it has capacity)
    components.push(ComponentHealth {
        name: "event_bus".to_string(),
        status: "healthy".to_string(),
    });

    // Get system information
    let system = get_system_info();

    // Check memory usage
    let memory_healthy = system.memory_usage_percent < 95.0;
    components.push(ComponentHealth {
        name: "memory".to_string(),
        status: if memory_healthy {
            "healthy".to_string()
        } else {
            all_healthy = false;
            "unhealthy".to_string()
        },
    });

    // Check disk usage
    let disk_healthy = system.disk_usage_percent < 95.0;
    components.push(ComponentHealth {
        name: "disk".to_string(),
        status: if disk_healthy {
            "healthy".to_string()
        } else {
            all_healthy = false;
            "unhealthy".to_string()
        },
    });

    // Check load average (warn if > 2x CPU cores)
    let load_healthy = system.load_average.one_min < (system.cpu_cores as f64 * 2.0);
    components.push(ComponentHealth {
        name: "load_average".to_string(),
        status: if load_healthy {
            "healthy".to_string()
        } else {
            "degraded".to_string()
        },
    });

    let overall = if all_healthy { "healthy" } else { "unhealthy" };

    Json(DeepHealthResponse {
        status: overall.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: START_TIME.elapsed().as_secs(),
        components,
        system,
    })
}

/// Get system resource information
fn get_system_info() -> SystemInfo {
    let (memory_used, memory_total) = get_memory_info();
    let (disk_used, disk_total) = get_disk_info();
    let cpu_cores = num_cpus::get();
    let load_avg = get_load_average();

    SystemInfo {
        memory_used_mb: memory_used,
        memory_total_mb: memory_total,
        memory_usage_percent: if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        },
        disk_used_gb: disk_used as f64 / 1024.0 / 1024.0 / 1024.0,
        disk_total_gb: disk_total as f64 / 1024.0 / 1024.0 / 1024.0,
        disk_usage_percent: if disk_total > 0 {
            (disk_used as f64 / disk_total as f64) * 100.0
        } else {
            0.0
        },
        cpu_cores,
        load_average: load_avg,
    }
}

/// Get memory information from /proc/meminfo (Linux)
fn get_memory_info() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
            let mut total = 0;
            let mut available = 0;

            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        total = value.parse::<u64>().unwrap_or(0) * 1024; // Convert from KB to bytes
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        available = value.parse::<u64>().unwrap_or(0) * 1024;
                    }
                }
            }

            let used = total.saturating_sub(available);
            return (used / 1024 / 1024, total / 1024 / 1024); // Convert to MB
        }
    }

    // Fallback for non-Linux or if /proc/meminfo is not available
    (0, 0)
}

/// Get disk space information
fn get_disk_info() -> (u64, u64) {
    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let path = CString::new("/").expect("path is valid");
        let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();

        unsafe {
            if libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) == 0 {
                let stat = stat.assume_init();
                let total = stat.f_blocks * stat.f_frsize;
                let available = stat.f_bavail * stat.f_frsize;
                let used = total - available;
                return (used, total);
            }
        }
    }

    // Fallback
    (0, 0)
}

/// Get system load average
fn get_load_average() -> LoadAverage {
    #[cfg(unix)]
    {
        let mut loadavg = [0.0f64; 3];
        unsafe {
            if libc::getloadavg(loadavg.as_mut_ptr(), 3) == 3 {
                return LoadAverage {
                    one_min: loadavg[0],
                    five_min: loadavg[1],
                    fifteen_min: loadavg[2],
                };
            }
        }
    }

    // Fallback
    LoadAverage {
        one_min: 0.0,
        five_min: 0.0,
        fifteen_min: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_memory_info() {
        let (used, total) = get_memory_info();
        // On Linux, we should get real values
        #[cfg(target_os = "linux")]
        {
            assert!(total > 0, "Total memory should be > 0");
            assert!(used <= total, "Used memory should be <= total");
        }
    }

    #[test]
    fn test_get_disk_info() {
        let (used, total) = get_disk_info();
        // On Unix, we should get real values
        #[cfg(unix)]
        {
            assert!(total > 0, "Total disk should be > 0");
            assert!(used <= total, "Used disk should be <= total");
        }
    }

    #[test]
    fn test_get_load_average() {
        let load = get_load_average();
        // Load average should be non-negative
        assert!(load.one_min >= 0.0);
        assert!(load.five_min >= 0.0);
        assert!(load.fifteen_min >= 0.0);
    }

    #[test]
    fn test_system_info() {
        let info = get_system_info();
        assert!(info.cpu_cores > 0, "Should detect at least 1 CPU core");
    }

    #[test]
    fn test_system_info_memory_percent_calculation() {
        let info = get_system_info();
        // On Linux, memory_total_mb should be > 0 and percent should be 0-100
        if info.memory_total_mb > 0 {
            assert!(info.memory_usage_percent >= 0.0);
            assert!(info.memory_usage_percent <= 100.0);
            // Verify calculation: used/total * 100
            let expected = (info.memory_used_mb as f64 / info.memory_total_mb as f64) * 100.0;
            assert!((info.memory_usage_percent - expected).abs() < 0.01);
        }
    }

    #[test]
    fn test_system_info_disk_percent_calculation() {
        let info = get_system_info();
        // On Unix, disk_total_gb should be > 0
        if info.disk_total_gb > 0.0 {
            assert!(info.disk_usage_percent >= 0.0);
            assert!(info.disk_usage_percent <= 100.0);
        }
    }

    #[test]
    fn test_system_info_memory_fields_consistent() {
        let info = get_system_info();
        // used_mb should not exceed total_mb
        assert!(info.memory_used_mb <= info.memory_total_mb);
        // disk_used_gb should not exceed disk_total_gb
        assert!(info.disk_used_gb <= info.disk_total_gb);
    }

    #[test]
    fn test_load_average_three_fields() {
        let load = get_load_average();
        // All three fields should be non-negative
        assert!(load.one_min >= 0.0);
        assert!(load.five_min >= 0.0);
        assert!(load.fifteen_min >= 0.0);
        // On a running system, at least one field should be > 0
        // (unless the system is completely idle, which is rare)
    }

    #[test]
    fn test_load_average_struct_debug() {
        let load = get_load_average();
        let debug_str = format!("{:?}", load);
        assert!(debug_str.contains("LoadAverage"));
        assert!(debug_str.contains("one_min"));
        assert!(debug_str.contains("five_min"));
        assert!(debug_str.contains("fifteen_min"));
    }

    #[test]
    fn test_system_info_struct_debug() {
        let info = get_system_info();
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("SystemInfo"));
        assert!(debug_str.contains("memory_used_mb"));
        assert!(debug_str.contains("disk_used_gb"));
        assert!(debug_str.contains("cpu_cores"));
    }
}

#[cfg(test)]
mod handler_tests {
    use super::*;
    use crate::AppState;
    use axum::extract::State;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn test_state() -> AppState {
        let config = kias_common::config::KiasConfig::default();
        let graph = kias_knowledge::graph::KnowledgeGraph::new();
        let embedding_engine =
            Arc::new(kias_knowledge::vector::LocalEmbeddingEngine::default_dim());
        let knowledge_retriever =
            kias_knowledge::vector::VectorRetriever::new(graph, embedding_engine)
                .await
                .expect("Failed to create knowledge retriever");

        AppState {
            config: Arc::new(config),
            agent_repository: None,
            agents: Arc::new(RwLock::new(HashMap::new())),
            nodes: Arc::new(RwLock::new(HashMap::new())),
            workflows: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(kias_common::audit::MemoryAuditLog::new()),
            sqlite_audit_log: None,
            dead_letter_queue: None,
            event_bus: crate::websocket::EventBus::default(),
            a2a_tasks: crate::handlers::a2a::A2aTaskStore::new(),
            connection_registry: crate::websocket::ConnectionRegistry::default(),
            event_replay_buffer: crate::websocket::EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
            ingested_docs: Arc::new(RwLock::new(Vec::new())),
            context_manager: None,
            tier_routing: crate::handlers::tier_routing::TierRoutingState::new(),
            gxp_auth: crate::handlers::auth_gxp::create_gxp_auth_state(
                kias_common::gxp_auth::PasswordPolicy::default(),
            ),
            jwt_config: crate::auth::JwtConfig::new(
                "kias-default-jwt-secret-change-me",
                "kias",
                24,
            ),
        }
    }

    #[tokio::test]
    async fn test_liveness_returns_ok() {
        let result = liveness().await;
        assert_eq!(result["status"], "ok");
    }

    #[tokio::test]
    async fn test_readiness_returns_healthy() {
        let state = test_state().await;
        let result = readiness(State(state)).await;
        assert_eq!(result.status, "healthy");
        assert_eq!(result.components.len(), 2);
        assert!(result.components.iter().all(|c| c.status == "healthy"));
    }

    #[tokio::test]
    async fn test_deep_health_returns_system_info() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        assert_eq!(result.status, "healthy");
        assert!(result.uptime_secs < 60); // just started
        assert!(result.system.memory_total_mb > 0);
        assert!(result.system.cpu_cores > 0);
        assert!(result.system.load_average.one_min >= 0.0);
    }

    #[tokio::test]
    async fn test_deep_health_components_include_all_stores() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"agents_store"));
        assert!(names.contains(&"nodes_store"));
        assert!(names.contains(&"workflows_store"));
        assert!(names.contains(&"event_bus"));
    }

    #[tokio::test]
    async fn test_deep_health_has_seven_components() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        // agents_store, nodes_store, workflows_store, event_bus, memory, disk, load_average
        assert_eq!(
            result.components.len(),
            7,
            "Deep health should have 7 components"
        );
    }

    #[tokio::test]
    async fn test_deep_health_version_matches_cargo() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        assert_eq!(result.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_deep_health_system_fields_valid() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        // System info should have valid fields
        assert!(result.system.cpu_cores > 0);
        assert!(result.system.memory_total_mb > 0);
        assert!(result.system.memory_usage_percent >= 0.0);
        assert!(result.system.memory_usage_percent <= 100.0);
        assert!(result.system.disk_total_gb > 0.0);
        assert!(result.system.disk_usage_percent >= 0.0);
        assert!(result.system.disk_usage_percent <= 100.0);
    }

    #[tokio::test]
    async fn test_deep_health_load_average_valid() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        assert!(result.system.load_average.one_min >= 0.0);
        assert!(result.system.load_average.five_min >= 0.0);
        assert!(result.system.load_average.fifteen_min >= 0.0);
    }

    #[tokio::test]
    async fn test_deep_health_component_names_include_system() {
        let state = test_state().await;
        let result = deep_health(State(state)).await;
        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"memory"), "Should include memory component");
        assert!(names.contains(&"disk"), "Should include disk component");
        assert!(
            names.contains(&"load_average"),
            "Should include load_average component"
        );
    }

    #[tokio::test]
    async fn test_readiness_version_matches_cargo() {
        let state = test_state().await;
        let result = readiness(State(state)).await;
        assert_eq!(result.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_readiness_component_names() {
        let state = test_state().await;
        let result = readiness(State(state)).await;
        let names: Vec<&str> = result.components.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"agents_store"));
        assert!(names.contains(&"nodes_store"));
    }
}
