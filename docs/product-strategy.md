# KIAS Product Strategy

## Strategic wedge

KIAS is not another agent framework. Its wedge is **agent operations control**: the layer a platform team adopts after an agent prototype exists but before that agent is trusted to perform meaningful work.

The product should answer six operator questions for every managed run:

1. **What is allowed?** Identity, tools, policy, budget and resource boundaries.
2. **What is running?** Desired state, observed state, placement and health.
3. **What happened?** Decisions, side effects, failures and evidence.
4. **Can it be stopped?** Cancellation, containment and approval boundaries.
5. **Can it recover?** Retry, checkpoint, reconciliation and dead-letter behavior.
6. **Can it be explained?** Correlated evidence without unnecessary raw sensitive data.

This is the product loop:

```text
Register → Constrain → Run → Observe → Intervene → Prove
```

A feature that does not improve this loop should normally remain an Extension, stay in Labs, or leave the repository.

## Ideal customer profile

### Primary adopters

KIAS is designed for teams that:

- operate multiple tool-using agents or agentic workflows;
- require self-hosting or transparent control-plane behavior;
- have platform, security or SRE ownership;
- care about failure containment and audit evidence more than visual prompt authoring;
- can operate infrastructure and integrate an external identity, secret and runtime boundary.

Typical first adopters are AI platform teams inside regulated, security-conscious or infrastructure-heavy organizations, plus engineering teams building an internal agent platform.

### Economic buyer

The likely economic buyer is the leader accountable for AI platform reliability, security or governance. The buying trigger is not “we need another chatbot”; it is “agents are beginning to take actions and our existing application telemetry cannot prove or control what they do.”

### Initial champion

The initial champion is usually a platform engineer or security architect who needs a deployable reference control plane and wants to avoid embedding authorization, retries, audit and recovery independently in every agent application.

## Explicitly excluded users

KIAS should not optimize for:

- consumers building personal assistants;
- teams seeking a no-code chatbot editor;
- model-training and GPU-cluster operators;
- users who want a hosted model gateway with no control-plane ownership;
- generic infrastructure automation without an agent-control use case;
- organization-specific compliance workflows in the public Core.

Trying to satisfy these audiences would blur the trust boundary and turn the product into a toolbox rather than an operating system for controlled agents.

## First successful user journey

A new evaluator should be able to complete this journey using only synthetic data:

1. build the default Core surface;
2. generate a runtime JWT signing secret;
3. start a loopback-only authenticated control plane;
4. issue a short-lived Operator token with `kias token`;
5. connect the Dashboard using that token;
6. inspect the instance capability contract;
7. register one synthetic agent with explicit resource limits;
8. observe its lifecycle and scheduling state;
9. cancel or fail the run and inspect recovery evidence;
10. export enough correlated evidence to explain the outcome.

The pre-1.0 product is not considered usable until this path is deterministic, documented and exercised in CI or a synthetic reference deployment.

## Flagship product workspaces

### 1. Fleet

The Fleet workspace is the operational inventory of managed agents. It should show ownership, environment, desired state, observed state, policy attachment, risk tier, runtime placement and last known health.

The current `AgentSpec` is intentionally minimal, but before 1.0 it needs stable fields for owner/service identity, environment, policy set and risk tier. Arbitrary environment variables must not become the product's secret-management mechanism.

### 2. Policy and approvals

Operators need one place to understand why an action was allowed, denied or suspended. The flagship experience is not a generic rules editor; it is an explainable decision and approval queue for high-impact actions.

Required capabilities include:

- policy simulation and dry run;
- human approval for irreversible or privileged actions;
- bounded approval lifetime and scope;
- explicit deny reasons;
- evidence linking the request, policy version, approver and resulting side effect.

### 3. Evidence

Evidence is a correlated view of identity, policy decisions, workflow steps, tool calls, state transitions and outcomes. It should minimize raw prompts, documents and direct identity while still answering who or what acted, under which authority, against which resource and with what result.

The product should converge on a stable run/correlation identifier and interoperable telemetry conventions rather than inventing a new event vocabulary for every subsystem.

### 4. Recovery

Recovery is where KIAS can be meaningfully differentiated from agent SDKs. The product should make retries, checkpoints, cancellation, reconciliation, dead letters and operator intervention visible as one recovery story.

A recovery feature is complete only when failure injection proves that authoritative state survives the promised failure mode and duplicate side effects are prevented.

## Runtime product profiles

KIAS exposes an observable instance profile:

- `core` — supported control-plane surfaces only;
- `core-with-extensions` — one or more optional integrations enabled;
- `labs-enabled` — at least one experimental surface enabled.

The default is `core`. Optional routes are absent, not merely hidden in the Dashboard. Clients discover the effective contract through `/api/v1/system/capabilities`.

### Extension opt-ins

- `KIAS_SURFACES__KNOWLEDGE=true`
- `KIAS_SURFACES__CONTEXT=true`
- `KIAS_SURFACES__A2A=true`
- `KIAS_SURFACES__TIER_ROUTING=true`
- `KIAS_SURFACES__REALTIME=true`

### Labs opt-ins

- `KIAS_SURFACES__NL_COMMANDS=true`
- `KIAS_SURFACES__IM=true`
- `KIAS_SURFACES__VISUALIZATION=true`

Synthetic nodes are available only through `KIAS_DEV_FIXTURES=true` and must never be confused with runtime discovery.

## Product decisions

### Keep and deepen

- agent lifecycle and reconciliation;
- controlled tool execution;
- scheduling and resource admission;
- workflow checkpoints and cancellation;
- audit and operational evidence;
- recovery and dead-letter handling;
- budgets, limits and explainable policy decisions.

### Keep optional

- knowledge retrieval;
- conversation context management;
- A2A interoperability;
- advanced routing;
- realtime browser event streaming until authenticated transport is complete;
- document and protocol adapters.

### Remove from the default product

- public deep diagnostics;
- wildcard CORS;
- synthetic infrastructure state;
- configuration APIs that report success without changing authoritative state;
- industry-specific dashboards and labels;
- incomplete webhook or messaging integrations;
- open-ended autonomous and self-modifying loops.

## Priority sequence

### P0 — Adoption and trust

- deterministic five-minute authenticated quickstart;
- capability-aware Dashboard;
- complete synthetic reference agent journey;
- tenant/object authorization misuse tests;
- truthful health, configuration and runtime state;
- dependency and release gates fully green.

### P1 — Flagship differentiation

- policy simulation and approval queue;
- unified run/evidence model;
- failure-injection and recovery demonstrations;
- tenant-scoped storage, quotas and audit;
- sandbox and egress capability matrix.

### P2 — Ecosystem

- plugin lifecycle contract;
- adapter conformance suite;
- signed metadata and provenance;
- versioned A2A and tool-protocol compatibility;
- reference integrations using synthetic fixtures.

## Success measures

The north-star measure is:

> **Percentage of managed runs with a complete Control–Evidence–Recovery envelope.**

A complete envelope means the run has an authenticated subject, a recorded policy decision, bounded resources, correlated state/tool evidence and a terminal or recoverable outcome.

Supporting measures:

- time from clone to first authenticated synthetic agent;
- percentage of high-impact actions covered by approval or explicit deny policy;
- percentage of state-changing APIs with idempotency and audit evidence;
- recovery success and duplicate-side-effect rate under injected failures;
- median time for an operator to explain a failed run;
- number of enabled adapters passing origin, replay, retention and revocation tests;
- Core dependency size, startup time and binary size;
- real external pilots that can be described without private customer data.

Feature count, crate count and raw lines of code are not product success metrics.
