# KIAS Architecture

KIAS is a self-hosted control plane for operating tool-using AI agents under explicit lifecycle, policy, audit, and recovery boundaries.

This document describes the intended architecture. It distinguishes the stable Core from optional Extensions and experimental Labs; repository presence alone does not imply production readiness.

## Architectural outcomes

The architecture is optimized for three outcomes:

- **Control:** an agent cannot obtain identity, tools, autonomy, budget, or resources without an explicit decision.
- **Evidence:** important decisions and state transitions produce privacy-aware operational and audit evidence.
- **Recovery:** managed work can be cancelled, reconciled, retried, resumed, or contained after failure.

## System context

```text
                  ┌──────────────────────────────┐
                  │ Operators and platform APIs │
                  └──────────────┬───────────────┘
                                 │ authenticated requests
                                 ▼
┌──────────────────────────────────────────────────────────────────┐
│                         KIAS control plane                       │
│                                                                  │
│  API boundary ─► identity/policy ─► desired state ─► scheduler   │
│                         │                │              │          │
│                         │                ▼              ▼          │
│                         │         reconciler       runtime/tool    │
│                         │                │          execution      │
│                         ▼                ▼              │          │
│                    audit evidence ◄── state/events ◄────┘          │
│                         │                                         │
│                         ▼                                         │
│              metrics, health and recovery state                  │
└─────────────────────────┬────────────────────────────────────────┘
                          │ explicitly configured adapters
                          ▼
        ┌────────────────────────────────────────────────┐
        │ External identity, model, tool, storage and    │
        │ observability systems                          │
        └────────────────────────────────────────────────┘
```

KIAS does not replace external identity providers, secret managers, network policy, backup systems, SIEMs, or incident-response processes. It integrates with them through explicit boundaries.

## Product tiers

### Core

Core owns the minimum control-plane contract:

- authenticated management interfaces;
- desired and observed agent state;
- scheduling and resource decisions;
- workflow execution, cancellation and checkpoints;
- tool, role, autonomy, rate and budget policies;
- persistence, audit, health, metrics and recovery primitives;
- normalized model-routing interfaces.

Core must remain useful when every optional integration is disabled.

### Extensions

Extensions provide optional orchestration, protocol, cache, knowledge, document or user-interface capabilities. They consume Core interfaces and must not bypass Core policy or audit decisions.

### Labs

Labs contains experimental autonomous loops, broad automation domains and incomplete integrations. Labs is disabled by default and carries no stability promise. See [`capability-maturity.md`](capability-maturity.md).

## Logical layers

```text
L4  Composition and operator surfaces
    kias-main · kias-cli · dashboard

L3  Control-plane interfaces
    api-server · protocol adapters

L2  Domain services
    controller · scheduler · workflow-engine · autonomy-controller
    executor · tool-executor · agent-runtime · model-router

L1  State, evidence and platform services
    data-store · data-governance · monitor · compliance-security

L0  Shared contracts
    common types · configuration · errors · masking · audit primitives
```

### Dependency rules

- dependencies flow downward;
- Core may depend on Core, not on Labs;
- Extensions depend on published Core interfaces;
- Labs cannot become a transitive requirement of the default Core build;
- API handlers contain transport logic, not domain decisions;
- authentication, authorization and data minimization occur before side effects;
- adapters translate external formats but do not own control-plane policy;
- persistence implementations do not decide authorization;
- audit generation is part of a state-changing use case, not an afterthought.

A Core-to-Labs dependency is a release blocker. Architecture checks should enforce these rules mechanically as the feature-gating refactor progresses.

## Core request path

```text
1. Receive request
2. Assign request/correlation identifier
3. Authenticate caller
4. Resolve tenant and subject scope
5. Validate input and size limits
6. Authorize action and target resource
7. Evaluate policy, autonomy, budget and rate limits
8. Persist the intended state or command atomically
9. Emit a pseudonymous audit event
10. Reconcile desired state with runtime state
11. Expose status, metrics and bounded error information
```

Sensitive request bodies, credentials, tokens, query strings and raw external payloads are excluded from normal application logs.

## Agent lifecycle

An agent is modeled as a managed resource with explicit desired and observed state.

```text
Declared ─► Pending ─► Scheduled ─► Starting ─► Running
    │            │          │           │          │
    │            └──────────┴───────────┴──────────┤
    │                                               ▼
    └────────────────────────────────────────► Degraded
                                                    │
                           retry / reconcile ◄──────┤
                                                    ▼
                                               Failed
                                                    │
                                      stop/delete ──┘
```

Required invariants:

- state transitions are validated;
- retries are bounded and observable;
- cancellation is idempotent;
- desired state survives process restarts where durability is promised;
- reconciliation does not silently widen permissions;
- terminal failures enter a dead-letter or operator-review path;
- health reflects real dependencies rather than successful construction alone.

## Scheduling boundary

The scheduler receives a constrained placement request rather than arbitrary agent code. Inputs may include:

- required capabilities and resource limits;
- tenant and policy constraints;
- workload priority and deadlines;
- current capacity and health;
- optional cache-affinity hints.

The scheduler returns a placement decision and explanation. It must not:

- grant a capability absent from the request;
- ignore tenant or policy constraints to improve utilization;
- treat optional cache state as a correctness dependency;
- retry indefinitely under overload.

