#!/bin/bash
API="https://api.minimaxi.com/v1/chat/completions"
KEY="sk-cp-xjItK0Upvlc8wfPRpGmPdmlzlNJV-g2-NMrThy9ftkIP-6HmKK7-b8D2VrM_rGQzg4iFYh9wnCm96Oil2aI7jsy2BXu0Lj_taGWh2HGycosXMEpLDRA-tc8"
LOG="/workspace/kias/scripts/minimax_wave3.txt"
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

cat > /tmp/tasks3.txt << 'TASKS'
/workspace/kias/crates/scheduler/src/rolling_update.rs|Rewrite RollingUpdate: zero-downtime rolling update with health checks between batches, automatic rollback on failure, progress tracking, connection draining. 400+ lines 8 tests.
/workspace/kias/crates/scheduler/src/auto_scaling.rs|Rewrite AutoScaling: load-driven scaling with cooldown period, min/max instances, scale-up/down policies, metrics collection. 400+ lines 8 tests.
/workspace/kias/crates/common/src/consistency_matrix.rs|ConsistencyMatrix: configure strong/eventual/compensation consistency per operation, transaction coordinator, retry with compensation. 300+ lines 6 tests.
/workspace/kias/crates/common/src/fault_injection_enhanced.rs|FaultInjector: inject network jitter, node crash, slow disk, slow query, random exceptions. Configurable probability and duration. 300+ lines 6 tests.
/workspace/kias/crates/common/src/idempotency_enhanced.rs|IdempotencyManager: request deduplication with idempotency keys, TTL, storage backend, collision handling. 300+ lines 6 tests.
/workspace/kias/crates/common/src/resilience_enhanced.rs|ResilienceManager: combine circuit breaker + retry + bulkhead + timeout into unified resilience decorator. 300+ lines 6 tests.
/workspace/kias/crates/compliance-security/src/policy_simulator.rs|PolicySimulator: replay historical traffic against new policy, compare old vs new decisions, impact analysis. 300+ lines 6 tests.
/workspace/kias/crates/compliance-security/src/red_team.rs|RedTeamSuite: prompt injection tests, tool abuse tests, data exfiltration tests, supply chain attack tests. 300+ lines 6 tests.
/workspace/kias/crates/compliance-security/src/trust_evaluator.rs|TrustEvaluator: evaluate model output trustworthiness, fact checking, citation completeness, conflict detection, hallucination grading. 300+ lines 6 tests.
/workspace/kias/crates/model-router/src/cost_optimizer.rs|CostOptimizer: multi-objective optimization (latency/cost/hit-rate/SLA), Pareto frontier, recommendation engine. 300+ lines 6 tests.
/workspace/kias/crates/model-router/src/batch_processor.rs|BatchProcessor: batch LLM requests for throughput, dynamic batching by timeout and size, priority lanes. 300+ lines 6 tests.
/workspace/kias/crates/data-aggregator/src/cost_panel_enhanced.rs|EnhancedCostPanel: per-request cost breakdown, per-tool cost, per-tenant cost, per-policy cost, time series charts. 300+ lines 6 tests.
/workspace/kias/crates/data-governance/src/data_bridge_enhanced.rs|EnhancedDataBridge: unified bridge interface for Kafka/Postgres/S3/GCP/Azure, auto-discovery, schema evolution. 300+ lines 6 tests.
/workspace/kias/crates/common/src/dynamic_config_enhanced.rs|EnhancedDynamicConfig: hierarchical config (global/tenant/project), inheritance, override validation, audit trail. 300+ lines 6 tests.
/workspace/kias/crates/compliance-security/src/auth_backends_enhanced.rs|EnhancedAuthBackends: LDAP bind + OAuth2 PKCE + mTLS client cert + Kerberos SPNEGO + SCRAM-SHA-256, unified interface. 400+ lines 8 tests.
/workspace/kias/crates/common/src/observability_std.rs|ObservabilityStd: unified trace_id propagation, span naming conventions, metric naming, structured log schema. 300+ lines 6 tests.
/workspace/kias/crates/compliance-security/src/data_masking.rs|DataMasking: detect and mask PII in logs/outputs, configurable rules, audit who viewed what. 300+ lines 6 tests.
/workspace/kias/crates/common/src/rollback_manager.rs|RollbackManager: blue-green deployment, canary deployment, shadow traffic, automatic rollback on error threshold. 300+ lines 6 tests.
/workspace/kias/crates/monitor/src/capacity_model.rs|CapacityModel: single-tenant/multi-tenant/burst scenarios, resource forecasting, cost projection. 300+ lines 6 tests.
/workspace/kias/crates/common/src/baseline_manager.rs|BaselineManager: manage performance/quality/strategy baselines, detect regression, auto-update baselines. 300+ lines 6 tests.
TASKS

echo "🔥 Wave 3: $(wc -l < /tmp/tasks3.txt) requests with 8 workers..."
cat /tmp/tasks3.txt | xargs -P 8 -I {} bash -c '
  IFS="|" read -r path prompt <<< "{}"
  call_api "$path" "$prompt"
'
echo "=== WAVE 3 DONE ==="
wc -l "$LOG"
