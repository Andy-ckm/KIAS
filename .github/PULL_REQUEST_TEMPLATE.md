## Problem and user impact

<!-- What problem does this solve? Who benefits? What happens if it is not changed? -->

## Approach

<!-- Describe the design, important trade-offs, and alternatives considered. -->

## Scope

- [ ] This change is focused and avoids unrelated churn.
- [ ] Public API or configuration changes are documented.
- [ ] Migration or rollback guidance is included where needed.

## Security and privacy

- [ ] I reviewed the threat and misuse cases for this change.
- [ ] No credentials, personal data, private organization/customer identifiers, production logs, internal paths, or proprietary documents are included.
- [ ] Logs exclude secrets, query strings, message/document content, and direct identity unless explicitly required.
- [ ] New external inputs are authenticated/verified, validated, size-bounded, and replay-safe where applicable.
- [ ] High-impact tools/actions remain policy-controlled and fail closed.
- [ ] Retention, deletion, and access controls are documented for new persisted data.

## Validation

<!-- Include exact commands and meaningful results. Do not paste sensitive logs. -->

```text
cargo fmt --all -- --check:
cargo clippy --workspace --all-targets --all-features -- -D warnings:
cargo test --workspace --all-features:
frontend lint/build (when applicable):
privacy/secret scan:
```

## Documentation

- [ ] User-facing documentation is updated.
- [ ] `CHANGELOG.md` is updated for notable changes.
- [ ] Threat model or project status is updated when a trust boundary or readiness claim changes.

## Reviewer notes

<!-- Call out risky code, assumptions, follow-up work, generated files, or areas needing specialist review. -->
