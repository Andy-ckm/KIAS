# Privacy and Data-Minimization Policy

KIAS is infrastructure software. Deployers remain responsible for determining their lawful basis, retention periods, access controls, and notices for any personal or confidential data processed by their deployment.

## Project principles

1. **Collect less.** Do not collect data that is not required for a documented function.
2. **Log metadata, not content.** Logs must not contain credentials, authorization headers, cookies, query strings, message bodies, document contents, exact locations, or raw identity-provider payloads.
3. **Use pseudonymous identifiers.** Operational metrics should use scoped, non-reversible identifiers instead of names, email addresses, or account IDs.
4. **Separate audit and application logs.** Audit records require stricter access controls, retention limits, and tamper evidence.
5. **Make sensitive capture opt-in.** Raw webhook payloads, prompts, responses, files, and diagnostic dumps must be disabled by default.
6. **Expire data.** Every persistent data class should have a documented retention period and deletion path.
7. **Do not commit real data.** Tests, examples, issues, and documentation must use synthetic values under reserved domains such as `example.invalid`.

## Sensitive data classes

Treat the following as sensitive even when they are not regulated personal data in every jurisdiction:

- names, email addresses, phone numbers, addresses, IP addresses, device identifiers, and precise location;
- user, tenant, channel, workspace, employee, customer, and document identifiers;
- prompts, model responses, chat messages, uploaded files, and extracted document text;
- passwords, password hashes, password history, API keys, tokens, cookies, private keys, assertions, tickets, backup codes, and two-factor secrets;
- employer, customer, partner, internal-system, project-code-name, and enterprise-domain identifiers.

## Repository content rules

The public repository must not contain:

- production configuration or credentials;
- real organization names used as private customer or employer identifiers;
- enterprise email domains, tenant IDs, internal hostnames, VPN addresses, or private endpoints;
- exported logs, databases, chat transcripts, support tickets, or user documents;
- internal agent state, task queues, prompts, scratch files, or local absolute paths.

Public third-party names may be used only where necessary to identify an open protocol, dependency, or supported integration. Examples should otherwise use neutral provider names.

## Logging requirements

- Record URL paths, not complete URLs or query strings.
- Never log request or response bodies by default.
- Redact authorization, cookie, token, password, key, assertion, and secret fields.
- Avoid logging raw `Debug` representations of request, credential, user, session, message, or secret types.
- Bound log retention and restrict export destinations.

## Incident handling

Potential privacy incidents should be reported privately under `SECURITY.md`. Immediately rotate exposed credentials, preserve necessary evidence securely, stop further collection, and identify any downstream copies such as artifacts, releases, forks, caches, and log systems.

## Contributor checklist

Before opening a pull request:

- run the secret and privacy scans;
- inspect fixtures, snapshots, screenshots, and binary files;
- verify that all identities and organizations are synthetic;
- check `git diff --cached` and the commit metadata;
- confirm that new telemetry has a purpose, retention rule, and redaction strategy.
