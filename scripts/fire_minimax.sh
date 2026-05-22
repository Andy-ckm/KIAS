#!/bin/bash
# Fire MiniMax API requests in parallel - 4 req/sec = 1200 in 5min
API="https://api.minimaxi.com/v1/chat/completions"
KEY="sk-cp-xjItK0Upvlc8wfPRpGmPdmlzlNJV-g2-NMrThy9ftkIP-6HmKK7-b8D2VrM_rGQzg4iFYh9wnCm96Oil2aI7jsy2BXu0Lj_taGWh2HGycosXMEpLDRA-tc8"
DIR="/workspace/kias/crates"
LOG="/workspace/kias/scripts/minimax_log.txt"
> "$LOG"

call_api() {
  local path="$1" crate="$2" desc="$3"
  local prompt="Write a COMPLETE Rust implementation file at path: $path

Description: $desc

REQUIREMENTS:
- Import error types: use crate::error::{KiasError}; (or crate-specific error)
- Every pub fn must have /// doc comments  
- NO unwrap/expect/panic in non-test code - use Result<T, KiasError>
- Use tracing::info!/warn! not println!
- End file with #[cfg(test)] mod tests { use super::*; ... } containing at least 6 #[test] functions
- Write REAL implementation logic (structs, methods, algorithms), NOT stubs
- 300+ lines minimum

Output ONLY the Rust code between triple backticks. No explanation."

  resp=$(curl -s --max-time 120 -X POST "$API" \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"MiniMax-M2.7\",\"messages\":[{\"role\":\"user\",\"content\":$(echo "$prompt" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))')}],\"max_tokens\":8000,\"temperature\":0.3}")
  
  result=$(echo "$resp" | python3 -c "
import sys,json
try:
    r=json.load(sys.stdin)
    c=r['choices'][0]['message']['content']
    if '\`\`\`' in c: c=c.split('\`\`\`')[1]
    if '\`\`\`' in c: c=c.split('\`\`\`')[0]
    c=c.strip()
    if c.startswith('rust'): c=c[4:]
    if len(c) > 50:
        import os
        os.makedirs(os.path.dirname('$path'), exist_ok=True)
        open('$path','w').write(c)
        print(f'OK:{len(c)}')
    else:
        print('EMPTY')
except Exception as e:
    print(f'ERR:{e}')
" 2>&1)
  echo "$(date +%H:%M:%S) $result $path" >> "$LOG"
}

export -f call_api
export API KEY DIR LOG

# Generate all 66 tasks
cat > /tmp/tasks.txt << 'TASKS'
common|circuit_breaker.rs|CircuitBreaker三态Closed/Open/HalfOpen, failure counting, timeout reset, half-open probe
common|concurrency_control.rs|AdaptiveConcurrency AIMD(additive increase multiplicative decrease), TokenBucket rate limiter with refill, semaphore
common|schema_validation.rs|SchemaValidator: register JSON schemas, validate data, batch validation, custom rules engine
common|uns_namespace.rs|UnsNamespace hierarchical topic tree (MQTT-style): create namespace, register topic, resolve path, validate hierarchy
common|dynamic_config.rs|DynamicConfig: hot-reload with file watcher, version tracking, rollback, schema validation, notify listeners
common|plugin_lifecycle.rs|PluginLifecycle: register/enable/disable/unload plugins, dependency resolution, health check, state machine
common|manifest.rs|Manifest: YAML manifest parsing with serde, validation rules, apply to system, diff computation
common|dep_ast_validator.rs|DepAstValidator: parse Cargo.toml deps, check for cycles in dependency graph, version compatibility matrix
common|test_pyramid.rs|TestPyramid: register tests by level(unit/contract/integration/e2e/chaos), coverage by level, balance check
common|coverage_gate.rs|CoverageGate: set threshold per crate, check coverage from tarpaulin output, gate pass/fail, report
common|change_audit.rs|ChangeAudit: record git changes, generate audit report with risk scoring, verify compliance checklist
common|property_test.rs|PropertyTest: define properties with invariants, fuzz testing runner, check invariants hold, report violations
compliance-security|accountability.rs|AccountabilityGraph: directed graph with nodes(Decision/Action/Evidence) and edges(caused_by/triggered_by), path query, report generation
compliance-security|gxp_audit.rs|GxpAuditChain: SHA256 hash chain, electronic signatures, tamper detection, ALCOA+ compliance verification
compliance-security|compliance_gate.rs|ComplianceGate: PII scanning with regex(SSN/CC/email/name), sensitive operation approval workflow, unauthorized access blocking
compliance-security|rbac_abac.rs|PermissionEngine: RBAC role hierarchy with permission matrix + ABAC attribute-based policies, evaluate(), grant(), revoke()
compliance-security|byok_kms.rs|KmsIntegration: register BYOK keys, encrypt/decrypt(AES-256 simulation), key rotation, audit trail
compliance-security|secrets_vault.rs|SecretsVault: encrypted secret storage, get/store/rotate, access audit log, TTL expiration
compliance-security|digital_signature.rs|DigitalSignature: sign data, verify signatures, revoke keys, non-repudiation check, X.509 cert management
compliance-security|eu_ai_act_v2.rs|EuAiActEngine: classify AI system risk level, generate Annex IV technical documentation, compliance checklist
compliance-security|autonomy_modes.rs|AutonomyController: Suggest/Auto/Full modes, trust level tracking, dynamic upgrade/downgrade based on success rate
compliance-security|mtls.rs|MtlsManager: generate self-signed certs, verify peer certs, cert rotation, CA chain validation
compliance-security|supply_chain.rs|SupplyChainSecurity: SBOM generation from Cargo.toml, artifact signing, vulnerability scanning, dependency audit
compliance-security|runtime_protection.rs|RuntimeProtection: detect prompt injection patterns, block malicious tool calls, rate limiting per source
compliance-security|pen_test.rs|PenTestSuite: SQL injection tests, auth bypass tests, XSS tests, privilege escalation tests, generate report
compliance-security|compliance_as_service.rs|ComplianceAsService: generate compliance reports, schedule audits, export to PDF, track findings
compliance-security|security_audit_report.rs|SecurityAuditReport: collect security metrics, generate report with trends, compare periods, risk scoring
monitor|health_monitor.rs|HealthMonitor: 4-layer health model(Liveness/Readiness/Degraded/Draining), heartbeat tracking, exponential backoff recovery
monitor|anomaly_detector.rs|AnomalyDetector: Z-score outlier detection, moving average, cost spike threshold, error rate monitoring, alert generation
monitor|latency_governor.rs|LatencyGovernor: sliding window p95/p99 calculation, degradation trigger, recovery detection, slow node isolation
monitor|regression_gate.rs|RegressionGate: benchmark runner, baseline comparison, gate pass/fail on regression, performance curve tracking
monitor|observability.rs|ObservabilityExporter: export metrics in Prometheus format, trace spans, log aggregation, OTel compatibility
data-governance|cost_attribution.rs|CostTracker: record per-Agent per-Task token usage, cost allocation, budget alerts, cost reports by dimension
data-governance|tenant_quota.rs|TenantQuota: set quota(QPS/Token/Storage/Calls), check usage, charge, remaining, quota inheritance
data-governance|data_residency.rs|DataResidency: set region, check compliance, schedule deletion, verify deletion, retention policies
data-governance|multi_tenant.rs|TenantIsolation: create tenant, namespace isolation, resource limits, cross-tenant blocking, tenant config
data-governance|sla_product.rs|SlaProduct: define SLA tiers, check compliance, breach alert, compensation tracking, uptime calculation
data-governance|data_bridge_kafka.rs|KafkaBridge: produce messages, consume with consumer groups, schema registry integration, error handling
data-governance|data_bridge_db.rs|DbBridge: connection pooling, query builder, stream changes via CDC, transaction support
data-governance|data_bridge_s3.rs|S3Bridge: upload/download objects, list with prefix, presigned URLs, multipart upload
a2a-registry|agent_registry.rs|AgentRegistry: register agents with capabilities, discover by capability, heartbeat tracking, deregister, TTL expiry
a2a-registry|a2a_registry.rs|A2ARegistry: register agents, lookup by name/capability, subscribe to events(unregister/update), health monitoring
a2a-registry|a2a_enhanced.rs|EnhancedA2A: capability matching algorithm, protocol negotiation, session management, message routing
scheduler|agent_scheduler.rs|AgentScheduler: schedule tasks with affinity rules, priority queue, resource-aware placement, preemption
executor|sandbox.rs|Sandbox: create isolated sandbox, apply seccomp filters, set cgroup limits, destroy, resource monitoring
model-router|smart_router.rs|SmartRouter: route by cost/quality scoring, model fallback chain, latency-aware routing, circuit breaking
model-router|model_agent.rs|ModelRoutingAgent: analyze task complexity, select optimal model, route with fallback, track performance
cache|tiered_cache.rs|TieredCache: multi-level LRU+TTL cache, get/put/invalidate per layer, stats, eviction policy
data-aggregator|cost_panel.rs|CostPanel: add metrics, breakdown by agent/model/task/tenant, time series, summary stats
data-store|durable_storage.rs|DurableStorage: put/get with persistence, snapshots, compaction, WAL, recovery
data-store|tiered_storage.rs|TieredStorage: hot/warm/cold storage tiers, auto-promote on access, auto-demote on age, size limits
controller|federation.rs|FederationManager: register regions, sync state across regions, cross-region routing, failover
controller|cluster_link.rs|ClusterLink: connect to peer clusters, sync state, failover, split-brain detection
team-engine|task_planner.rs|TaskPlanner: decompose complex tasks into DAG, plan execution order, parallel scheduling, step tracking
team-engine|adversarial_validation.rs|AdversarialValidator: Worker/Verifier/Critic/Judge pipeline, challenge responses, certify quality
team-engine|memory_governance.rs|MemoryGovernance: store/retrieve memories, consolidation, expiration, conflict resolution, trust scoring
team-engine|self_eval_loop.rs|SelfEvalLoop: evaluate output quality, identify failure patterns, auto-fix strategies, regression verification
team-engine|memory_enhanced.rs|EnhancedMemory: semantic storage, vector similarity search, context window management, memory compression
team-engine|memory_conflict.rs|MemoryConflictResolver: detect conflicting memories, resolve by recency/trust/source, merge strategies
workflow-engine|flow_durable.rs|DurableFlow: persist workflow state, checkpoint/recover, state machine transitions, resume from failure
workflow-engine|hitl_enhanced.rs|EnhancedHITL: multi-level approval chain, timeout handling, auto-escalation, approval delegation
workflow-engine|session_buffer.rs|SessionBuffer: push messages, flush to storage, compact old entries, drain with LRU eviction
knowledge|industry_pack.rs|IndustryPack: load industry-specific templates(finance/healthcare/manufacturing), customize, list available
compliance-security|auth_backends.rs|AuthBackend: LDAP/OAuth2/mTLS/Kerberos/SCRAM authentication backends, register backend, authenticate user
autonomy-controller|autonomy_modes_v2.rs|AutonomyModeV2: trust score tracking, gradual autonomy increase/decrease, safety net triggers, mode history
TASKS

# Fire all tasks - 8 concurrent workers
echo "🔥 Firing $(wc -l < /tmp/tasks.txt) MiniMax API requests with 8 workers..."
cat /tmp/tasks.txt | xargs -P 8 -I {} bash -c '
  IFS="|" read -r crate file desc <<< "{}"
  call_api "'$DIR'/${crate}/src/${file}" "$crate" "$desc"
'

echo "=== ALL DONE ==="
wc -l "$LOG"
grep -c "^.*OK:" "$LOG" && echo "successful" || echo "0 successful"
grep -c "^.*ERR:" "$LOG" && echo "errors" || echo "0 errors"
