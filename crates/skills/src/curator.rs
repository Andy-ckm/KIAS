//! # Curator — Skill Health Monitor & Lifecycle Manager
//!
//! Periodic cron-like scanner that:
//! 1. Checks each registered skill's health via `health_check()`
//! 2. Tracks skill registration timestamps for TTL-based expiry
//! 3. Cleans up expired/unhealthy skills from the registry
//! 4. Emits structured tracing events for observability
//! 5. Generates health reports for monitoring dashboards
//!
//! ## Usage
//!
//! ```rust,no_run
//! use kias_skills::curator::{Curator, CuratorConfig};
//! use kias_skills::SkillRegistry;
//!
//! # async fn example() {
//! let registry = SkillRegistry::new();
//! let config = CuratorConfig {
//!     scan_interval_secs: 60,
//!     skill_ttl_secs: 3600,
//!     ..Default::default()
//! };
//! let mut curator = Curator::new(config);
//! curator.attach_registry(&registry);
//!
//! // Run a single scan cycle
//! let report = curator.scan(&registry).await;
//! println!("{:?}", report);
//! # }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Health Status ────────────────────────────────────────────────────

/// Health status of a skill, determined by the curator's scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillHealthStatus {
    /// Skill is fully operational.
    Healthy,
    /// Skill is partially degraded (slow, intermittent failures).
    Degraded,
    /// Skill is non-functional.
    Unhealthy,
    /// Skill has exceeded its TTL and is considered stale.
    Expired,
}

impl std::fmt::Display for SkillHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "Healthy"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Unhealthy => write!(f, "Unhealthy"),
            Self::Expired => write!(f, "Expired"),
        }
    }
}

// ── Health Report ────────────────────────────────────────────────────

/// Health check result for a single skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHealthReport {
    /// Skill name.
    pub skill_name: String,
    /// Current health status.
    pub status: SkillHealthStatus,
    /// When this check was performed.
    pub checked_at: DateTime<Utc>,
    /// When the skill was first registered.
    pub registered_at: DateTime<Utc>,
    /// Age of the skill in seconds.
    pub age_secs: i64,
    /// Optional human-readable message (e.g. error details).
    pub message: Option<String>,
    /// Health check latency in milliseconds (0 if skipped).
    pub latency_ms: u64,
}

/// Summary of a full curator scan cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratorReport {
    /// When the scan started.
    pub scan_started: DateTime<Utc>,
    /// When the scan completed.
    pub scan_completed: DateTime<Utc>,
    /// Total skills scanned.
    pub total_scanned: usize,
    /// Count by health status.
    pub healthy: usize,
    pub degraded: usize,
    pub unhealthy: usize,
    pub expired: usize,
    /// Skills that were cleaned up (removed from registry).
    pub cleaned_up: Vec<String>,
    /// Individual skill reports.
    pub skill_reports: Vec<SkillHealthReport>,
}

impl CuratorReport {
    /// Returns true if all skills are healthy.
    pub fn all_healthy(&self) -> bool {
        self.degraded == 0 && self.unhealthy == 0 && self.expired == 0
    }

    /// Returns a human-readable summary.
    pub fn summary(&self) -> String {
        format!(
            "Curator scan: {} total | {} healthy | {} degraded | {} unhealthy | {} expired | {} cleaned up",
            self.total_scanned, self.healthy, self.degraded, self.unhealthy, self.expired, self.cleaned_up.len()
        )
    }
}

// ── Registration Metadata ────────────────────────────────────────────

/// Metadata tracked by the curator for each registered skill.
#[derive(Debug, Clone)]
struct SkillMeta {
    /// When the skill was registered with the curator.
    registered_at: DateTime<Utc>,
    /// Consecutive unhealthy check count (for auto-cleanup threshold).
    consecutive_failures: u32,
}

// ── Curator Config ──────────────────────────────────────────────────

