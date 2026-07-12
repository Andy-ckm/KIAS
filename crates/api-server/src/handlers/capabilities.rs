use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::surfaces::SurfaceConfig;
use crate::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct CapabilityDescriptor {
    pub id: &'static str,
    pub label: &'static str,
    pub tier: &'static str,
    pub enabled: bool,
    pub support: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProductCapabilities {
    pub product: &'static str,
    pub version: &'static str,
    pub profile: &'static str,
    pub contract: [&'static str; 3],
    pub capabilities: Vec<CapabilityDescriptor>,
}

/// Return the effective product contract for this running instance.
///
/// Clients use this response to avoid presenting disabled or experimental
/// surfaces as if they were part of the stable control plane.
pub async fn get_capabilities(State(state): State<AppState>) -> Json<ProductCapabilities> {
    let surfaces = SurfaceConfig::from_env(&state.config);

    Json(ProductCapabilities {
        product: "KIAS Agent Operations Control Plane",
        version: env!("CARGO_PKG_VERSION"),
        profile: surfaces.profile(),
        contract: ["control", "evidence", "recovery"],
        capabilities: vec![
            capability(
                "fleet",
                "Agent fleet and lifecycle",
                "core",
                true,
                "supported",
            ),
            capability(
                "scheduling",
                "Policy-aware scheduling",
                "core",
                true,
                "supported",
            ),
            capability(
                "workflows",
                "Bounded workflow execution",
                "core",
                true,
                "supported",
            ),
            capability(
                "evidence",
                "Audit and operational evidence",
                "core",
                true,
                "supported",
            ),
            capability(
                "recovery",
                "Failure recovery primitives",
                "core",
                true,
                "supported",
            ),
            capability(
                "knowledge",
                "Knowledge retrieval",
                "extension",
                surfaces.knowledge,
                "optional",
            ),
            capability(
                "context",
                "Conversation context management",
                "extension",
                surfaces.context,
                "optional",
            ),
            capability(
                "a2a",
                "Agent-to-agent protocol",
                "extension",
                surfaces.a2a,
                "optional",
            ),
            capability(
                "tier-routing",
                "Experimental tier routing",
                "extension",
                surfaces.tier_routing,
                "optional",
            ),
            capability(
                "realtime-events",
                "Realtime event stream",
                "extension",
                surfaces.realtime,
                "pre-1.0 opt-in",
            ),
            capability(
                "natural-language-commands",
                "Natural-language command surface",
                "labs",
                surfaces.nl_commands,
                "experimental",
            ),
            capability(
                "instant-messaging",
                "Messaging adapters",
                "labs",
                surfaces.im,
                "experimental",
            ),
            capability(
                "visualization",
                "Industry-oriented visualization",
                "labs",
                surfaces.visualization,
                "experimental",
            ),
        ],
    })
}

fn capability(
    id: &'static str,
    label: &'static str,
    tier: &'static str,
    enabled: bool,
    support: &'static str,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id,
        label,
        tier,
        enabled,
        support,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_instance_reports_core_profile() {
        let state = AppState::new(kias_common::config::KiasConfig::default()).await;
        let response = get_capabilities(State(state)).await;

        assert_eq!(response.profile, "core");
        assert_eq!(response.contract, ["control", "evidence", "recovery"]);
        assert!(response
            .capabilities
            .iter()
            .filter(|capability| capability.tier == "labs")
            .all(|capability| !capability.enabled));
    }
}
