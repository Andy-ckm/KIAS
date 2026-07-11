# Changelog

All notable user-visible changes will be documented here.

The project follows [Semantic Versioning](https://semver.org/) for releases. Before 1.0, minor versions may contain breaking changes when they are documented with a migration path.

## Unreleased

### Security

- Remove tracked local development state, research queues, orchestration state, and API-key queue artifacts.
- Log HTTP paths without query strings.
- Pseudonymize authentication audit subjects and redact audit diagnostics.
- Redact credential and secret `Debug`/serialization output.
- Use salted Argon2id password hashes instead of direct SHA-256 password hashing.
- Use standards-compatible TOTP and hashed, single-use recovery codes.
- Suppress external provider response bodies in application errors.
- Disable raw webhook payload retention by default.
- Verify signed requests for a supported webhook adapter and make incomplete adapters fail closed.
- Store secret scan findings as masked values and fingerprints rather than copied secrets.
- Add masked repository scanning for secrets, PII, private domains, local paths, and privately supplied organization identifiers.
- Remove shared JWT and TLS demonstration fallbacks; reject misleading or unauthenticated public listener configurations.
- Align runtime configuration with the documented `KIAS_` environment prefix and fail startup when required security or durable-storage configuration is invalid.

### Product

- Define KIAS as a self-hosted, policy-driven control plane for tool-using AI agents.
- Establish **Control, Evidence, and Recovery** as the stable product outcomes.
- Define primary users, non-goals, feature-admission criteria, removal criteria, and evidence-based success measures.
- Classify crates as Core, Extensions, or Labs, with explicit promotion requirements and pre-1.0 support boundaries.
- Replace the Core API dependency on experimental self-modifying loops with bounded deterministic intent classification and decomposition.

### Engineering

- Add workspace tests, Clippy, formatting, dashboard lint/build, CodeQL, dependency updates, and OpenSSF Scorecard workflows.
- Pin third-party workflow actions by full commit SHA.
- Add evidence-based project status, threat model, and readiness criteria.
- Replace unsupported README claims and undated provider comparisons with reproducible project information.
- Make Core crates the default Cargo surface while retaining full-workspace verification.
- Enforce Core-to-Labs dependency boundaries in CI.
- Reduce the process composition root to resources it actually owns and remove false-positive health claims.
- Make CLI startup explicit, listener overrides consistent, and CI diagnostics concise and reproducible.

### Community

- Add security, privacy, contribution, governance, conduct, maintainer, support, roadmap, and release policies.
- Replace obsolete branding with a neutral KIAS identity aligned to the documented product contract.

## 0.1.0

Initial public pre-1.0 release.
