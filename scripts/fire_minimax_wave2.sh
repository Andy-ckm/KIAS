#!/bin/bash
# Second wave - enhance existing code + generate tests + docs
API="https://api.minimaxi.com/v1/chat/completions"
KEY="sk-cp-xjItK0Upvlc8wfPRpGmPdmlzlNJV-g2-NMrThy9ftkIP-6HmKK7-b8D2VrM_rGQzg4iFYh9wnCm96Oil2aI7jsy2BXu0Lj_taGWh2HGycosXMEpLDRA-tc8"
LOG="/workspace/kias/scripts/minimax_wave2.txt"
> "$LOG"

call_api() {
  local path="$1" prompt="$2"
  resp=$(curl -s --max-time 120 -X POST "$API" \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "{\"model\":\"MiniMax-M2.7\",\"messages\":[{\"role\":\"user\",\"content\":$(echo "$prompt" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))')}],\"max_tokens\":8000,\"temperature\":0.3}")
  
  echo "$resp" | python3 -c "
import sys,json
try:
    r=json.load(sys.stdin)
    c=r['choices'][0]['message']['content']
    if '\`\`\`' in c: c=c.split('\`\`\`')[1]
    if '\`\`\`' in c: c=c.split('\`\`\`')[0]
    c=c.strip()
    if c.startswith('rust'): c=c[4:]
    if len(c)>50:
        open('$path','w').write(c)
        print(f'OK:{len(c)} $path')
    else: print(f'EMPTY $path')
except Exception as e:
    print(f'ERR:{e} $path')
" 2>&1
}

export -f call_api
export API KEY LOG

# Wave 2: Tests, integration, enhancement for each crate
cat > /tmp/tasks2.txt << 'TASKS'
/workspace/kias/crates/common/tests/integration_tests.rs|Write comprehensive integration tests for common crate: test CircuitBreaker state transitions, test ConcurrencyControl AIMD, test SchemaValidator with complex schemas, test UnsNamespace tree operations, test DynamicConfig hot reload. 20+ tests.
/workspace/kias/crates/compliance-security/tests/integration_tests.rs|Write integration tests for compliance-security: test AccountabilityGraph full causal chain, test GxpAuditChain tamper detection, test ComplianceGate PII detection accuracy, test RBAC+ABAC combined evaluation, test BYOK encrypt/decrypt roundtrip. 20+ tests.
/workspace/kias/crates/monitor/tests/integration_tests.rs|Write integration tests for monitor: test HealthMonitor state transitions, test AnomalyDetector with known anomalies, test LatencyGovernor p99 accuracy, test RegressionGate baseline comparison, test ObservabilityExporter Prometheus format. 20+ tests.
/workspace/kias/crates/data-governance/tests/integration_tests.rs|Write integration tests for data-governance: test CostTracker multi-agent allocation, test TenantQuota enforcement, test TenantIsolation cross-tenant blocking, test DataResidency compliance checking, test KafkaBridge produce/consume. 20+ tests.
/workspace/kias/crates/workflow-engine/tests/integration_tests.rs|Write integration tests for workflow-engine: test DurableFlow checkpoint/recover, test EnhancedHITL approval chain, test SessionBuffer flush/compact, test FlowDurable state machine transitions. 20+ tests.
/workspace/kias/crates/team-engine/tests/integration_tests.rs|Write integration tests for team-engine: test TaskPlanner DAG decomposition, test AdversarialValidator pipeline, test MemoryGovernance consolidation, test SelfEvalLoop auto-fix, test MemoryConflictResolver merge. 20+ tests.
/workspace/kias/crates/a2a-registry/tests/integration_tests.rs|Write integration tests for a2a-registry: test AgentRegistry full lifecycle, test A2ARegistry event subscription, test EnhancedA2A capability matching, test heartbeat expiry. 15+ tests.
/workspace/kias/crates/scheduler/tests/agent_scheduler_tests.rs|Write tests for AgentScheduler: test priority scheduling, test affinity rules, test resource-aware placement, test preemption, test concurrent scheduling. 15+ tests.
/workspace/kias/crates/model-router/tests/smart_router_tests.rs|Write tests for SmartRouter: test cost-based routing, test quality-based routing, test fallback chain, test circuit breaking, test latency-aware routing. 15+ tests.
/workspace/kias/crates/cache/tests/tiered_cache_tests.rs|Write tests for TieredCache: test LRU eviction, test TTL expiration, test multi-level get/put, test cache stats, test invalidation. 15+ tests.
/workspace/kias/crates/controller/tests/federation_tests.rs|Write tests for FederationManager and ClusterLink: test region registration, test state sync, test failover, test split-brain detection. 15+ tests.
/workspace/kias/crates/data-store/tests/storage_tests.rs|Write tests for DurableStorage and TieredStorage: test put/get persistence, test snapshot/compact, test hot/warm/cold promotion, test recovery. 15+ tests.
/workspace/kias/crates/executor/tests/sandbox_tests.rs|Write tests for Sandbox: test create/destroy lifecycle, test seccomp filter application, test cgroup limits, test resource monitoring, test isolation. 10+ tests.
/workspace/kias/crates/knowledge/tests/industry_pack_tests.rs|Write tests for IndustryPack: test load finance pack, test load healthcare pack, test customize, test list available, test template rendering. 10+ tests.
/workspace/kias/crates/autonomy-controller/tests/autonomy_tests.rs|Write tests for AutonomyMode: test Suggest mode, test Auto mode, test Full mode, test trust level upgrade/downgrade, test safety net triggers. 10+ tests.
/workspace/kias/docs/ARCHITECTURE-V2.md|Write comprehensive architecture documentation for AgentGuard v2.0: system overview, crate dependency graph, data flow diagrams (ASCII), security model, deployment topology, API reference summary. 500+ lines.
/workspace/kias/docs/SECURITY-MODEL.md|Write security model documentation: mTLS flow, RBAC/ABAC model, secrets management, PKI chain, GxP audit trail, EU AI Act compliance mapping. 300+ lines.
/workspace/kias/docs/PERFORMANCE-GUIDE.md|Write performance guide: caching strategy, concurrency model, latency targets, cost optimization, capacity planning, benchmark methodology. 300+ lines.
/workspace/kias/docs/MULTI-TENANT-GUIDE.md|Write multi-tenant guide: tenant isolation model, quota management, SSO integration, data residency, cross-region federation. 300+ lines.
/workspace/kias/docs/DEPLOYMENT-GUIDE.md|Write deployment guide: Docker setup, K8s manifests, monitoring setup, backup/restore, rolling upgrades, disaster recovery. 300+ lines.
TASKS

echo "🔥 Wave 2: $(wc -l < /tmp/tasks2.txt) requests with 8 workers..."
cat /tmp/tasks2.txt | xargs -P 8 -I {} bash -c '
  IFS="|" read -r path prompt <<< "{}"
  call_api "$path" "$prompt"
'

echo "=== WAVE 2 DONE ==="
wc -l "$LOG"
