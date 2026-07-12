# Project Status and Readiness

KIAS is a pre-1.0 open-source project under active development. This page distinguishes implemented code from verified behavior and from production-readiness claims.

## Maturity statement

| Area | Current status | Evidence expected |
|---|---|---|
| Product scope | Defined | `PRODUCT.md`, product strategy and capability maturity matrix |
| Runtime product profiles | Verified for Core | Core-only default routes and authenticated capability discovery |
| Sandboxed Agent Run lifecycle | Verified in runtime smoke | admission, execution, logs, resource evidence, cancel, retry, restart recovery and input non-persistence |
| Architecture boundaries | Enforced in CI | Core default surface; no Core dependency on Labs packages |
| Workspace build | Gated in CI | `cargo check --workspace --all-features --locked` |
| Unit and integration tests | Gated in CI | `cargo test --workspace --all-features --locked` |
| Rust lint and formatting | Gated in CI | Core Clippy with warnings denied; rustfmt check |
| Dashboard | Gated in CI | authenticated connection gate, dependency install, lint and production build |
| Static security analysis | Enabled | CodeQL results in repository Security tab |
| Dependency updates and audit | Enabled | Dependabot and RustSec advisory workflow |
| Repository security posture | Enabled | OpenSSF Scorecard workflow |
| Secret, PII, and organization scanning | Enabled | masked read-only CI scan with private denylist support |
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

The workspace and runtime expose three explicit maturity tiers:

- **Core** is the default Cargo and API surface and the long-term security/compatibility boundary;
- **Extensions** provide optional integrations and higher-level capabilities through explicit runtime opt-ins;
- **Labs** contains disabled-by-default research with no compatibility promise.

A machine-enforced architecture check rejects Core dependencies on Labs packages. Optional routes are absent by default rather than merely hidden. The authenticated `/api/v1/system/capabilities` endpoint reports the effective instance profile and whether a Docker-backed Agent Run service is available.

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
- stopped idempotency middleware from persisting request bodies;
- made Agent Run persistence store only input SHA-256 and byte length, never raw input;
- denied AgentSpec environment values in the Core execution path;
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
- added durable Agent and Agent Run state backed by SQLite;
- added policy admission using explicit execution opt-in, image allowlist, bounded command/input, timeout, retry and resource constraints;
- added Docker sandbox execution with no network, read-only root filesystem, dropped capabilities, non-root user, bounded tmpfs, CPU/memory/PID limits and no host mounts;
- added Run status, bounded logs, resource observations, sandbox evidence and SHA-256 evidence envelopes;
- added cancellation, bounded automatic retries, lineage-linked manual retry, interrupted-run detection and replay recovery;
- extracted a locally reproducible runtime smoke script proving the complete lifecycle and verifying raw input is absent from SQLite;
- refreshed and locked Rust dependencies;
- added concise failure artifacts for Rust, Dashboard, architecture, privacy and runtime checks;
- documented the product wedge, ideal customer, first user journey, flagship workspaces, priority sequence and success measures;
- rewrote README around verified startup and Agent Run paths rather than repository feature inventory.

## Verification status for this pull request

The pull request remains Draft until its current maintainer-authored head revision passes all configured checks:

- complete Agent Run runtime smoke;
- Docker Compose API/Dashboard build, restart and persistence smoke;
- architecture boundary check;
- Core tests and Clippy;
- complete-workspace build and tests;
- Rust formatting;
- Dashboard lint and production build;
- privacy and secret scan;
- CodeQL analysis;
- Rust dependency audit.

The native runtime smoke has demonstrated admission, real Docker execution, logs, evidence, failure retries, manual retry, cancellation, restart interruption, replay recovery and raw-input non-persistence. Final acceptance still requires these checks on the final formatted head revision.

Normal CI is read-only. Any bounded mechanical maintenance workflow must use an exact file allowlist and delete itself in its successful commit.

## Known pre-1.0 limitations

- the current runner uses the local Docker CLI and shares the control-plane host trust boundary; a production-oriented deployment needs an independently authenticated Runner service;
- the standard Compose stack intentionally does not mount the host container socket, so it provides the control plane and Dashboard without execution privilege;
- SQLite is the current single-node authority; high availability and distributed transaction semantics are not implemented;
- the current `AgentSpec` lacks stable owner/service identity, environment, policy-set and risk-tier fields;
- object-level and tenant-level authorization are not complete;
- replay recovery restarts the admitted command and verifies the resupplied input digest; it is not a process-memory or filesystem snapshot;
- image signature, digest provenance and SBOM admission are not implemented;
- resource observations are best-effort samples; configured limits and final exit state are authoritative;
- network policy is currently `none`; selective controlled egress is not implemented;
- the Dashboard does not yet expose the complete Run evidence and intervention workflow;
- authenticated browser WebSocket transport is incomplete, so realtime events remain an explicit pre-1.0 opt-in;
- native TLS is not wired into the `kias` server binary, so deployments must use a trusted TLS-terminating proxy and explicit acknowledgement for non-loopback listeners;
- not every external integration has completed signature, replay, retention and conformance testing;
- Core still constructs some optional subsystem state internally even when the corresponding routes are disabled; dependency extraction remains desirable;
- release provenance and reproducibility require validation on an actual tagged release;
- no independent security audit has been completed.

## Release blockers for a production-oriented 1.0

- separate the execution Runner from the API process and mutually authenticate control-plane-to-runner traffic;
- add stable agent ownership, environment, policy and risk metadata;
- implement policy simulation and a bounded human-approval queue;
- add image digest, signature, provenance and SBOM admission;
- complete end-to-end tenant isolation and object-authorization tests;
- define backup, restore, migration, upgrade and high-availability behavior for durable Run evidence;
- add configurable evidence/log retention and external secret references;
- complete signature and replay validation for every enabled integration;
- separate optional subsystem construction and heavy dependencies from the Core process;
- expose the verified Run lifecycle in the Dashboard;
- complete external security review and remediate findings;
- document performance, failure, recovery and capacity tests;
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