Future hardening includes fairness, admission control, backpressure and adversarial scheduling tests.

## Workflow and tool execution

The workflow engine coordinates bounded steps; the tool-execution boundary performs side effects.

```text
Workflow state
    │
    ├─► route / condition / fan-out
    │
    ├─► policy decision
    │       ├─ deny ─► auditable failure
    │       ├─ require approval ─► suspended checkpoint
    │       └─ allow
    │
    └─► isolated tool execution
            ├─ timeout
            ├─ cancellation
            ├─ resource and egress limits
            ├─ normalized output
            └─ result / failure evidence
```

Tool output is untrusted input. It must be size-limited, validated and prevented from implicitly changing authorization context.

## Identity and authorization

Authentication establishes a subject; authorization decides whether that subject may perform an action on a resource within a scope.

Design requirements:

- no static default administrative credential;
- secrets are provided at runtime through environment or external providers;
- credential values have redacted diagnostics and serialization;
- direct personal identifiers are avoided in ordinary logs and audit views;
- audit subjects use keyed pseudonyms when direct identity is unnecessary;
- tenant context is explicit and included in every storage and cache key;
- authorization is checked on both collection and object-level operations;
- service identities and human identities are distinguishable;
- high-impact actions can require human approval.

Multi-tenant isolation is not considered hardened until end-to-end cross-tenant misuse tests pass.

## Data classification and retention

KIAS data falls into four broad classes:

| Class | Examples | Default treatment |
|---|---|---|
| Credentials | passwords, API keys, tokens, private keys, recovery codes | never logged; encrypted/externalized; shortest lifetime |
| Personal/confidential content | prompts, messages, documents, identity claims, locations | do not retain by default; explicit purpose and retention required |
| Operational state | desired state, checkpoints, health, resource usage | persist only as needed for recovery and operations |
| Audit evidence | pseudonymous subject, action, resource, outcome, policy decision | integrity-protected, access-controlled, retention documented |

Raw webhook bodies and provider error bodies are not normal telemetry. Adapters extract only fields required for the configured use case.

## Persistence and consistency

Core state-changing operations should use an atomic boundary that records:

- the accepted command or desired-state change;
- the resulting resource version;
- an audit event or durable event reference;
- an idempotency key where clients may retry.

Required behaviors before 1.0:

- schema migrations are versioned and tested;
- backup and restore are documented;
- corruption and partial-write behavior is tested;
- retries do not duplicate irreversible effects;
- cache loss does not lose authoritative state;
- recovery after process termination is deterministic.

## Observability

KIAS separates operational telemetry from security audit evidence.

### Operational telemetry

- request duration and outcome;
- queue depth, saturation and retry counts;
- reconciliation and workflow latency;
- resource usage and health;
- provider and tool error categories without raw response bodies.

### Audit evidence

- pseudonymous actor or service subject;
- scoped action and target resource;
- policy decision and outcome;
- timestamp and correlation identifier;
- approval or override reference where applicable.

Metrics must avoid unbounded labels. Logs must avoid credentials, raw prompts, messages, files, query strings and direct identifiers unless a documented incident workflow temporarily enables controlled capture.

## Integration adapter contract

An external adapter must:

1. validate origin, signature and replay window where the platform supports them;
2. fail closed when validation is incomplete or configuration is missing;
3. parse bounded input without panics;
4. retain only required normalized fields;
5. call Core policy and identity interfaces rather than bypassing them;
6. normalize provider errors without returning raw confidential bodies;
7. provide deterministic fixtures and failure-path tests;
8. document permissions, data flow, retention and revocation.

An adapter that cannot meet this contract remains in Labs and is disabled by default.

## Composition and feature flags

The long-term composition model is:

```text
kias-core binary
  ├─ required Core services
  ├─ optional Extension features
  └─ no Labs features by default

separate experimental binaries or examples
  └─ explicitly selected Labs capabilities
```

`kias-main` is currently a composition root under refactoring. Optional services that are always constructed, and health checks that only report successful construction, are tracked as pre-1.0 gaps.

## Deployment trust boundaries

A production-oriented deployment should separate:

- public ingress from the management API;
- human/operator identity from agent/service identity;
- control-plane state from untrusted tool workloads;
- secret storage from repository and application configuration;
- tenant data and encryption contexts;
- audit storage from mutable application logs;
- network egress for tools from control-plane network access;
- build/release identities from runtime identities.

KIAS cannot create these boundaries merely by being installed; the deployment must configure and verify them.

## Failure model

The system assumes:

- external providers become unavailable or return malformed data;
- tools hang, exceed limits or produce hostile output;
- processes terminate between state transitions;
- duplicate and reordered requests occur;
- storage may be slow, full or partially unavailable;
- credentials are revoked during active work;
- operators make configuration mistakes;
- a tenant attempts to reference another tenant's resources.

Design reviews and tests should begin with these failure cases, not only the happy path.

## Architecture decision policy

Significant changes require an Architecture Decision Record covering:

- context and user problem;
- alternatives considered;
- security and privacy impact;
- compatibility and migration;
- operational failure behavior;
- dependency and binary-size impact;
- evidence and rollback plan.

See [`../PRODUCT.md`](../PRODUCT.md), [`capability-maturity.md`](capability-maturity.md), [`threat-model.md`](threat-model.md), and [`project-status.md`](project-status.md).