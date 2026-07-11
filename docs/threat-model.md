# KIAS Threat Model

This document defines the security assumptions and primary trust boundaries for KIAS. It is a starting point for deployment-specific analysis, not a certification or substitute for an independent review.

## Security objectives

KIAS should:

- prevent unauthorized creation, modification, execution, or deletion of managed agents and workflows;
- limit the impact of compromised agents, prompts, tools, integrations, and model responses;
- keep credentials, identity data, message content, documents, and configuration secrets out of logs and public artifacts;
- provide attributable but privacy-aware audit evidence for security-relevant actions;
- preserve tenant and workload boundaries;
- fail closed when authentication, signature verification, or policy evaluation is unavailable;
- produce releases whose source and build provenance can be verified.

## Assets

High-value assets include:

- model-provider, API, webhook, database, TLS, and signing credentials;
- authentication assertions, sessions, recovery codes, and identity claims;
- prompts, model responses, chat messages, documents, embeddings, and tool output;
- agent definitions, policies, budgets, schedules, and workflow state;
- audit records and operational telemetry;
- build credentials, release artifacts, dependency manifests, and provenance;
- host, container, cluster, and tenant resources reachable by tools.

## Trust boundaries

```text
Untrusted user / external service
            │
            ▼
    API and webhook boundary
            │ authentication, signature, replay, size limits
            ▼
       Control plane
            │ authorization, policy, tenancy, audit
            ▼
      Agent runtime boundary
            │ tool allowlist, sandbox, budgets, egress
            ▼
 External models, tools, stores, and infrastructure
```

Every boundary must validate data again. Validation at an upstream layer does not make downstream content trusted.

## Threat actors

- unauthenticated internet attackers;
- authenticated users exceeding their authorization;
- malicious or compromised tenants;
- prompt-injection content embedded in messages, files, web pages, or tool output;
- compromised external integrations or model providers;
- malicious dependencies or CI actions;
- maintainers or operators making accidental configuration mistakes;
- attackers with read access to logs, backups, artifacts, or repository history.

## Primary threats and controls

### Credential exposure

Threats:

- committed secrets;
- credential values printed through `Debug`, serialization, errors, or tracing;
- raw provider error bodies or webhook headers entering logs;
- secrets retained in process memory longer than necessary.

Controls:

- external secret injection and secret references;
- redacted credential and secret types;
- memory zeroization where practical;
- repository and CI secret scanning that masks matches;
- restricted log schemas;
- immediate credential rotation after suspected exposure.

### Identity and personal-data leakage

Threats:

- names, email addresses, account IDs, IP addresses, device information, messages, or precise location entering logs and audit exports;
- raw webhook or identity-provider payload retention;
- organization/customer identifiers committed to the public repository.

Controls:

- data minimization and explicit retention;
- path-only HTTP logging;
- pseudonymous audit subjects;
- raw-payload retention disabled by default;
- private organization denylist in CI;
- public examples restricted to synthetic data and reserved domains.

### Authentication and session attacks

Threats:

- password cracking;
- token theft or replay;
- weak recovery codes;
- session fixation or excessive lifetime;
- authorization bypass through fallback authentication.

Controls:

- Argon2id password hashing with unique salts;
- standards-compatible TOTP;
- hashed, single-use recovery codes;
- bounded sessions and role enforcement;
- fail-closed authentication configuration;
- no credential values in diagnostic output.

### Webhook forgery and replay

Threats:

- forged messages and events;
- replay of previously valid requests;
- attacker-controlled payloads injected into downstream agents.

Controls:

- provider-specific signature verification;
- bounded timestamp window and replay protection;
- unsigned or unsupported adapters fail closed;
- request-size and content validation;
- raw request retention disabled.

### Prompt injection and unsafe tools

Threats:

- untrusted content instructs an agent to reveal data, change policy, or execute unsafe tools;
- tool parameters cause command injection, path traversal, network exfiltration, or destructive changes;
- model output is treated as authoritative code or policy.

Controls:

- untrusted-content labels and context separation;
- allowlisted tools and constrained parameters;
- sandboxing, filesystem and network restrictions;
- autonomy, rate, budget, and side-effect gates;
- output validation and human approval for high-impact actions;
- audit of policy decisions without sensitive payloads.

### Tenant-boundary failure

Threats:

- cross-tenant object references;
- shared cache or storage keys exposing another tenant's data;
- globally scoped agent or memory state;
- insufficient resource quotas.

Controls:

- tenant identifiers included in authorization and storage keys;
- deny-by-default access checks at service and repository layers;
- tenant-scoped encryption, cache namespaces, quotas, and tests;
- no global fallback for tenant-specific resources.

Multi-tenant isolation remains a release-blocking area until end-to-end adversarial tests demonstrate these properties.

### Supply-chain compromise

Threats:

- unpinned CI actions;
- compromised dependency updates;
- release artifacts that do not correspond to reviewed source;
- mutable or unsigned distribution channels.

Controls:

- full-SHA pinning for CI actions;
- automated dependency updates and vulnerability checks;
- locked builds;
- checksums and signed build attestations;
- least-privilege workflow permissions;
- protected default branch and required reviews.

## Logging rules

Do not log:

- authorization headers, cookies, tokens, keys, passwords, assertions, tickets, recovery codes, or private keys;
- complete URLs with query strings;
- request/response bodies, prompts, model output, documents, or webhook payloads by default;
- names, email addresses, precise location, IP addresses, device strings, or raw identity claims unless a documented audit requirement exists;
- complete `Debug` output for credentials, users, sessions, messages, or secrets.

Audit data and application logs must use separate storage, access, export, and retention controls.

## Deployment assumptions

A secure deployment is expected to provide:

- TLS termination and network segmentation;
- a managed secret store and regular rotation;
- protected identity provider configuration;
- least-privilege service accounts;
- sandbox and egress restrictions appropriate to enabled tools;
- encrypted storage and backups;
- centralized monitoring with redaction and bounded retention;
- tested incident response, recovery, and deletion processes.

## Known limitations

- Not every integration adapter has a complete signature implementation; incomplete adapters must remain disabled and fail closed.
- The project has not completed an independent penetration test or security audit.
- Multi-tenant isolation requires further end-to-end verification.
- In-memory zeroization cannot guarantee removal of every compiler/runtime copy.
- A secure framework cannot make arbitrary user-supplied tools safe without deployment controls.

## Review cadence

Update this threat model when adding:

- a new authentication method, integration, storage backend, sandbox, tool, or protocol;
- multi-tenant behavior;
- new data collection, retention, export, or telemetry;
- a release or build-system change;
- a newly discovered attack path or incident lesson.
