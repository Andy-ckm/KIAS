# Project Status and Readiness

KIAS is a pre-1.0 open-source project under active development. This page distinguishes implemented code from verified behavior and from production-readiness claims.

## Maturity statement

| Area | Current status | Evidence expected |
|---|---|---|
| Product scope | Defined | `PRODUCT.md` and capability maturity matrix |
| Architecture boundaries | Enforced in CI | Core default surface; no Core dependency on Labs packages |
| Workspace build | Gated in CI | `cargo check --workspace --all-features --locked` |
| Unit and integration tests | Gated in CI | `cargo test --workspace --all-features --locked` |
| Rust lint and formatting | Gated in CI | Clippy with warnings denied; rustfmt check |
| Dashboard | Gated in CI | dependency install, lint, and production build |
| Static security analysis | Enabled | CodeQL results in repository Security tab |
| Dependency updates and audit | Enabled | Dependabot and RustSec advisory workflow |
| Repository security posture | Enabled | OpenSSF Scorecard workflow |
| Secret, PII, and organization scanning | Enabled | masked read-only CI scan with private denylist support |
| Release provenance | Partially implemented | checksums, attestations, and verification instructions require release validation |
| Independent security audit | Not completed | third-party report and tracked remediation |
| Multi-tenant isolation | Experimental | adversarial end-to-end tests required |
| Production support commitment | Not offered | documented support and regular release practice required |

A green CI run demonstrates that configured checks passed for a revision. It is not proof that the system is vulnerability-free or suitable for every deployment.

## Product boundary

KIAS is a self-hosted, policy-driven control plane for teams operating tool-using AI agents. The intended users are AI platform engineers, security and governance engineers, SRE/operations teams, and architects evaluating transparent agent-control infrastructure.

The stable product direction is organized around three outcomes:

- **Control** — identity, tools, autonomy, budgets, rate limits, and resources require explicit decisions;
- **Evidence** — important behavior produces privacy-aware operational and audit evidence;
- **Recovery** — work has bounded retries, cancellation, checkpoints, reconciliation, and graceful shutdown.

KIAS is not intended to be a hosted model service, model-training platform, no-code chatbot builder, generic operating-system automation suite, public-data collection platform, or repository for organization-specific workflows and compliance mappings.

See [`../PRODUCT.md`](../PRODUCT.md) and [`capability-maturity.md`](capability-maturity.md).

## Core, Extensions, and Labs

The workspace contains three explicit maturity tiers:

- **Core** is the default Cargo surface and the long-term security/compatibility boundary;
- **Extensions** provide optional integrations and higher-level capabilities through Core interfaces;
- **Labs** contains disabled-by-default research with no compatibility promise.

A machine-enforced architecture check rejects Core dependencies on Labs packages. The API no longer depends on the experimental self-modifying loop; bounded deterministic classification and decomposition now live in the Core `kias-intent-core` crate.

The full workspace is still built and tested in CI so experimental code cannot silently decay, but its presence is not presented as a production commitment.

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
- removed a repository-wide fixed JWT fallback;
- aligned runtime environment loading with the documented `KIAS_` prefix and `__` nested separator;
- made the shipped listener loopback-only and authentication-enabled;
- refused unauthenticated public listeners and misleading native-TLS configuration;
- removed silent durable-storage fallback to volatile memory;
- removed self-modifying CI workflows after their bounded one-time remediation tasks;
- added security, privacy, governance, contribution, support, conduct, release, and maintenance policies;
- added architecture, static-analysis, Scorecard, dependency, CI, and privacy gates with pinned workflow actions.

## Engineering changes completed in the current change set

