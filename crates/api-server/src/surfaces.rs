//! Runtime product-surface selection.
//!
//! KIAS ships a focused Core control plane. Optional Extensions and Labs
//! capabilities are mounted only when operators opt in explicitly. Repository
//! presence is never treated as runtime enablement.

use kias_common::config::KiasConfig;

/// Runtime-visible product surfaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SurfaceConfig {
    pub knowledge: bool,
    pub context: bool,
    pub a2a: bool,
    pub tier_routing: bool,
    /// Browser-compatible authenticated streaming is not complete yet, so the
    /// WebSocket surface remains an explicit opt-in during pre-1.0.
    pub realtime: bool,
    /// The current direct invocation path reaches a shell executor. It remains
    /// Labs-only until sandbox, egress and side-effect guarantees are verified.
    pub direct_execution: bool,
    pub nl_commands: bool,
    pub im: bool,
    pub visualization: bool,
    pub dev_fixtures: bool,
}

impl SurfaceConfig {
    /// Resolve product surfaces from safe defaults and explicit environment
    /// opt-ins. Existing `[knowledge].enabled` remains the canonical switch for
    /// the knowledge extension during the pre-1.0 compatibility window.
    pub fn from_env(config: &KiasConfig) -> Self {
        Self {
            knowledge: config.knowledge.enabled || env_flag("KIAS_SURFACES__KNOWLEDGE"),
            context: env_flag("KIAS_SURFACES__CONTEXT"),
            a2a: env_flag("KIAS_SURFACES__A2A"),
            tier_routing: env_flag("KIAS_SURFACES__TIER_ROUTING"),
            realtime: env_flag("KIAS_SURFACES__REALTIME"),
            direct_execution: env_flag("KIAS_SURFACES__DIRECT_EXECUTION"),
            nl_commands: env_flag("KIAS_SURFACES__NL_COMMANDS"),
            im: env_flag("KIAS_SURFACES__IM"),
            visualization: env_flag("KIAS_SURFACES__VISUALIZATION"),
            dev_fixtures: env_flag("KIAS_DEV_FIXTURES"),
        }
    }

    /// The instance profile is observable to operators and clients.
    pub fn profile(self) -> &'static str {
        if self.direct_execution || self.nl_commands || self.im || self.visualization {
            "labs-enabled"
        } else if self.knowledge || self.context || self.a2a || self.tier_routing || self.realtime {
            "core-with-extensions"
        } else {
            "core"
        }
    }
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_surface_is_core_only() {
        let config = KiasConfig::default();
        let surfaces = SurfaceConfig::from_env(&config);

        assert!(!surfaces.knowledge);
        assert!(!surfaces.context);
        assert!(!surfaces.a2a);
        assert!(!surfaces.tier_routing);
        assert!(!surfaces.realtime);
        assert!(!surfaces.direct_execution);
        assert!(!surfaces.nl_commands);
        assert!(!surfaces.im);
        assert!(!surfaces.visualization);
        assert_eq!(surfaces.profile(), "core");
    }

    #[test]
    fn knowledge_config_promotes_instance_to_extension_profile() {
        let mut config = KiasConfig::default();
        config.knowledge.enabled = true;
        let surfaces = SurfaceConfig::from_env(&config);

        assert!(surfaces.knowledge);
        assert_eq!(surfaces.profile(), "core-with-extensions");
    }
}
