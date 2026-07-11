#!/usr/bin/env python3
"""Scan tracked source files for secrets, personal data, and private identifiers.

The scanner deliberately masks matches and stores only short fingerprints so the
report cannot become a second copy of a leaked credential. A private organization
or customer denylist can be supplied through ORGANIZATION_DENYLIST_B64 or
ORGANIZATION_DENYLIST without committing those names to the public repository.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import ipaddress
import json
import math
import os
import re
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

MAX_TEXT_BYTES = 5 * 1024 * 1024
SAFE_EMAIL_DOMAINS = {
    "example.com",
    "example.net",
    "example.org",
    "example.invalid",
    "test.invalid",
}
BLOCKED_TRACKED_SUFFIXES = {
    ".db",
    ".sqlite",
    ".sqlite3",
    ".dump",
    ".har",
    ".pcap",
    ".p12",
    ".pfx",
    ".jks",
    ".keystore",
    ".key",
}
BLOCKED_TRACKED_NAMES = {
    ".env",
    ".dev-log",
    ".dev-state.yaml",
    ".dev-insights.md",
    ".dev-tasks.yaml",
    ".goal-state.yaml",
    ".orchestrator-state.json",
    ".research-queue.yaml",
}

PATTERNS: list[tuple[str, re.Pattern[str], str]] = [
    (
        "private_key",
        re.compile(r"-----BEGIN (?:RSA |DSA |EC |OPENSSH )?PRIVATE KEY-----"),
        "critical",
    ),
    (
        "github_token",
        re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})\b"),
        "critical",
    ),
    (
        "cloud_access_key",
        re.compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"),
        "critical",
    ),
    (
        "jwt",
        re.compile(r"\beyJ[A-Za-z0-9_-]{5,}\.eyJ[A-Za-z0-9_-]{5,}\.[A-Za-z0-9_-]{8,}\b"),
        "high",
    ),
    (
        "provider_token",
        re.compile(r"\b(?:sk|pk|xox[baprs])[-_][A-Za-z0-9_-]{20,}\b", re.I),
        "high",
    ),
    (
        "secret_assignment",
        re.compile(
            r"(?i)\b(api[_-]?key|secret|password|passwd|pwd|token|client[_-]?secret)\b"
            r"\s*[:=]\s*[\"']?([A-Za-z0-9_./+=:@-]{12,})[\"']?"
        ),
        "high",
    ),
    (
        "email",
        re.compile(r"\b[A-Z0-9._%+-]+@([A-Z0-9.-]+\.[A-Z]{2,})\b", re.I),
        "medium",
    ),
    ("cn_phone", re.compile(r"(?<!\d)1[3-9]\d{9}(?!\d)"), "medium"),
    (
        "international_phone",
        re.compile(r"(?<!\d)(?:\+\d{1,3}[-.\s]?)?(?:\(?\d{2,4}\)?[-.\s]?)\d{3,4}[-.\s]?\d{4}(?!\d)"),
        "medium",
    ),
    (
        "cn_national_id",
        re.compile(r"(?<!\d)\d{6}(?:19|20)\d{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12]\d|3[01])\d{3}[0-9Xx](?!\d)"),
        "high",
    ),
    (
        "local_user_path",
        re.compile(r"(?:/Users/[^/\s]+|/home/[^/\s]+|[A-Za-z]:\\Users\\[^\\\s]+)"),
        "medium",
    ),
    (
        "private_domain",
        re.compile(r"\b[A-Za-z0-9.-]+\.(?:corp|internal|intranet|lan|local)\b", re.I),
        "high",
    ),
]

SAFE_HINTS = (
    "placeholder",
    "dummy",
    "redacted",
    "fake",
    "do-not-log",
    "${",
    "<required>",
    "000000000000",
    "sk-test",
    "test-api-key",
    "your_api_key",
    "example",
)


@dataclass(frozen=True)
class Finding:
    path: str
    line: int
    kind: str
    severity: str
    masked: str
    fingerprint: str
    note: str = ""


def run_git(repo: Path, *args: str) -> bytes:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace").strip())
    return proc.stdout


def mask(value: str) -> str:
    value = value.strip()
    if len(value) <= 6:
        return "*" * len(value)
    return f"{value[:3]}…{value[-3:]}"


def fingerprint(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8", "replace")).hexdigest()[:16]


def entropy(value: str) -> float:
    if not value:
        return 0.0
    counts: dict[str, int] = {}
    for char in value:
        counts[char] = counts.get(char, 0) + 1
    total = len(value)
    return -sum((count / total) * math.log2(count / total) for count in counts.values())


def load_private_denylist(path: Path | None) -> list[str]:
    values: list[str] = []
    if path is not None and path.exists():
        values.extend(path.read_text("utf-8").splitlines())

    plain = os.environ.get("ORGANIZATION_DENYLIST", "")
    values.extend(plain.splitlines())

    encoded = os.environ.get("ORGANIZATION_DENYLIST_B64", "").strip()
    if encoded:
        try:
            values.extend(base64.b64decode(encoded).decode("utf-8").splitlines())
        except (ValueError, UnicodeDecodeError) as exc:
            raise ValueError("ORGANIZATION_DENYLIST_B64 is invalid") from exc

    cleaned = []
    for raw in values:
        value = raw.strip()
        if value and not value.startswith("#"):
            cleaned.append(value)
    return sorted(set(cleaned), key=str.casefold)


def tracked_paths(repo: Path) -> list[str]:
    output = run_git(repo, "ls-files", "-z")
    return [item.decode("utf-8", "surrogateescape") for item in output.split(b"\0") if item]


def is_binary(data: bytes) -> bool:
    sample = data[:8192]
    if b"\x00" in sample:
        return True
    if not sample:
        return False
    control = sum(byte < 9 or 13 < byte < 32 for byte in sample)
    return control / len(sample) > 0.08


def is_public_ip_candidate(value: str) -> bool:
    try:
        address = ipaddress.ip_address(value)
    except ValueError:
        return False
    return not (
        address.is_private
        or address.is_loopback
        or address.is_link_local
        or address.is_multicast
        or address.is_reserved
        or address.is_unspecified
    )


def scan_line(
    path: str,
    line_no: int,
    line: str,
    denylist: Iterable[str],
    *,
    synthetic_context: bool = False,
    pattern_scan: bool = True,
) -> list[Finding]:
    findings: list[Finding] = []
    lower_line = line.lower()

    patterns = PATTERNS if pattern_scan else []
    for kind, pattern, severity in patterns:
        for match in pattern.finditer(line):
            value = match.group(0)
            if kind == "secret_assignment" and not re.search(r"[:=]\s*[\"']", value):
                continue
            finding_severity = severity
            note = ""

            if kind == "email":
                domain = match.group(1).lower()
                if domain in SAFE_EMAIL_DOMAINS:
                    finding_severity = "info"
                    note = "reserved_test_domain"

            if any(hint in lower_line for hint in SAFE_HINTS):
                finding_severity = "info"
                note = "likely_test_or_placeholder"
            elif synthetic_context and kind in {
                "secret_assignment",
                "email",
                "cn_phone",
                "international_phone",
                "cn_national_id",
                "local_user_path",
            }:
                finding_severity = "info"
                note = "synthetic_test_context"

            findings.append(
                Finding(
                    path=path,
                    line=line_no,
                    kind=kind,
                    severity=finding_severity,
                    masked=mask(value),
                    fingerprint=fingerprint(value),
                    note=note,
                )
            )

    for candidate in re.findall(r"\b(?:\d{1,3}\.){3}\d{1,3}\b", line):
        if is_public_ip_candidate(candidate):
            findings.append(
                Finding(
                    path=path,
                    line=line_no,
                    kind="public_ip",
                    severity="medium",
                    masked=mask(candidate),
                    fingerprint=fingerprint(candidate),
                )
            )

    for match in re.finditer(r"[\"']([A-Za-z0-9_./+=:@-]{24,})[\"']", line):
        value = match.group(1)
        if entropy(value) >= 4.1 and not any(hint in value.lower() for hint in SAFE_HINTS):
            findings.append(
                Finding(
                    path=path,
                    line=line_no,
                    kind="high_entropy_string",
                    severity="medium",
                    masked=mask(value),
                    fingerprint=fingerprint(value),
                )
            )

    for private_identifier in denylist:
        if private_identifier.casefold() in line.casefold():
            findings.append(
                Finding(
                    path=path,
                    line=line_no,
                    kind="private_organization_identifier",
                    severity="high",
                    masked="[PRIVATE-DENYLIST-MATCH]",
                    fingerprint=fingerprint(private_identifier.casefold()),
                )
            )

    return findings


def scan_repository(repo: Path, denylist: list[str]) -> tuple[list[Finding], list[str]]:
    findings: list[Finding] = []
    binary_files: list[str] = []

    for relative in tracked_paths(repo):
        path = repo / relative
        suffix = path.suffix.lower()
        if path.name in BLOCKED_TRACKED_NAMES or suffix in BLOCKED_TRACKED_SUFFIXES:
            findings.append(
                Finding(
                    path=relative,
                    line=0,
                    kind="blocked_tracked_file",
                    severity="high",
                    masked="[FILE-POLICY-VIOLATION]",
                    fingerprint=fingerprint(relative),
                )
            )

        try:
            size = path.stat().st_size
            data = path.read_bytes()
        except (OSError, PermissionError):
            continue

        if size > MAX_TEXT_BYTES or is_binary(data):
            binary_files.append(relative)
            continue

        text = data.decode("utf-8", "replace")
        rust_test_module = False
        fixture_path = any(
            part in {"tests", "testdata", "fixtures"}
            for part in Path(relative).parts
        )
        for line_no, line in enumerate(text.splitlines(), 1):
            if relative.endswith(".rs") and line.strip() == "#[cfg(test)]":
                rust_test_module = True
            findings.extend(
                scan_line(
                    relative,
                    line_no,
                    line,
                    denylist,
                    synthetic_context=fixture_path or rust_test_module,
                    pattern_scan=relative != "scripts/privacy_scan.py",
                )
            )

        for private_identifier in denylist:
            if private_identifier.casefold() in relative.casefold():
                findings.append(
                    Finding(
                        path=relative,
                        line=0,
                        kind="private_identifier_in_path",
                        severity="high",
                        masked="[PRIVATE-DENYLIST-PATH]",
                        fingerprint=fingerprint(private_identifier.casefold()),
                    )
                )

    unique = {
        (item.path, item.line, item.kind, item.fingerprint): item for item in findings
    }
    return sorted(unique.values(), key=lambda item: (item.path, item.line, item.kind)), sorted(binary_files)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--denylist-file", type=Path)
    parser.add_argument("--output", type=Path, default=Path("privacy-scan.json"))
    parser.add_argument("--fail-on", choices=("critical", "high", "medium"), default="high")
    args = parser.parse_args()

    repo = args.repo.resolve()
    denylist = load_private_denylist(args.denylist_file)
    findings, binary_files = scan_repository(repo, denylist)

    severity_rank = {"info": 0, "medium": 1, "high": 2, "critical": 3}
    threshold = severity_rank[args.fail_on]
    summary: dict[str, int] = {}
    for item in findings:
        summary[item.severity] = summary.get(item.severity, 0) + 1

    payload = {
        "summary": summary,
        "finding_count": len(findings),
        "private_denylist_entries_loaded": len(denylist),
        "binary_files_requiring_manual_review": binary_files,
        "findings": [asdict(item) for item in findings],
        "notice": "Values are masked; fingerprints are truncated SHA-256 values.",
    }
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2), "utf-8")

    print(json.dumps(summary, sort_keys=True))
    print(f"Privacy report: {args.output}")
    if binary_files:
        print(f"Manual binary review required for {len(binary_files)} tracked files")

    return 1 if any(severity_rank[item.severity] >= threshold for item in findings) else 0


if __name__ == "__main__":
    sys.exit(main())
