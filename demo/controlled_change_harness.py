#!/usr/bin/env python3
"""Machine-verifiable controlled-change Agent Harness reference.

Synthetic only. Demonstrates state, policy, evidence, separation of duties,
checkpoints, hash-chain audit, failure, and resume primitives. It does not claim
regulatory certification or production authorization.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def canonical(value: Any) -> str:
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(",", ":"), default=str)


def digest(value: Any) -> str:
    return hashlib.sha256(canonical(value).encode()).hexdigest()


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


TRANSITIONS = {
    "draft": {"submitted"},
    "submitted": {"impact_assessed"},
    "impact_assessed": {"awaiting_approval"},
    "awaiting_approval": {"approved", "rejected"},
    "approved": {"implementing"},
    "implementing": {"verifying", "rolled_back"},
    "verifying": {"closed", "rolled_back"},
    "closed": set(),
    "rejected": set(),
    "rolled_back": set(),
}

REQUIRED = {
    "approved": {"impact_assessment", "risk_assessment", "implementation_plan", "rollback_plan", "approval_record"},
    "implementing": {"approval_record", "precheck_result"},
    "closed": {"verification_result", "postcheck_result"},
}


@dataclass
class Event:
    sequence: int
    event_type: str
    state: str
    actor: str
    payload: dict[str, Any]
    previous_hash: str
    event_hash: str
    occurred_at: str


@dataclass
class ChangeRun:
    run_id: str
    requester: str
    state: str = "draft"
    evidence: dict[str, dict[str, Any]] = field(default_factory=dict)
    events: list[Event] = field(default_factory=list)
    checkpoints: list[dict[str, Any]] = field(default_factory=list)
    side_effect_keys: set[str] = field(default_factory=set)

    def append_event(self, event_type: str, actor: str, payload: dict[str, Any]) -> None:
        previous_hash = self.events[-1].event_hash if self.events else "GENESIS"
        body = {
            "sequence": len(self.events) + 1,
            "event_type": event_type,
            "state": self.state,
            "actor": actor,
            "payload": payload,
            "previous_hash": previous_hash,
        }
        self.events.append(Event(**body, event_hash=digest(body), occurred_at=now()))

    def add_evidence(self, name: str, actor: str, content: dict[str, Any]) -> None:
        record = {"actor": actor, "content": content, "content_hash": digest(content), "recorded_at": now()}
        self.evidence[name] = record
        self.append_event("evidence_recorded", actor, {"name": name, "content_hash": record["content_hash"]})

    def transition(self, target: str, actor: str) -> None:
        if target not in TRANSITIONS[self.state]:
            raise ValueError(f"invalid_transition:{self.state}->{target}")
        missing = sorted(REQUIRED.get(target, set()) - self.evidence.keys())
        if missing:
            raise ValueError(f"missing_evidence:{','.join(missing)}")
        if target == "approved":
            approval = self.evidence["approval_record"]["content"]
            if approval.get("approver") == self.requester:
                raise ValueError("separation_of_duties_violation:requester_cannot_approve")
            if actor != approval.get("approver"):
                raise ValueError("approval_actor_mismatch")
        previous = self.state
        self.state = target
        self.append_event("state_transition", actor, {"from": previous, "to": target})
        self.checkpoint(actor)

    def execute_side_effect(self, *, actor: str, idempotency_key: str, operation: str) -> None:
        if self.state != "implementing":
            raise ValueError("side_effect_outside_implementing")
        if idempotency_key in self.side_effect_keys:
            self.append_event("duplicate_side_effect_blocked", actor, {"idempotency_key": idempotency_key})
            return
        self.side_effect_keys.add(idempotency_key)
        self.append_event("synthetic_side_effect", actor, {
            "idempotency_key": idempotency_key,
            "operation": operation,
            "mode": "dry_run",
        })
        self.checkpoint(actor)

    def checkpoint(self, actor: str) -> dict[str, Any]:
        payload = {
            "run_id": self.run_id,
            "state": self.state,
            "evidence_hashes": {k: v["content_hash"] for k, v in sorted(self.evidence.items())},
            "last_event_hash": self.events[-1].event_hash if self.events else "GENESIS",
            "side_effect_keys": sorted(self.side_effect_keys),
        }
        checkpoint = {
            "sequence": len(self.checkpoints) + 1,
            "actor": actor,
            "payload": payload,
            "payload_hash": digest(payload),
            "created_at": now(),
        }
        self.checkpoints.append(checkpoint)
        return checkpoint

    def verify(self) -> list[str]:
        errors: list[str] = []
        previous = "GENESIS"
        for event in self.events:
            body = {
                "sequence": event.sequence,
                "event_type": event.event_type,
                "state": event.state,
                "actor": event.actor,
                "payload": event.payload,
                "previous_hash": event.previous_hash,
            }
            if event.previous_hash != previous:
                errors.append(f"event_chain_break:{event.sequence}")
            if event.event_hash != digest(body):
                errors.append(f"event_hash_mismatch:{event.sequence}")
            previous = event.event_hash
        for checkpoint in self.checkpoints:
            if checkpoint["payload_hash"] != digest(checkpoint["payload"]):
                errors.append(f"checkpoint_hash_mismatch:{checkpoint['sequence']}")
        return errors

    def report(self, scenario: str, expected_failure: str | None = None) -> dict[str, Any]:
        errors = self.verify()
        report = {
            "schema_version": "controlled-change-evidence/1.0",
            "scenario": scenario,
            "run_id": self.run_id,
            "state": self.state,
            "expected_failure": expected_failure,
            "events": [asdict(event) for event in self.events],
            "checkpoints": self.checkpoints,
            "evidence_index": {
                key: {"actor": value["actor"], "content_hash": value["content_hash"]}
                for key, value in sorted(self.evidence.items())
            },
            "verification": {"passed": not errors, "errors": errors},
            "non_claims": [
                "synthetic fixture only",
                "no production action performed",
                "no regulatory certification claimed",
            ],
        }
        report["report_hash"] = digest(report)
        return report


def seed_until_approval(run: ChangeRun, approver: str = "quality-reviewer") -> None:
    run.append_event("run_created", run.requester, {"intended_use": "synthetic controlled change"})
    run.checkpoint(run.requester)
    run.transition("submitted", run.requester)
    run.add_evidence("impact_assessment", "impact-agent", {"impact": "indirect", "unknowns": []})
    run.add_evidence("risk_assessment", "risk-agent", {"risk": "medium", "controls": ["dry_run"]})
    run.add_evidence("implementation_plan", "change-owner", {"steps": ["precheck", "apply", "verify"]})
    run.add_evidence("rollback_plan", "change-owner", {"steps": ["restore_fixture"]})
    run.transition("impact_assessed", "impact-agent")
    run.transition("awaiting_approval", "workflow-engine")
    run.add_evidence("approval_record", approver, {"approver": approver, "decision": "approve", "scope": run.run_id})


def run_scenario(scenario: str) -> tuple[dict[str, Any], int]:
    run = ChangeRun(run_id=f"ccr-{scenario}-001", requester="requester")
    expected_failure = None
    exit_code = 0
    try:
        if scenario == "happy":
            seed_until_approval(run)
            run.transition("approved", "quality-reviewer")
            run.add_evidence("precheck_result", "executor", {"passed": True})
            run.transition("implementing", "executor")
            run.execute_side_effect(actor="executor", idempotency_key="apply-001", operation="update_synthetic_fixture")
            run.execute_side_effect(actor="executor", idempotency_key="apply-001", operation="duplicate_should_be_blocked")
            run.add_evidence("verification_result", "independent-verifier", {"passed": True})
            run.add_evidence("postcheck_result", "independent-verifier", {"passed": True})
            run.transition("verifying", "independent-verifier")
            run.transition("closed", "independent-verifier")
        elif scenario == "approval-bypass":
            seed_until_approval(run, approver=run.requester)
            expected_failure = "separation_of_duties_violation"
            run.transition("approved", run.requester)
        elif scenario == "evidence-missing":
            run.append_event("run_created", run.requester, {})
            run.checkpoint(run.requester)
            run.transition("submitted", run.requester)
            run.transition("impact_assessed", "impact-agent")
            run.transition("awaiting_approval", "workflow-engine")
            expected_failure = "missing_evidence"
            run.transition("approved", "quality-reviewer")
        else:
            raise ValueError(f"unknown_scenario:{scenario}")
    except ValueError as exc:
        run.append_event("policy_denied", "harness", {"reason": str(exc)})
        if not expected_failure or expected_failure not in str(exc):
            exit_code = 2
    report = run.report(scenario, expected_failure)
    if not report["verification"]["passed"]:
        exit_code = 3
    return report, exit_code


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", choices=["happy", "approval-bypass", "evidence-missing"], default="happy")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report, exit_code = run_scenario(args.scenario)
    text = json.dumps(report, ensure_ascii=False, indent=2)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text + "\n")
    print(text)
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
