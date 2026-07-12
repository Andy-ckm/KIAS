# Project Status and Readiness

KIAS is a pre-1.0 open-source project under active development. This page distinguishes implemented code from verified behavior and from production-readiness claims.

## Maturity statement

| Area | Current status | Evidence expected |
|---|---|---|
| Product scope | Defined | `PRODUCT.md`, product strategy and capability maturity matrix |
| Runtime product profiles | Implemented, under verification | Core-only default routes and authenticated capability discovery |
| Architecture boundaries | Enforced in CI | Core default surface; no Core dependency on Labs packages |
| Workspace build | Gated in CI | `cargo check --workspace --all-features --locked` |
| Unit and integration tests | Gated in CI | `cargo test --workspace --all-features --locked` |
| Rust lint and formatting | Gated in CI | Core Clippy with warnings denied; rustfmt check |
| Dashboard | Gated in CI | authenticated connection gate, dependency install, lint and production build |
| Static security analysis | Enabled | CodeQL results in repository Security tab |
| Dependency updates and audit | Enabled | Dependabot and RustSec advisory workflow |
| Repository security posture | Enabled | OpenSSF Scorecard workflow |
| Secret, PII, and organization scanning | Enabled | masked read-only CI scan with private denylist support |
| Sandboxed Agent Run lifecycle | Implemented and runtime-smoke verified | policy admission, Docker isolation, logs, evidence, retry, cancellation, restart interruption and recovery |
| Release provenance | Partially implemented | checksums, attestations and verification instructions require release validation |
| Independent security audit | Not completed | third-party report and tracked remediation |
| Multi-tenant isolation | Experimental | adversarial end-to-end tests required |
| Production support commitment | Not offered | documented support and regular release practice required |

A green CI run demonstrates that configured checks passed for a revision. It is not proof that the system is vulnerability-free or suitable for every deployment.

## Product boundary

KIAS is a self-hosted Agent Operations Control Plane for teams operating tool-using AI agents. The intended users are AI platform engineers, security and governance engineers, SRE/operations teams, and architects evaluating transparent agent-control infrastructure.

The stable product direction is organized around three outcomes:

- **Control** — identity, tools, autonomy, budgets, rate limits and resources require explicit decisions;
- **Evidence** — important behavior produces privacy-aware operational and audit evidence;
- **Recovery** — work has bounded retries, cancellation, checkpoints, reconciliation and graceful shutdown.

The product loop is `Register → Constrain → Run → Observe → Intervene → Prove`. KIAS is not intended to be a hosted model service, model-training platform, no-code chatbot builder, generic operating-system automation suite, public-data collection platform, or repository for organization-specific workflows and compliance mappings.

See [`../PRODUCT.md`](../PRODUCT.md), [`product-strategy.md`](product-strategy.md), and [`capability-maturity.md`](capability-maturity.md).

## Core, Extensions, and Labs

The workspace and runtime now expose three explicit maturity tiers:

- **Core** is the default Cargo and API surface and the long-term security/compatibility boundary;
- **Extensions** provide optional integrations and higher-level capabilities through explicit runtime opt-ins;
- **Labs** contains disabled-by-default research with no compatibility promise.

A machine-enforced architecture check rejects Core dependencies on Labs packages. Optional routes are absent by default rather than merely hidden. The authenticated `/api/v1/system/capabilities` endpoint reports the effective instance profile and surface maturity.

The former monolithic API router has been removed. The Core-first router no longer mounts knowledge, context, A2A, advanced routing, natural-language commands, messaging adapters, realtime events or industry-oriented visualization unless the corresponding surface switch is enabled.

The full workspace is still built and tested in CI so optional and experimental code cannot silently decay, but its presence is not presented as a production commitment.

## Security and privacy hardening completed in the current change set

- removed tracked local development state, research queues, orchestration state, API-key queue data, and unnecessary binary/reference material;
- replaced full-URI logging with path-only logging;
- removed raw provider error bodies from application errors;
- added redacted `Debug` and serialization behavior for credentials and secret values;
- changed password storage to salted Argon2id hashes;
- implemented standards-compatible TOTP generation;
- stored recovery codes as hashes and consumed them once;
- pseudonymized authentication audit subjects;
- disabled raw webhook payload retention by default;
- implemented signed-request validation for a supported adapter and made incomplete adapters fail closed;
- changed secret scan findings to masked values and short fingerprints;
- removed repository-wide fixed JWT and TLS demonstration fallbacks;
- made the shipped listener loopback-only and authentication-enabled;
- refused unauthenticated public listeners and misleading native-TLS configuration;
- removed wildcard CORS from the canonical product router;
- moved deep health and WebSocket statistics behind authentication;
- removed silent durable-storage fallback to volatile memory;
- excluded raw Agent Run input from durable metadata; only its SHA-256 digest and byte count are persisted;
- removed self-modifying CI workflows after bounded one-time remediation tasks;
- added architecture, static-analysis, Scorecard, dependency, CI and privacy gates with pinned workflow actions.

## Product and engineering changes completed in the current change set