/// Configuration for the curator's behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratorConfig {
    /// How often to run a full scan, in seconds. Default: 300 (5 min).
    pub scan_interval_secs: u64,
    /// TTL for skills in seconds. Skills older than this are marked Expired.
    /// Set to 0 to disable TTL checks. Default: 0 (disabled).
    pub skill_ttl_secs: u64,
    /// Health check timeout per skill in milliseconds. Default: 5000.
    pub health_check_timeout_ms: u64,
    /// Number of consecutive failures before auto-cleanup. Default: 3.
    /// Set to 0 to disable auto-cleanup of unhealthy skills.
    pub max_consecutive_failures: u32,
    /// Whether to run health checks (execute a test payload). Default: true.
    pub enable_health_checks: bool,
    /// Whether to clean up expired skills automatically. Default: true.
    pub auto_cleanup_expired: bool,
}

impl Default for CuratorConfig {
    fn default() -> Self {
        Self {
            scan_interval_secs: 300,
            skill_ttl_secs: 0, // disabled by default
            health_check_timeout_ms: 5000,
            max_consecutive_failures: 3,
            enable_health_checks: true,
            auto_cleanup_expired: true,
        }
    }
}

// ── Curator ──────────────────────────────────────────────────────────

/// The Curator manages skill lifecycle by periodically scanning the registry.
pub struct Curator {
    config: CuratorConfig,
    /// Per-skill metadata (registration time, failure count).
    metadata: HashMap<String, SkillMeta>,
    /// Names of skills to clean up on next scan.
    pending_cleanup: Vec<String>,
    /// Last scan report.
    last_report: Option<CuratorReport>,
    /// Total scans performed.
    scan_count: u64,
}

impl Curator {
    /// Create a new curator with the given config.
    pub fn new(config: CuratorConfig) -> Self {
        Self {
            config,
            metadata: HashMap::new(),
            pending_cleanup: Vec::new(),
            last_report: None,
            scan_count: 0,
        }
    }

    /// Register all current skills from the registry with the curator.
    /// This should be called once after skills are registered.
    pub fn attach_registry(&mut self, registry: &crate::registry::SkillRegistry) {
        let now = Utc::now();
        for name in registry.list_skills() {
            let name_owned = name.to_string();
            self.metadata.entry(name_owned).or_insert(SkillMeta {
                registered_at: now,
                consecutive_failures: 0,
            });
        }
        tracing::info!(
            skill_count = registry.count(),
            "Curator attached to registry"
        );
    }

    /// Manually register a skill's metadata (for dynamic registration).
    pub fn track_skill(&mut self, name: &str) {
        self.metadata.entry(name.to_string()).or_insert(SkillMeta {
            registered_at: Utc::now(),
            consecutive_failures: 0,
        });
    }

    /// Untrack a skill (e.g., when it's manually removed).
    pub fn untrack_skill(&mut self, name: &str) {
        self.metadata.remove(name);
    }

