# Security Policy

## Supported versions

Security fixes are provided for the latest release and the default branch. Older releases may receive fixes when the issue is critical and backporting is practical.

| Version | Supported |
|---|---|
| Latest release | Yes |
| `main` | Yes |
| Older releases | Best effort |

## Reporting a vulnerability

Please do **not** open a public issue for suspected vulnerabilities, exposed credentials, privacy incidents, or bypasses of authentication and authorization controls.

Use GitHub's **Report a vulnerability** private security advisory flow for this repository. Include:

- affected revision or version;
- reproduction steps or a minimal proof of concept;
- expected and observed behavior;
- impact and realistic attack scenario;
- suggested mitigation, when known.

Do not include real personal data, production credentials, customer information, or proprietary documents in a report. Use synthetic test data and redact secrets.

## Response targets

The project aims to:

- acknowledge a report within 3 business days;
- provide an initial severity assessment within 7 business days;
- publish a remediation plan for confirmed issues;
- coordinate disclosure after a fix is available.

These are service targets, not contractual guarantees.

## Scope

Security-sensitive areas include:

- authentication, authorization, sessions, tokens, and key handling;
- audit logs and privacy controls;
- webhook origin verification and replay protection;
- agent sandboxing and tool execution;
- supply-chain integrity and release provenance;
- multi-tenant isolation;
- data ingestion, document processing, and model-provider integrations.

## Secret exposure

If a credential is committed or disclosed:

1. revoke or rotate it immediately;
2. review provider audit logs;
3. remove it from the current tree;
4. rewrite repository history when necessary;
5. notify affected users where required.

Deleting a file in a later commit does not remove it from Git history.

## Safe-harbor intent

Good-faith security research that avoids privacy violations, service disruption, persistence, data destruction, and access beyond what is necessary to demonstrate an issue is welcomed. The maintainers ask researchers to provide reasonable time for remediation before public disclosure.
