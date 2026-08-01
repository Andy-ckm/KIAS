# Controlled Change Agent Harness — Synthetic Reference Profile

Status: Labs  
Date: 2026-08-02

## Why this replaces the old presentation-only CCR demo

The previous shell demo printed predetermined approvals, signatures, successful execution, and compliance claims. It did not execute a state machine, enforce evidence, prove separation of duties, block duplicate effects, or create verifiable recovery state. Presentation text is not operational evidence.

This reference profile executes three deterministic scenarios:

1. `happy`: evidence-complete approval, dry-run side effect, independent verification, closure.
2. `approval-bypass`: requester attempts to approve their own request and is denied.
3. `evidence-missing`: approval is attempted before required evidence exists and is denied.

## Harness contract

```text
Register
  → Collect bounded evidence
  → Evaluate deterministic policy
  → Require independent approval
  → Checkpoint
  → Execute idempotent dry-run effect
  → Verify independently
  → Close or roll back
  → Export hash-verifiable evidence
```

The LLM boundary is intentionally non-authoritative. A model may summarize, identify missing facts, draft questions, or explain rules. It cannot approve, sign, implement, waive evidence, or close a change.

## Control–Evidence–Recovery envelope

**Control**

- explicit transition allowlist;
- evidence gates before high-impact transitions;
- requester/approver separation of duties;
- implementation only from the `implementing` state;
- idempotency key blocks duplicate effects.

**Evidence**

- append-only event sequence;
- SHA-256 link from each event to its predecessor;
- content hashes for evidence objects;
- report hash over the exported envelope.

**Recovery**

- checkpoint after every state transition and side effect;
- checkpoint contains state, evidence hashes, last event hash, and side-effect keys;
- resume must verify checkpoint and event-chain hashes before continuing.

## Run

```bash
./demo/ccr-demo.sh happy
./demo/ccr-demo.sh approval-bypass
./demo/ccr-demo.sh evidence-missing
```

The full JSON evidence package is written under `/tmp` unless another output path is supplied.

## Important boundary

This is a synthetic engineering reference. It is not regulatory certification, not an electronic-signature system, and not authorization to modify production systems. Organization-specific procedures, approved policies, identity providers, records retention, validation, and Quality decisions must remain downstream and controlled.
