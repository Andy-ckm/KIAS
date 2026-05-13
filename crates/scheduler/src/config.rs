use serde::Deserialize;

/// Scheduler configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    /// Default scheduling algorithm
    #[serde(default = "default_algorithm")]
    pub algorithm: String,

    /// Whether preemption is enabled
    #[serde(default)]
    pub preemption_enabled: bool,

    /// Priority classes
    #[serde(default)]
    pub priority_classes: Vec<PriorityClass>,

    /// Cache-aware scheduling weight (0.0 - 1.0)
    #[serde(default = "default_cache_weight")]
    pub cache_weight: f64,

    /// Max scheduling attempts before fallback
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PriorityClass {
    pub name: String,
    pub value: u32,
}

fn default_algorithm() -> String {
    "round-robin".to_string()
}

fn default_cache_weight() -> f64 {
    0.3
}

fn default_max_attempts() -> u32 {
    3
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            algorithm: default_algorithm(),
            preemption_enabled: false,
            priority_classes: vec![],
            cache_weight: default_cache_weight(),
            max_attempts: default_max_attempts(),
        }
    }
}