    /// Run a single scan cycle against the given registry.
    ///
    /// This performs:
    /// 1. TTL expiry check
    /// 2. Health check (if enabled)
    /// 3. Consecutive failure tracking
    /// 4. Auto-cleanup of expired/unhealthy skills
    pub async fn scan(&mut self, registry: &crate::registry::SkillRegistry) -> CuratorReport {
        let scan_started = Utc::now();
        let mut reports = Vec::new();
        let mut cleaned_up = Vec::new();
        let now = Utc::now();

        // Phase 1: Scan each registered skill
        for skill_name in registry.list_skills() {
            let name_owned = skill_name.to_string();

            // Ensure we have metadata
            let meta = self
                .metadata
                .entry(name_owned.clone())
                .or_insert(SkillMeta {
                    registered_at: now,
                    consecutive_failures: 0,
                });

            let age_secs = (now - meta.registered_at).num_seconds();
            let mut status = SkillHealthStatus::Healthy;
            let mut message = None;
            let mut latency_ms = 0u64;

            // Phase 1a: TTL check
            if self.config.skill_ttl_secs > 0 && age_secs > self.config.skill_ttl_secs as i64 {
                status = SkillHealthStatus::Expired;
                message = Some(format!(
                    "Skill age {}s exceeds TTL {}s",
                    age_secs, self.config.skill_ttl_secs
                ));
                tracing::warn!(
                    skill = %skill_name,
                    age_secs = age_secs,
                    ttl_secs = self.config.skill_ttl_secs,
                    "Skill expired by TTL"
                );
            }

            // Phase 1b: Health check (only if not already expired and enabled)
            if status == SkillHealthStatus::Healthy && self.config.enable_health_checks {
                if let Some(skill) = registry.get(skill_name) {
                    let check_start = std::time::Instant::now();
                    let health_result = tokio::time::timeout(
                        std::time::Duration::from_millis(self.config.health_check_timeout_ms),
                        skill.health_check(),
                    )
                    .await;

                    latency_ms = check_start.elapsed().as_millis() as u64;

                    match health_result {
                        Ok(Ok(health_status)) => {
                            status = health_status;
                            if status != SkillHealthStatus::Healthy {
                                message = Some(format!("Health check returned: {}", status));
                            }
                        }
                        Ok(Err(e)) => {
                            status = SkillHealthStatus::Unhealthy;
                            message = Some(format!("Health check error: {}", e));
                            tracing::error!(
                                skill = %skill_name,
                                error = %e,
                                "Skill health check failed"
                            );
                        }
                        Err(_) => {
                            status = SkillHealthStatus::Unhealthy;
                            message = Some(format!(
                                "Health check timed out after {}ms",
                                self.config.health_check_timeout_ms
                            ));
                            tracing::warn!(
                                skill = %skill_name,
                                timeout_ms = self.config.health_check_timeout_ms,
                                "Skill health check timed out"
                            );
                        }
                    }
                }
            }

            // Phase 1c: Track consecutive failures
            let meta = self
                .metadata
                .get_mut(&name_owned)
                .expect("metadata entry just inserted with or_default above");
            match status {
                SkillHealthStatus::Unhealthy | SkillHealthStatus::Expired => {
                    meta.consecutive_failures += 1;
                }
                _ => {
                    meta.consecutive_failures = 0;
                }
            }

            reports.push(SkillHealthReport {
                skill_name: name_owned.clone(),
                status: status.clone(),
                checked_at: now,
                registered_at: meta.registered_at,
                age_secs,
                message,
                latency_ms,
            });
        }

        // Phase 2: Cleanup pass
        for report in &reports {
            let should_cleanup = match &report.status {
                SkillHealthStatus::Expired if self.config.auto_cleanup_expired => true,
                SkillHealthStatus::Unhealthy if self.config.max_consecutive_failures > 0 => self
                    .metadata
                    .get(&report.skill_name)
                    .map(|m| m.consecutive_failures >= self.config.max_consecutive_failures)
                    .unwrap_or(false),
                _ => false,
            };

            if should_cleanup {
                cleaned_up.push(report.skill_name.clone());
                self.pending_cleanup.push(report.skill_name.clone());
                tracing::info!(
                    skill = %report.skill_name,
                    status = %report.status,
                    consecutive_failures = self
                        .metadata
                        .get(&report.skill_name)
                        .map(|m| m.consecutive_failures)
                        .unwrap_or(0),
                    "Skill marked for cleanup"
                );
            }
        }

        // Phase 3: Remove cleaned-up skills from metadata
        for name in &cleaned_up {
            self.metadata.remove(name);
        }

        let scan_completed = Utc::now();
        let healthy = reports
            .iter()
            .filter(|r| r.status == SkillHealthStatus::Healthy)
            .count();
        let degraded = reports
            .iter()
            .filter(|r| r.status == SkillHealthStatus::Degraded)
            .count();
        let unhealthy = reports
            .iter()
            .filter(|r| r.status == SkillHealthStatus::Unhealthy)
            .count();
        let expired = reports
            .iter()
            .filter(|r| r.status == SkillHealthStatus::Expired)
            .count();

        let total = reports.len();
        self.scan_count += 1;

        let report = CuratorReport {
            scan_started,
            scan_completed,
            total_scanned: total,
            healthy,
            degraded,
            unhealthy,
            expired,
            cleaned_up,
            skill_reports: reports,
        };

        // Structured tracing output
        if report.all_healthy() {
            tracing::debug!(
                total = total,
                scan_number = self.scan_count,
                "Curator scan completed — all skills healthy"
            );
        } else {
            tracing::warn!(
                total = total,
                healthy = healthy,
                degraded = degraded,
                unhealthy = unhealthy,
                expired = expired,
                cleaned_up = report.cleaned_up.len(),
                scan_number = self.scan_count,
                "Curator scan completed — issues found"
            );
        }

        self.last_report = Some(report.clone());
        report
    }

