# Governance

KIAS is maintained in the open. Technical decisions should be traceable, reviewable, and based on user value, security, correctness, and maintainability.

## Roles

### Contributor

Anyone who reports issues, improves documentation, reviews changes, writes code, or helps users.

### Reviewer

A contributor trusted to review changes in one or more areas. Reviewers evaluate correctness, tests, compatibility, documentation, security, and privacy but do not merge changes unless they are also maintainers.

### Maintainer

A contributor with responsibility for repository administration, release decisions, security response, roadmap stewardship, and final merge authority. Maintainers are listed in `MAINTAINERS.md`.

## Decision process

- Small, reversible changes are decided through normal pull-request review.
- Significant architecture, compatibility, governance, security-model, or data-model changes require a public design issue or proposal.
- Decisions should record the problem, constraints, alternatives, trade-offs, security/privacy impact, and migration path.
- Maintainers seek rough consensus. When consensus is not possible, a maintainer may decide and must document the rationale.
- Security incidents may be handled privately until coordinated disclosure is safe.

## Merge policy

Changes to the default branch should arrive through pull requests and pass required checks. Self-approval is discouraged for security-sensitive changes. High-risk changes should receive review from at least one maintainer who did not author the change.

Examples of high-risk areas include:

- authentication, authorization, session, key, and secret handling;
- webhook verification and external integrations;
- sandboxing and command execution;
- multi-tenant isolation;
- audit, privacy, retention, and export behavior;
- build, release, dependency, and supply-chain workflows.

## Maintainer selection

A reviewer may become a maintainer after sustained, high-quality contributions that demonstrate:

- sound technical judgment;
- respectful and reliable collaboration;
- attention to security, privacy, tests, and documentation;
- willingness to maintain existing code, not only add features;
- understanding of the project's scope and users.

Existing maintainers approve new maintainers by documented consensus.

## Inactivity and removal

Maintainers may step down at any time. Maintainer access may be removed after prolonged inactivity, repeated failure to protect the project, serious conduct violations, or conflicts of interest that cannot be managed. The reason should be documented while respecting privacy.

## Conflicts of interest

Participants should disclose material conflicts that could affect a decision. Employment alone is not disqualifying, but undisclosed customer, vendor, financial, or organizational interests must not control project decisions.

## Project scope

KIAS focuses on secure, observable, policy-driven infrastructure for operating AI agents. Features should solve reusable problems for a broad community. Organization-specific workflows, customer identifiers, private compliance mappings, and proprietary integrations belong in external extensions rather than the public core.
