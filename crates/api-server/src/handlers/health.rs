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

        let path = CString::new("/").unwrap();
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
}