- defined a Core-only default Cargo build while retaining full-workspace CI;
- reduced the process composition root to resources it actually owns;
- replaced unconditional construction claims and false-positive health language with explicit process readiness;
- removed synthetic nodes from normal startup; fixtures now require `KIAS_DEV_FIXTURES=true`;
- removed the configuration PATCH route that reported acceptance without mutating authoritative state;
- added explicit Core, Extension and Labs runtime switches;
- added an observable instance capability contract;
- added `kias token` for short-lived Viewer, Operator and Admin JWT issuance;
- added a Dashboard operator-token gate using tab-scoped session storage;
- updated Dashboard language from scheduler-centric to Agent Operations Control Plane;
- added deterministic startup validation for authentication, token length, JWT lifetime, scheduler settings, controller timing, storage mode and TLS configuration;
- added a durable Agent Run model with policy decisions, constraints, lineage, evidence, checkpoints, bounded retries, cancellation and restart recovery;
- added a hardened Docker CLI sandbox with no network, a read-only root filesystem, dropped capabilities, no-new-privileges, a non-root user and resource limits;
- added a reproducible runtime smoke script that exercises the complete Agent Run lifecycle and directly checks that raw input is absent from SQLite;
- refreshed and locked Rust dependencies;
- added concise failure artifacts for Rust, Dashboard, architecture and privacy checks;
- documented the product wedge, ideal customer, first user journey, flagship workspaces, priority sequence and success measures.

## Verification status for this pull request

The pull request remains Draft until its current maintainer-authored head revision passes all configured read-only checks:

- architecture boundary check;
- Core tests and Clippy;
- complete-workspace build and tests;
- Rust formatting;
- Dashboard lint and production build;
- authenticated native and Docker Compose runtime smoke tests;
- privacy and secret scan;
- CodeQL analysis;
- Rust dependency audit.

The native runtime smoke has verified successful and failed executions, automatic retries, operator retries, cancellation, restart interruption, recovery, sandbox evidence, resource reporting, and the absence of raw Run input in SQLite. The same behavior must remain green on the final reviewed revision.

## Known pre-1.0 limitations

- the Core Docker executor currently runs in the control-plane process and should move behind a separately permissioned Runner service before production use;
- the current durable store and reconciliation model are single-node oriented; high availability, leader election and distributed recovery are not implemented;
- replay recovery re-executes an admitted AgentSpec after the caller resupplies matching input; it is not a memory, process or filesystem snapshot;
- image allowlisting exists, but signature verification, SBOM policy and digest-only production enforcement are not yet complete;
- the current `AgentSpec` lacks stable owner/service identity, environment, policy-set and risk-tier fields;
- policy simulation, human approval and explainable deny decisions are not yet a complete flagship workflow;
- the Dashboard does not yet provide a complete Run/evidence/recovery operator workspace for every API capability;
- authenticated browser WebSocket transport is incomplete, so realtime events remain an explicit pre-1.0 opt-in;
- native TLS is not wired into the `kias` server binary, so deployments must use a trusted TLS-terminating proxy and explicit acknowledgement for non-loopback listeners;
- multi-tenant storage and authorization isolation has not completed adversarial end-to-end verification;
- not every external integration has completed signature, replay, retention and conformance testing;
- Core still constructs some optional subsystem state internally even when the corresponding routes are disabled; dependency extraction remains desirable;
- release provenance and reproducibility require validation on an actual tagged release;
- no independent security audit has been completed.

## Release blockers for a production-oriented 1.0

- separate the control plane from a least-privilege Runner service and define its authenticated protocol;
- add high-availability coordination, reconciliation ownership and tested backup/restore behavior;
- add stable agent ownership, environment, policy and risk metadata;
- complete image digest, signature, provenance and SBOM admission policy;
- implement policy simulation and a bounded human-approval queue;
- complete the Dashboard Run, evidence and recovery workspace;
- complete end-to-end tenant isolation and object-authorization tests;
- complete signature and replay validation for every enabled integration;
- separate optional subsystem construction and heavy dependencies from the Core process;
- complete external security review and remediate findings;
- document performance, failure, recovery, backup, restore, migration and upgrade tests;
- publish compatibility and deprecation policy backed by release practice;
- validate reproducible release artifacts, provenance, SBOM and verification instructions;
- establish maintainer succession and regular security-response practice.

## Repository-administrator actions outside the code change

The following actions cannot be completed safely by a normal source-code pull request and remain mandatory:

- rotate every API key, bot token, webhook secret, signing key and other credential ever used on a machine working with this repository;
- inspect provider usage logs for unexpected calls or source addresses;
- enable repository secret scanning and push protection where available;
- configure branch/ruleset protection so required checks and review cannot be bypassed casually;
- configure the private `ORGANIZATION_DENYLIST_B64` secret with employer, customer, partner, domain, project-code and internal-system aliases;
- perform a reviewed full-history rewrite for previously tracked internal-state paths and sensitive blobs, force-push all branches/tags, and require collaborators to re-clone;
- inspect forks, mirrors, releases, Actions artifacts, caches and old clones that may retain pre-rewrite objects;
- consider moving the project to a neutral organization account if personal repository ownership itself reveals unwanted identity context.

History rewriting never substitutes for credential rotation.

## Evidence policy

README and release claims must be supported by one or more of:

- a stable automated test;
- a reproducible benchmark with environment and methodology;
- a public design document and implementation link;
- a release artifact and verification command;
- an independent assessment.

Avoid vanity metrics, unsupported superlatives, undated provider comparisons and statements such as “production-grade” when the corresponding evidence is absent.

## Privacy and organizational independence

The public repository must not contain personal data, employer/customer/partner identifiers, enterprise domains, tenant IDs, internal systems, private project names, production logs or real credentials. Examples use synthetic data and reserved domains. The project is general-purpose and does not claim endorsement by any employer or customer.
