# KIAS Product Definition

## One-sentence definition

KIAS is a self-hosted, policy-driven control plane for operating tool-using AI agents with explicit lifecycle, authorization, audit, recovery, and resource boundaries.

KIAS is infrastructure for teams operating agents. It is not an end-user chatbot builder, a model-training platform, or a catalogue of unrelated AI experiments.

## Primary users

### AI platform engineers

They need a repeatable way to register, schedule, observe, stop, recover, and upgrade multiple agents without embedding operational logic in every application.

### Security and governance engineers

They need enforceable controls for identity, tools, budgets, autonomy, sensitive data, audit evidence, and high-impact actions.

### SRE and operations teams

They need health signals, bounded retries, graceful degradation, durable recovery, incident evidence, and predictable failure behavior.

### Solution and platform architects

They need a transparent, self-hosted reference architecture for evaluating agent-control patterns without depending on a proprietary hosted control plane.

## Jobs to be done

KIAS should help a team:

1. declare the desired state and policy of an agent;
2. place work using capacity and policy signals;
3. execute resumable workflows through controlled tools;
4. observe behavior without collecting unnecessary personal or confidential data;
5. interrupt, contain, recover, and explain agent actions;
6. produce evidence that controls ran and failures were handled;
7. extend integrations without weakening the core trust boundary.

## Core product promise

KIAS is organized around three outcomes.

### Control

Every agent and tool action is subject to explicit identity, policy, budget, autonomy, and resource decisions. Unsupported security integrations fail closed.

### Evidence

Operational and security-relevant behavior is observable through privacy-aware metrics, traces, state transitions, and pseudonymous audit records.

### Recovery

Workflows and managed resources have explicit health, retry, cancellation, checkpoint, reconciliation, and graceful-shutdown behavior.

A capability that does not materially strengthen one of these outcomes should not enter the stable core.

## Capability tiers

### Core

Core capabilities form the supported control-plane contract. They require deterministic tests, documented failure behavior, stable configuration, security review, and an upgrade path.

Core currently includes:

- common types, configuration, masking, and errors;
- API and identity boundary;
- lifecycle controller and reconciliation primitives;
- workload scheduling;
- workflow execution and checkpoints;
- policy, autonomy, and tool-execution boundaries;
- persistence, audit, metrics, health, and recovery primitives;
- model routing interfaces required by controlled execution.

### Extensions

Extensions solve useful integration or higher-level orchestration problems but are not required to operate the core control plane. They must use published interfaces and may evolve faster than Core.

Candidate extensions include:

- cache and knowledge backends;
- agent collaboration and verifier patterns;
- protocol adapters;
- skills and harness registries;
- document-processing components;
- optional user interfaces and CLI clients.

### Labs

Labs are research or demonstration code. They are disabled by default, carry no compatibility promise, and must not be described as production-ready.

Labs include:

- autonomous goal loops and self-modifying development loops;
- social or public-data aggregation;
- instant-messaging adapters that have not completed platform conformance tests;
- generic operating-system and change-management automation;
- industry-specific compliance mappings;
- experimental agent frameworks and evaluation harnesses.

A Labs capability graduates only after it has a named user problem, an owner, a threat model, deterministic tests, integration evidence, operational documentation, and a maintenance commitment.

## Non-goals

KIAS does not aim to be:

- a hosted foundation-model or inference service;
- a model-training, dataset-management, or GPU-cluster platform;
- a no-code chatbot or consumer assistant builder;
- a universal automation suite for every infrastructure task;
- a repository for proprietary workflows or organization-specific compliance rules;
- a replacement for an identity provider, secret manager, network policy, SIEM, backup system, or incident-response program;
- a claim of certification, regulatory approval, or safety by itself.

## Feature admission test

A proposed Core feature must answer all of the following:

1. Which primary user and operational problem does it serve?
2. Which Core outcome—Control, Evidence, or Recovery—does it strengthen?
3. Why can it not be implemented as an extension?
4. What is the smallest stable interface?
5. What are the abuse, privacy, failure, and rollback paths?
6. What deterministic evidence will prove it works?
7. Who maintains it through upgrades and security incidents?

Features that cannot answer these questions remain Extensions, stay in Labs, or are removed.

## Removal and deprecation policy

A capability is a removal candidate when it:

- has no identified user or maintainer;
- duplicates a better-supported component;
- bypasses Core security or observability boundaries;
- cannot fail safely;
- adds heavy dependencies disproportionate to its value;
- makes unsupported product claims;
- is organization-specific or contains sensitive context;
- has remained experimental without evidence of adoption.

Public APIs use pre-1.0 semantic versioning. Breaking changes require release notes, migration guidance where practical, and a deprecation window once an interface is declared stable.

## Success measures

KIAS should be evaluated by evidence rather than feature count:

- time to deploy a synthetic reference agent safely;
- percentage of Core actions covered by authorization and failure-path tests;
- recovery time and correctness under injected failures;
- audit completeness without raw PII retention;
- number of optional integrations that fail closed and pass conformance tests;
- reproducibility and verifiability of releases;
- issue response, review, and release health;
- real external deployments or case studies using synthetic/public evidence.

A smaller system that can explain and survive its failures is more useful than a sprawling system that merely compiles.