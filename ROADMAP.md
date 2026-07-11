# Roadmap

KIAS uses an evidence-driven roadmap. Priorities are ordered by user safety and usefulness rather than feature count.

## Guiding principles

- secure and understandable defaults before broad integration coverage;
- a smaller verified capability is more valuable than a larger unverified claim;
- public core features must solve reusable problems and contain no organization-specific data;
- every security boundary needs failure-path tests and operational documentation;
- compatibility, migration, and removal are part of feature design.

## Phase 1 — Trustworthy pre-1.0 foundation

### Security and privacy

- [x] Remove tracked local state and credential queue artifacts.
- [x] Redact credential and secret diagnostics.
- [x] Use salted Argon2id password hashing.
- [x] Standardize TOTP and hash single-use recovery codes.
- [x] Remove query strings and raw provider bodies from logs.
- [x] Make incomplete webhook adapters fail closed.
- [x] Add private organization-identifier scanning support.
- [ ] Add end-to-end authorization and tenant-isolation misuse tests.
- [ ] Add fuzzing for parsers, policy inputs, protocol messages, and workflow state.
- [ ] Complete an independent security assessment.

### Engineering quality

- [x] Workspace tests, Clippy, formatting, frontend build, and static analysis in CI.
- [x] Automated dependency updates and OpenSSF Scorecard.
- [ ] Stabilize deterministic integration-test fixtures.
- [ ] Publish code coverage without using it as a substitute for meaningful tests.
- [ ] Define public API compatibility and deprecation policy.
- [ ] Reduce or feature-gate heavy optional dependencies.

### Open-source sustainability

- [x] Security, privacy, governance, contribution, support, and conduct policies.
- [x] Evidence-based project status and threat model.
- [ ] Establish at least one additional regular reviewer.
- [ ] Introduce issue triage and release cadence.
- [ ] Publish a complete synthetic reference deployment.

## Phase 2 — Verifiable releases

- [ ] Correct, locked release builds for documented targets.
- [ ] Checksums and signed artifact attestations.
- [ ] Software bill of materials and license report.
- [ ] Release verification instructions and rollback guidance.
- [ ] Upgrade and data-migration tests.
- [ ] Container image with minimal base, non-root execution, and signed provenance.

## Phase 3 — Hardened control plane

- [ ] Tenant-scoped storage, caches, policies, audit, and resource quotas.
- [ ] Authorization tests against cross-tenant object references.
- [ ] Durable event and workflow recovery under process/node failure.
- [ ] Backpressure, overload, and graceful-degradation behavior.
- [ ] Secret-manager plugin contract with rotation and revocation tests.
- [ ] Policy simulation and dry-run explanations.

## Phase 4 — Safe agent execution

- [ ] Documented sandbox capability matrix and escape assumptions.
- [ ] Network egress, filesystem, process, and resource policies.
- [ ] Human approval for irreversible/high-impact actions.
- [ ] Prompt-injection and tool-output adversarial test suite.
- [ ] Reproducible evaluation of recovery, policy, and containment behavior.

## Phase 5 — Ecosystem and interoperability

- [ ] Versioned protocol compatibility tests.
- [ ] Integration conformance suite.
- [ ] Plugin SDK and lifecycle contract.
- [ ] Reference adapters that use synthetic fixtures and fail closed.
- [ ] Examples focused on user outcomes rather than provider-specific marketing.

## Non-goals

The public core will not contain:

- employer, customer, partner, tenant, or internal project identifiers;
- proprietary workflows, private compliance mappings, or internal documents;
- default collection of prompts, messages, documents, precise location, or raw identity data;
- unsupported claims of certification, endorsement, safety, or production readiness.

Roadmap items may change as evidence and user feedback improve. Completed checkboxes indicate repository work, not third-party certification.
