#!/usr/bin/env python3
"""Apply exact follow-up fixes exposed by the read-only quality preview."""

from pathlib import Path


def replace_exact(path: str, old: str, new: str, count: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise SystemExit(f"{path}: expected {count} matches, found {actual}: {old[:80]!r}")
    target.write_text(text.replace(old, new), encoding="utf-8")


# Protected route tests must use an authenticated application state. The public
# liveness/CORS tests intentionally retain the unauthenticated state.
replace_exact(
    "crates/api-server/src/routes/product.rs",
    '''    async fn core_profile_exposes_basic_probe_and_capability_contract() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());
''',
    '''    async fn core_profile_exposes_basic_probe_and_capability_contract() {
        let app =
            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());
''',
)
replace_exact(
    "crates/api-server/src/routes/product.rs",
    '''    async fn optional_and_labs_routes_are_absent_by_default() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());
''',
    '''    async fn optional_and_labs_routes_are_absent_by_default() {
        let app =
            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());
''',
)
replace_exact(
    "crates/api-server/src/routes/product.rs",
    '''    async fn fake_runtime_config_mutation_is_not_advertised() {
        let app = create_router_with_surfaces(test_state().await, SurfaceConfig::default());
''',
    '''    async fn fake_runtime_config_mutation_is_not_advertised() {
        let app =
            create_router_with_surfaces(authenticated_state().await, SurfaceConfig::default());
''',
)

# Use the process readiness result during startup rather than retaining a
# test-only convenience method.
replace_exact(
    "crates/kias-main/src/main.rs",
    '''    let health = manager.health_check();
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial readiness check");
''',
    '''    let health = manager.health_check();
    if !health.is_healthy() {
        anyhow::bail!("KIAS process resources did not pass the initial readiness check");
    }
    tracing::info!(overall = %health.overall, uptime = health.uptime_secs, "Initial readiness check");
''',
)
replace_exact(
    "crates/kias-main/src/services/init.rs",
    '.any(|token| token.as_bytes().len() < MIN_STATIC_TOKEN_BYTES)',
    '.any(|token| token.len() < MIN_STATIC_TOKEN_BYTES)',
)
replace_exact(
    "crates/kias-main/src/services/init.rs",
    'if secret.as_bytes().len() < MIN_JWT_SECRET_BYTES {',
    'if secret.len() < MIN_JWT_SECRET_BYTES {',
)

# The serialized credential intentionally exposes only type metadata and the
# redaction marker. Test exact sensitive values rather than matching substrings
# that also occur in safe field/type names.
replace_exact(
    "crates/compliance-security/src/auth_providers.rs",
    '''        assert!(json.contains("password"));
        assert!(json.contains("[REDACTED]"));
        assert!(!json.contains("user"));
        assert!(!json.contains("pass"));
''',
    '''        assert!(json.contains("password"));
        assert!(json.contains("[REDACTED]"));
        assert!(!json.contains("\\\"user\\\""));
        assert!(!json.contains("\\\"pass\\\""));
''',
)
