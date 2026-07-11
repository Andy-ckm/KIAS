# Capability Maturity and Support Matrix

This document separates implemented code from the product contract. A crate's presence in the repository does not make it stable, secure, enabled by default, or supported for production deployment.

## Maturity levels

| Level | Meaning | Minimum evidence |
|---|---|---|
| Labs | Research or demonstration capability | Builds in CI; explicit limitations; disabled by default |
| Tested | Deterministic tests cover normal and important failure paths | Unit/component tests and documented configuration |
| Integrated | Behavior is verified across its public boundary | API or cross-crate tests, authorization checks, failure tests |
| Hardened | Suitable for controlled production evaluation | Threat model, misuse cases, limits, recovery, observability, upgrade guidance |
| Audited | Independently reviewed | Published assessment and tracked remediation |

No KIAS capability is currently classified as independently audited.

## Product tiers

### Core control plane

Core is the intended long-term compatibility and security boundary.

| Crate | Role | Current level | Promotion work |
|---|---|---:|---|
| `common` | shared types, configuration, errors, masking, audit primitives | Integrated | reduce legacy surface; stabilize public types |
| `controller` | desired/observed state, health, reconciliation and recovery | Tested | durable failure-injection tests |
| `scheduler` | workload placement and policy-aware scheduling | Tested | fairness, overload and adversarial-input tests |
| `workflow-engine` | bounded DAG execution, retries and checkpoints | Tested | crash/restart and migration tests |
| `autonomy-controller` | authorization of autonomous actions and budgets | Tested | cross-boundary misuse tests |
| `executor` | controlled task execution interfaces | Tested | cancellation, timeout and isolation conformance |
| `tool-executor` | tool policy and invocation boundary | Tested | sandbox and egress policy tests |
| `agent-runtime` | managed runtime primitives | Tested | lifecycle integration and resource-limit evidence |
| `data-store` | persistence, audit, cache and recovery storage | Integrated | migrations, backup/restore and corruption tests |
| `data-governance` | governance evidence and policy records | Tested | privacy lifecycle and tamper tests |
| `monitor` | metrics and telemetry primitives | Tested | cardinality, overload and redaction tests |
| `model-router` | bounded provider-routing interface | Tested | fallback, budget and error-normalization tests |
| `compliance-security` | reusable authentication and security primitives | Tested | external cryptographic review and misuse tests |
| `api-server` | authenticated control-plane API | Integrated | remove Labs dependency; tenant-isolation tests |
| `kias-main` | composition root and service lifecycle | Prototype | feature-gate optional services; real health checks |
| `kias-cli` | operator client | Prototype | stable API compatibility and end-to-end tests |

### Supported extensions

Extensions are useful but not part of the minimum control plane. They must remain optional and may have a shorter compatibility window before 1.0.

| Crate | Role | Current level | Boundary requirement |
|---|---|---:|---|
| `cache` | optional cache strategies | Tested | no correctness dependency on cache availability |
| `knowledge` | optional knowledge representation | Tested | no raw sensitive-data retention by default |
| `skills` | reusable tool/skill registration | Tested | policy enforcement remains in Core |
| `team-engine` | worker/verifier collaboration | Tested | no implicit privilege inheritance |
| `langgraph-engine` | state-graph orchestration | Tested | versioned state and checkpoint contract |
| `mcp-protocol` | tool-protocol integration | Tested | conformance, origin and capability checks |
| `a2a-registry` | agent discovery and inter-agent routing | Prototype | authenticated discovery and task authorization |
| `harness-registry` | optional evaluation/runner registry | Prototype | signed metadata and sandbox boundary |
| `document-management` | optional document workflows | Prototype | content limits, malware handling and retention policy |
| `llm-engine` | optional inference abstraction | Prototype | provider-neutral contracts and budget limits |
| `agent-view` | operator-facing inspection tooling | Prototype | read authorization and privacy-aware output |

### Labs

Labs are excluded from the stable product promise and should be disabled by default.

| Crate | Reason for Labs classification | Decision before 1.0 |
|---|---|---|
| `goal-engine` | open-ended autonomous loops increase control risk | feature-gate; promote only with bounded objectives and approval controls |
| `auto-loop` | self-modifying/development loop is not required by the control plane | remove from API default dependencies; keep experimental or split repository |
| `data-aggregator` | unrelated public/social data collection expands privacy scope | split into an optional project or remove |
| `im-integration` | platform-specific message and identity data; conformance incomplete | keep disabled until signed-request and retention tests pass per adapter |
| `it-change-management` | broad domain workflow outside minimum agent control plane | move to example/extension or separate repository |
| `linux-automation` | generic infrastructure automation creates a large privileged surface | move to extension with strict sandboxing or separate repository |
| `gxp-compliance` | industry-specific mapping creates product and organization fingerprint | move to a neutral optional profile or private downstream extension |

## Default-build policy

- The repository CI validates the entire workspace.
- The default developer build should target Core rather than every experiment.
- Extensions and Labs must use explicit Cargo features or separate binaries.
- Disabling an optional capability must not weaken authentication, audit, policy or recovery behavior.
- A dependency from Core to Labs is a release blocker.

## Capability promotion checklist

Promotion from Labs to Extensions or from Extensions to Core requires:

- a documented primary user and job to be done;
- a stable, minimal interface;
- deterministic success and failure tests;
- authorization, privacy and abuse-case analysis;
- explicit limits, timeouts and resource behavior;
- fail-closed behavior when security configuration is incomplete;
- migration, rollback and deprecation strategy;
- an accountable maintainer;
- evidence linked from `docs/project-status.md`.

## Quarterly pruning review

At least once per quarter, maintainers should review:

- crates with no external or Core consumers;
- duplicate orchestration abstractions;
- stale integrations and provider-specific code;
- heavy or vulnerable transitive dependencies;
- unused APIs and `allow(dead_code)` exceptions;
- features without owners, tests or documented users.

The expected outcome may be promotion, repair, feature-gating, extraction to another repository, or deletion. Keeping code forever is not a compatibility strategy.