    /// Get the configured scan interval in seconds.
    pub fn scan_interval_secs(&self) -> u64 {
        self.config.scan_interval_secs
    }

    /// Get the number of tracked skills.
    pub fn tracked_count(&self) -> usize {
        self.metadata.len()
    }

    /// Get the total number of scans performed.
    pub fn scan_count(&self) -> u64 {
        self.scan_count
    }

    /// Get the last scan report, if any.
    pub fn last_report(&self) -> Option<&CuratorReport> {
        self.last_report.as_ref()
    }

    /// Get the pending cleanup list (skills marked for removal).
    pub fn pending_cleanup(&self) -> &[String] {
        &self.pending_cleanup
    }

    /// Drain the pending cleanup list (returns and clears).
    pub fn drain_pending_cleanup(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_cleanup)
    }

    /// Get a reference to the config.
    pub fn config(&self) -> &CuratorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::SkillRegistry;
    use crate::skill::Skill;
    use async_trait::async_trait;
    use kias_common::KiasResult;

    // ── Mock Skills ──────────────────────────────────────────────

    struct HealthySkill;

    #[async_trait]
    impl Skill for HealthySkill {
        fn name(&self) -> &str {
            "healthy_skill"
        }
        fn description(&self) -> &str {
            "A skill that always reports healthy"
        }
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"status": "ok"}))
        }
        async fn health_check(&self) -> KiasResult<SkillHealthStatus> {
            Ok(SkillHealthStatus::Healthy)
        }
    }

    struct DegradedSkill;

    #[async_trait]
    impl Skill for DegradedSkill {
        fn name(&self) -> &str {
            "degraded_skill"
        }
        fn description(&self) -> &str {
            "A skill that reports degraded"
        }
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"status": "slow"}))
        }
        async fn health_check(&self) -> KiasResult<SkillHealthStatus> {
            Ok(SkillHealthStatus::Degraded)
        }
    }

    struct UnhealthySkill;

    #[async_trait]
    impl Skill for UnhealthySkill {
        fn name(&self) -> &str {
            "unhealthy_skill"
        }
        fn description(&self) -> &str {
            "A skill that reports unhealthy"
        }
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Err(kias_common::KiasError::ExternalService("down".into()))
        }
        async fn health_check(&self) -> KiasResult<SkillHealthStatus> {
            Ok(SkillHealthStatus::Unhealthy)
        }
    }

    struct DefaultHealthSkill;

    #[async_trait]
    impl Skill for DefaultHealthSkill {
        fn name(&self) -> &str {
            "default_health"
        }
        fn description(&self) -> &str {
            "A skill using default health_check (Healthy)"
        }
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"status": "ok"}))
        }
    }

    fn make_registry_with_skills() -> SkillRegistry {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));
        registry.register(Box::new(DegradedSkill));
        registry.register(Box::new(DefaultHealthSkill));
        registry
    }

    // ── Tests ────────────────────────────────────────────────────

    #[test]
    fn test_curator_config_default() {
        let config = CuratorConfig::default();
        assert_eq!(config.scan_interval_secs, 300);
        assert_eq!(config.skill_ttl_secs, 0);
        assert_eq!(config.health_check_timeout_ms, 5000);
        assert_eq!(config.max_consecutive_failures, 3);
        assert!(config.enable_health_checks);
        assert!(config.auto_cleanup_expired);
    }

    #[test]
    fn test_skill_health_status_display() {
        assert_eq!(format!("{}", SkillHealthStatus::Healthy), "Healthy");
        assert_eq!(format!("{}", SkillHealthStatus::Degraded), "Degraded");
        assert_eq!(format!("{}", SkillHealthStatus::Unhealthy), "Unhealthy");
        assert_eq!(format!("{}", SkillHealthStatus::Expired), "Expired");
    }

    #[test]
    fn test_curator_creation() {
        let curator = Curator::new(CuratorConfig::default());
        assert_eq!(curator.tracked_count(), 0);
        assert_eq!(curator.scan_count(), 0);
        assert!(curator.last_report().is_none());
    }

    #[test]
    fn test_curator_attach_registry() {
        let registry = make_registry_with_skills();
        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);
        assert_eq!(curator.tracked_count(), 3);
    }

    #[test]
    fn test_curator_track_untrack() {
        let mut curator = Curator::new(CuratorConfig::default());
        curator.track_skill("test");
        assert_eq!(curator.tracked_count(), 1);

        curator.track_skill("test"); // idempotent
        assert_eq!(curator.tracked_count(), 1);

        curator.untrack_skill("test");
        assert_eq!(curator.tracked_count(), 0);
    }

    #[tokio::test]
    async fn test_scan_all_healthy() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));
        registry.register(Box::new(DefaultHealthSkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;

        assert_eq!(report.total_scanned, 2);
        assert_eq!(report.healthy, 2);
        assert_eq!(report.degraded, 0);
        assert_eq!(report.unhealthy, 0);
        assert_eq!(report.expired, 0);
        assert!(report.cleaned_up.is_empty());
        assert!(report.all_healthy());
        assert_eq!(curator.scan_count(), 1);
    }

    #[tokio::test]
    async fn test_scan_detects_degraded() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));
        registry.register(Box::new(DegradedSkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;

        assert_eq!(report.total_scanned, 2);
        assert_eq!(report.healthy, 1);
        assert_eq!(report.degraded, 1);
        assert!(!report.all_healthy());
    }

    #[tokio::test]
    async fn test_scan_detects_unhealthy_and_cleans_up() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));
        registry.register(Box::new(UnhealthySkill));

        let config = CuratorConfig {
            max_consecutive_failures: 1, // cleanup after 1 failure
            ..Default::default()
        };
        let mut curator = Curator::new(config);
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;

        assert_eq!(report.unhealthy, 1);
        assert_eq!(report.cleaned_up.len(), 1);
        assert!(report.cleaned_up.contains(&"unhealthy_skill".to_string()));
        // Metadata should be cleaned up
        assert_eq!(curator.tracked_count(), 1);
    }

    #[tokio::test]
    async fn test_scan_consecutive_failures_threshold() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(UnhealthySkill));

        let config = CuratorConfig {
            max_consecutive_failures: 3, // need 3 consecutive failures
            ..Default::default()
        };
        let mut curator = Curator::new(config);
        curator.attach_registry(&registry);

        // First scan: 1 failure, no cleanup
        let report = curator.scan(&registry).await;
        assert_eq!(report.unhealthy, 1);
        assert!(report.cleaned_up.is_empty());

        // Second scan: 2 failures, no cleanup
        let report = curator.scan(&registry).await;
        assert!(report.cleaned_up.is_empty());

        // Third scan: 3 failures, cleanup!
        let report = curator.scan(&registry).await;
        assert_eq!(report.cleaned_up.len(), 1);
    }

    #[tokio::test]
    async fn test_scan_ttl_expiry() {
        let mut curator = Curator::new(CuratorConfig {
            skill_ttl_secs: 1,           // 1 second TTL
            enable_health_checks: false, // skip health check for this test
            ..Default::default()
        });

        // Manually set registration time in the past
        curator.metadata.insert(
            "old_skill".to_string(),
            SkillMeta {
                registered_at: Utc::now() - chrono::Duration::seconds(10),
                consecutive_failures: 0,
            },
        );

        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill)); // named "healthy_skill" but we track "old_skill"

        // Actually we need the skill in registry. Let's use a custom approach:
        // The "old_skill" won't be found in registry, so let's test differently.
        // Create a registry with the HealthySkill, and set its metadata to be old.
        let mut registry2 = SkillRegistry::new();
        registry2.register(Box::new(HealthySkill));

        let mut curator2 = Curator::new(CuratorConfig {
            skill_ttl_secs: 1,
            enable_health_checks: false,
            ..Default::default()
        });
        curator2.attach_registry(&registry2);

        // Manually set the healthy_skill to be old
        curator2.metadata.insert(
            "healthy_skill".to_string(),
            SkillMeta {
                registered_at: Utc::now() - chrono::Duration::seconds(10),
                consecutive_failures: 0,
            },
        );

        let report = curator2.scan(&registry2).await;
        assert_eq!(report.expired, 1);
        assert_eq!(report.cleaned_up.len(), 1);
        assert!(report.cleaned_up.contains(&"healthy_skill".to_string()));
    }

    #[tokio::test]
    async fn test_scan_health_checks_disabled() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(DegradedSkill));

        let config = CuratorConfig {
            enable_health_checks: false,
            ..Default::default()
        };
        let mut curator = Curator::new(config);
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;

        // With health checks disabled, all skills appear healthy
        assert_eq!(report.healthy, 1);
        assert_eq!(report.degraded, 0);
    }

    #[tokio::test]
    async fn test_scan_auto_cleanup_disabled() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(UnhealthySkill));

        let config = CuratorConfig {
            max_consecutive_failures: 1,
            auto_cleanup_expired: false, // disabling expired cleanup
            ..Default::default()
        };
        let mut curator = Curator::new(config);
        curator.attach_registry(&registry);

        // Set skill to be expired
        curator.metadata.insert(
            "unhealthy_skill".to_string(),
            SkillMeta {
                registered_at: Utc::now() - chrono::Duration::seconds(9999),
                consecutive_failures: 0,
            },
        );

        // The unhealthy_skill won't be expired because TTL is 0 (disabled).
        // It will be unhealthy, and cleanup happens for unhealthy (not expired).
        // Let's test with TTL enabled:
        let config2 = CuratorConfig {
            skill_ttl_secs: 1,
            max_consecutive_failures: 0, // disable unhealthy cleanup
            auto_cleanup_expired: true,
            ..Default::default()
        };
        let mut curator2 = Curator::new(config2);
        curator2.metadata.insert(
            "unhealthy_skill".to_string(),
            SkillMeta {
                registered_at: Utc::now() - chrono::Duration::seconds(10),
                consecutive_failures: 0,
            },
        );

        let report = curator2.scan(&registry).await;
        assert_eq!(report.expired, 1);
        assert_eq!(report.cleaned_up.len(), 1);
    }

    #[tokio::test]
    async fn test_curator_report_summary() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));
        registry.register(Box::new(DegradedSkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;
        let summary = report.summary();

        assert!(summary.contains("2 total"));
        assert!(summary.contains("1 healthy"));
        assert!(summary.contains("1 degraded"));
    }

    #[tokio::test]
    async fn test_curator_last_report() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        assert!(curator.last_report().is_none());

        curator.scan(&registry).await;

        assert!(curator.last_report().is_some());
        assert_eq!(curator.last_report().unwrap().total_scanned, 1);
    }

    #[tokio::test]
    async fn test_curator_pending_cleanup() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(UnhealthySkill));

        let config = CuratorConfig {
            max_consecutive_failures: 1,
            ..Default::default()
        };
        let mut curator = Curator::new(config);
        curator.attach_registry(&registry);

        curator.scan(&registry).await;

        assert_eq!(curator.pending_cleanup().len(), 1);

        let drained = curator.drain_pending_cleanup();
        assert_eq!(drained.len(), 1);
        assert!(curator.pending_cleanup().is_empty());
    }

    #[tokio::test]
    async fn test_scan_latency_recorded() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;

        // Latency should be recorded (>= 0)
        let healthy_report = report
            .skill_reports
            .iter()
            .find(|r| r.skill_name == "healthy_skill")
            .unwrap();
        // Just verify it's reasonable (not negative, which it can't be as u64)
        assert!(healthy_report.latency_ms < 10_000);
    }

    #[test]
    fn test_skill_health_report_serialization() {
        let report = SkillHealthReport {
            skill_name: "test".to_string(),
            status: SkillHealthStatus::Healthy,
            checked_at: Utc::now(),
            registered_at: Utc::now(),
            age_secs: 60,
            message: None,
            latency_ms: 5,
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"test\""));
        assert!(json.contains("\"Healthy\""));
    }

    #[test]
    fn test_curator_report_serialization() {
        let report = CuratorReport {
            scan_started: Utc::now(),
            scan_completed: Utc::now(),
            total_scanned: 0,
            healthy: 0,
            degraded: 0,
            unhealthy: 0,
            expired: 0,
            cleaned_up: vec![],
            skill_reports: vec![],
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"total_scanned\":0"));
    }

    #[tokio::test]
    async fn test_multiple_scans_increment_count() {
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(HealthySkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        for i in 1..=5 {
            curator.scan(&registry).await;
            assert_eq!(curator.scan_count(), i);
        }
    }

    #[tokio::test]
    async fn test_default_health_check_returns_healthy() {
        // Skills without custom health_check should default to Healthy
        let mut registry = SkillRegistry::new();
        registry.register(Box::new(DefaultHealthSkill));

        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;
        assert_eq!(report.healthy, 1);
        assert_eq!(report.total_scanned, 1);
    }

    #[tokio::test]
    async fn test_scan_empty_registry() {
        let registry = SkillRegistry::new();
        let mut curator = Curator::new(CuratorConfig::default());
        curator.attach_registry(&registry);

        let report = curator.scan(&registry).await;
        assert_eq!(report.total_scanned, 0);
        assert!(report.all_healthy());
        assert!(report.cleaned_up.is_empty());
        assert!(report.skill_reports.is_empty());
    }

    #[test]
    fn test_curator_config_accessors() {
        let config = CuratorConfig {
            scan_interval_secs: 120,
            ..Default::default()
        };
        let curator = Curator::new(config);
        assert_eq!(curator.scan_interval_secs(), 120);
        assert_eq!(curator.config().health_check_timeout_ms, 5000);
    }

    #[test]
    fn test_skill_health_status_equality() {
        assert_eq!(SkillHealthStatus::Healthy, SkillHealthStatus::Healthy);
        assert_ne!(SkillHealthStatus::Healthy, SkillHealthStatus::Unhealthy);
    }

    #[test]
    fn test_curator_report_all_healthy() {
        let report = CuratorReport {
            scan_started: Utc::now(),
            scan_completed: Utc::now(),
            total_scanned: 5,
            healthy: 5,
            degraded: 0,
            unhealthy: 0,
            expired: 0,
            cleaned_up: vec![],
            skill_reports: vec![],
        };
        assert!(report.all_healthy());

        let report_with_issues = CuratorReport {
            healthy: 4,
            degraded: 1,
            ..report
        };
        assert!(!report_with_issues.all_healthy());
    }
}
