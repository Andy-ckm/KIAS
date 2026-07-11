# Project Status and Readiness

KIAS is a pre-1.0 open-source project under active development. This page distinguishes implemented code from verified behavior and from production-readiness claims.

## Maturity statement

| Area | Current status | Evidence expected |
|---|---|---|
| Workspace build | Gated in CI | `cargo build/check --workspace --all-features` |
| Unit and integration tests | Gated in CI | `cargo test --workspace --all-features` |
| Rust lint and formatting | Gated in CI | Clippy with warnings denied; rustfmt check |
| Dashboard | Gated in CI | dependency install, lint, production build |
| Static security analysis | Enabled | CodeQL results in repository Security tab |
| Dependency updates | Enabled | automated Cargo, frontend, and workflow updates |
| Repository security posture | Enabled | OpenSSF Scorecard workflow |
| Secret and privacy scanning | Enabled | masked CI scan with private organization denylist support |
| Release provenance | Planned in this hardening cycle | checksums and signed artifact attestation |
| Independent security audit | Not completed | third-party report and tracked remediation |
| Multi-tenant isolation | Experimental | adversarial end-to-end tests required |
| Production support commitment | Not offered | documented support and release policy required |

A green CI run demonstrates that configured checks passed for a revision. It is not proof that the system is vulnerability-free or suitable for every deployment.

## Useful project scope

KIAS is intended to be useful to teams that need reusable infrastructure for one or more of these problems:

- managing agent lifecycle state instead of launching isolated scripts;
- scheduling agent workloads by capacity, cost, or cache affinity;
- expressing long-running or branching work as resumable workflows;
- applying role, tool, autonomy, rate, and budget policies;
- collecting operational metrics and privacy-aware audit evidence;
- testing isolation and failure-recovery designs for agent runtimes;
- experimenting with protocol and integration adapters behind explicit boundaries.

KIAS is not intended to be:

- a hosted model service;
- a replacement for a deployment's identity provider, secret manager, network controls, or incident-response program;
- a guarantee that arbitrary tools, prompts, models, or external data are safe;
- an organization-specific workflow or private compliance mapping repository.

## Verification levels

Features should be described using one of these levels:

1. **Prototype** — code demonstrates an interface or approach; not enabled by default.
2. **Tested** — deterministic automated tests cover normal and important failure paths.
3. **Integrated** — cross-crate or API-level tests cover the behavior and its security boundary.
4. **Hardened** — threat model, misuse cases, resource limits, observability, and operational documentation exist.
5. **Audited** — an independent review has been completed and findings are tracked publicly when safe.

Until a feature has explicit evidence, readers should assume the lowest applicable level.

## Security and privacy hardening completed in the current change set

- removed tracked local development state, research queues, orchestration state, and API-key queue data;
- replaced full-URI logging with path-only logging;
- removed raw provider error bodies from application errors;
- added redacted `Debug` and serialization behavior for credentials and secret values;
- changed password storage to salted Argon2id hashes;
- implemented standards-compatible TOTP generation;
- stored recovery codes as hashes and consumed them once;
- pseudonymized authentication audit subjects;
- disabled raw webhook payload retention by default;
- implemented signed-request validation for a supported adapter and made incomplete adapters fail closed;
- changed secret scan findings to masked values and fingerprints;
- added security, privacy, governance, contribution, support, and conduct policies;
- added static analysis, Scorecard, dependency, CI, and privacy gates.

## Release blockers for a production-oriented 1.0

- complete end-to-end tenant isolation and authorization tests;
- complete signature and replay validation for every enabled integration;
- replace or isolate any remaining demonstration-grade security primitives;
- complete external security review and remediate findings;
- document performance, failure, recovery, backup, and upgrade tests;
- publish compatibility and deprecation policy;
- establish reproducible release artifacts, provenance, and verification instructions;
- demonstrate at least one complete, documented reference deployment using only synthetic data;
- establish maintainer succession and regular release/security response practice.

## Evidence policy

README and release claims must be supported by one or more of:

- a stable automated test;
- a reproducible benchmark with environment and methodology;
- a public design document and implementation link;
- a release artifact and verification command;
- an independent assessment.

Avoid vanity metrics, unsupported superlatives, undated model/provider comparisons, and statements such as “production-grade” when the corresponding evidence is absent.

## Privacy and organizational independence

The public repository must not contain personal data, employer/customer/partner identifiers, enterprise domains, tenant IDs, internal systems, private project names, production logs, or real credentials. The project is general-purpose and does not claim endorsement by any employer or customer.