- defined a Core-only default Cargo build while retaining full-workspace CI;
- reduced the process composition root to resources it actually owns;
- replaced unconditional construction and false-positive health claims with explicit process readiness;
- added deterministic startup validation for authentication, token length, JWT lifetime, scheduler settings, controller timing, storage mode, and TLS configuration;
- changed the CLI so no arguments display help rather than unexpectedly starting a service;
- synchronized command-line listener overrides with runtime configuration;
- refreshed and locked Rust dependencies;
- added concise failure artifacts for Rust, dashboard, architecture, and privacy checks;
- clarified feature maturity and promotion criteria crate by crate;
- aligned README, architecture, logo, and product language with evidence rather than unsupported superlatives.

## Verification status for this pull request

The pull request remains Draft until its current maintainer-authored head revision passes all configured read-only checks:

- architecture boundary check;
- Rust workspace tests;
- Rust Clippy with warnings denied;
- Rust formatting;
- dashboard lint and production build;
- privacy and secret scan;
- CodeQL analysis;
- Rust dependency audit.

The current verification cycle was re-triggered after bounded mechanical formatting and lockfile workflows removed themselves. Failures are handled through masked or build-only diagnostic artifacts. Normal CI does not write code or comments back to the repository.

## Known pre-1.0 limitations

- the API still contains optional knowledge, messaging, protocol, and industry-oriented surfaces that require further feature-gating or extraction;
- synthetic bootstrap nodes remain for dashboard and handler-contract tests; production discovery must replace them;
- native TLS is not wired into the `kias` server binary, so deployments must use a trusted TLS-terminating proxy and explicit acknowledgement for non-loopback listeners;
- multi-tenant storage and authorization isolation has not completed adversarial end-to-end verification;
- not every external integration has completed signature, replay, retention, and conformance testing;
- optional provider-specific configuration names remain for pre-1.0 compatibility;
- release provenance and reproducibility require validation on an actual tagged release;
- no independent security audit has been completed.

## Release blockers for a production-oriented 1.0

- complete end-to-end tenant isolation and authorization tests;
- complete signature and replay validation for every enabled integration;
- feature-gate or extract remaining Extensions and Labs from the default server/API surface;
- replace or isolate any remaining demonstration-grade security primitives;
- remove synthetic bootstrap state from production composition;
- complete external security review and remediate findings;
- document performance, failure, recovery, backup, restore, migration, and upgrade tests;
- publish compatibility and deprecation policy backed by release practice;
- validate reproducible release artifacts, provenance, SBOM, and verification instructions;
- demonstrate at least one complete reference deployment using only synthetic data;
- establish maintainer succession and regular security-response practice.

## Repository-administrator actions outside the code change

The following actions cannot be completed safely by a normal source-code pull request and remain mandatory:

- rotate every API key, bot token, webhook secret, signing key, and other credential ever used on a machine working with this repository;
- inspect provider usage logs for unexpected calls or source addresses;
- enable repository secret scanning and push protection where available;
- configure branch/ruleset protection so required checks and review cannot be bypassed casually;
- configure the private `ORGANIZATION_DENYLIST_B64` secret with employer, customer, partner, domain, project-code, and internal-system aliases;
- perform a reviewed full-history rewrite for previously tracked internal-state paths and sensitive blobs, force-push all branches/tags, and require collaborators to re-clone;
- inspect forks, mirrors, releases, Actions artifacts, caches, and old clones that may retain pre-rewrite objects;
- consider moving the project to a neutral organization account if personal repository ownership itself reveals unwanted identity context.

History rewriting never substitutes for credential rotation.

## Evidence policy

README and release claims must be supported by one or more of:

- a stable automated test;
- a reproducible benchmark with environment and methodology;
- a public design document and implementation link;
- a release artifact and verification command;
- an independent assessment.

Avoid vanity metrics, unsupported superlatives, undated provider comparisons, and statements such as “production-grade” when the corresponding evidence is absent.

## Privacy and organizational independence

The public repository must not contain personal data, employer/customer/partner identifiers, enterprise domains, tenant IDs, internal systems, private project names, production logs, or real credentials. Examples use synthetic data and reserved domains. The project is general-purpose and does not claim endorsement by any employer or customer.
