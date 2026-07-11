# Contributing to KIAS

Thank you for helping improve KIAS. Contributions are welcome in code, tests, documentation, design review, security hardening, examples, and issue triage.

## Before you start

- Search existing issues and pull requests.
- For a substantial feature or architecture change, open a design issue first.
- For a security or privacy concern, follow `SECURITY.md` instead of opening a public issue.
- Never use production credentials, real personal data, customer data, internal company names, or proprietary documents in code, tests, screenshots, logs, issues, or pull requests.

## Development setup

KIAS is a Rust workspace. A typical local validation loop is:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Some crates may require optional system services. Keep tests deterministic and isolate external services behind mocks or feature flags.

## Change requirements

Every pull request should:

1. solve one clearly described problem;
2. include tests for success and failure paths;
3. preserve backward compatibility or document the migration;
4. avoid new warnings and unsafe code unless justified;
5. update user-facing documentation when behavior changes;
6. avoid unrelated formatting or generated-file churn;
7. pass privacy, secret, dependency, and static-analysis checks.

## Security and privacy checklist

Before committing:

```bash
git diff --cached
# Run the repository-provided privacy and secret checks when available.
```

Verify that the change does not introduce:

- secrets, tokens, private keys, session identifiers, or credentials;
- names, email addresses, phone numbers, precise locations, IP addresses, or message content;
- employer, customer, partner, enterprise-domain, tenant, or internal project identifiers;
- raw webhook bodies, request headers, query strings, prompts, model responses, or uploaded documents in logs;
- binary dumps, databases, archives, screenshots, or generated artifacts without review.

Use reserved synthetic values such as:

- `user@example.invalid`
- `tenant-00000000`
- `198.51.100.10` or `2001:db8::1`
- `example.invalid`

## Commit and pull-request style

Use concise, imperative commit subjects, for example:

- `fix: reject replayed webhook requests`
- `security: redact credential debug output`
- `docs: clarify multi-tenant isolation`

A pull request description should explain:

- the problem and user impact;
- the chosen approach and alternatives considered;
- security, privacy, compatibility, and performance implications;
- tests and validation performed;
- follow-up work that is intentionally out of scope.

## Review expectations

Maintainers may request changes for correctness, maintainability, security, privacy, documentation, or project scope. Authors should keep discussions technical and respectful and resolve review threads with evidence, not merely acknowledgement.

## Licensing

By submitting a contribution, you agree that it is licensed under the repository's MIT License and that you have the right to contribute it.
