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

### Engineering

- Add workspace tests, Clippy, formatting, dashboard lint/build, CodeQL, dependency updates, and OpenSSF Scorecard workflows.
- Pin third-party workflow actions by full commit SHA.
- Add evidence-based project status, threat model, and readiness criteria.
- Replace unsupported README claims and undated provider comparisons with reproducible project information.

### Community

- Add security, privacy, contribution, governance, conduct, maintainer, support, roadmap, and release policies.

## 0.1.0

Initial public pre-1.0 release.
