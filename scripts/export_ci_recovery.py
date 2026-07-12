#!/usr/bin/env python3
"""Build the bounded CI recovery candidate in the working tree.

This helper exists only to export a reviewable, testable change bundle. It removes
itself and the temporary recovery workflows from the candidate before packaging.
"""

from pathlib import Path
import subprocess


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def patch_existing_remediator() -> None:
    path = Path("scripts/apply_ci_fixture_fixes.py")
    text = path.read_text(encoding="utf-8")

    text = replace_once(
        text,
        '            if marker < 0:\n                break\n            opening = text.find("{", marker)\n',
        '            if marker < 0:\n                break\n'
        '            line_start = text.rfind("\\n", 0, marker) + 1\n'
        '            line_prefix = text[line_start:marker]\n'
        '            preceding = text[marker - 1] if marker else ""\n'
        '            if preceding.isalnum() or preceding == "_" or "->" in line_prefix:\n'
        '                cursor = marker + len("AppState {")\n'
        '                continue\n'
        '            opening = text.find("{", marker)\n',
        "AppState literal detection",
    )
    text = replace_once(
        text,
        '                indentation = text[line_start:marker]\n'
        '                if indentation.strip():\n'
        '                    raise SystemExit(f"{path}: unexpected AppState indentation at byte {marker}")\n'
        '                insertions.append((opening + 1, f"\\n{indentation}    run_service: None,"))\n',
        '                line_prefix = text[line_start:marker]\n'
        '                indentation = line_prefix[: len(line_prefix) - len(line_prefix.lstrip())]\n'
        '                insertions.append((opening + 1, f"\\n{indentation}    run_service: None,"))\n',
        "AppState indentation",
    )
    text = replace_once(
        text,
        '    replace_exact(\n'
        '        "crates/a2a-registry/src/a2a_enhanced.rs",\n'
        '        "    protocol_negotiator: ProtocolNegotiator,\\n",\n'
        '        "",\n'
        '    )\n'
        '    replace_exact(\n'
        '        "crates/a2a-registry/src/a2a_enhanced.rs",\n'
        '        "            protocol_negotiator: ProtocolNegotiator,\\n",\n'
        '        "",\n'
        '    )\n',
        '    replace_exact(\n'
        '        "crates/a2a-registry/src/a2a_enhanced.rs",\n'
        '        "    protocol_negotiator: ProtocolNegotiator,\\n",\n'
        '        "",\n'
        '        count=2,\n'
        '    )\n',
        "duplicate protocol negotiator fields",
    )
    text = replace_once(
        text,
        '    replace_exact(\n'
        '        "crates/llm-engine/src/provider.rs",\n'
        '        "        let mut chunks: Vec<StreamChunk> = Vec::new();\\n",\n'
        '        "        let chunks: Vec<StreamChunk> = Vec::new();\\n",\n'
        '    )\n',
        '    replace_exact(\n'
        '        "crates/llm-engine/src/provider.rs",\n'
        '        "    fn test_anthropic_sse_parsing_empty_body() {\\n'
        '        let sse_data = \\\"\\\";\\n'
        '        let mut chunks: Vec<StreamChunk> = Vec::new();\\n",\n'
        '        "    fn test_anthropic_sse_parsing_empty_body() {\\n'
        '        let sse_data = \\\"\\\";\\n'
        '        let chunks: Vec<StreamChunk> = Vec::new();\\n",\n'
        '    )\n',
        "provider empty-body fixture",
    )
    path.write_text(text, encoding="utf-8")


def replace_dependency(path_name: str, old: str, new: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise SystemExit(f"{path_name}: SQLx declaration not found")
    path.write_text(text, encoding="utf-8")


def replace_if_needed(path_name: str, old: str, new: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise SystemExit(f"{path_name}: expected old or corrected form")
    path.write_text(text, encoding="utf-8")


def main() -> None:
    patch_existing_remediator()
    subprocess.run(["python3", "scripts/apply_ci_fixture_fixes.py"], check=True)

    replace_dependency(
        "Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }',
        'sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite"] }',
    )
    replace_dependency(
        "crates/data-store/Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }',
        'sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }',
    )
    replace_dependency(
        "crates/data-governance/Cargo.toml",
        'sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }',
        'sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio"] }',
    )
    replace_dependency(
        "crates/api-server/Cargo.toml",
        'sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }',
        'sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite"] }',
    )

    replace_if_needed(
        "crates/common/src/metrics.rs",
        '.find(|mf| mf.get_name() == "kias_scheduler_latency_seconds")',
        '.find(|mf| mf.name() == "kias_scheduler_latency_seconds")',
    )
    replace_if_needed(
        "crates/common/src/tls.rs",
        '        let key = b"not a pem file";\n'
        '        let err = validate_pem_files(cert, key.as_bytes()).unwrap_err();\n',
        '        let key = b"not a pem file";\n'
        '        let err = validate_pem_files(cert, key).unwrap_err();\n',
    )

    privacy = subprocess.run(
        [
            "git",
            "show",
            "c02c29149071f377b38a6004a7cba7e61f625601:.github/workflows/privacy.yml",
        ],
        check=True,
        capture_output=True,
    ).stdout
    Path(".github/workflows/privacy.yml").write_bytes(privacy)

    for path_name in (
        ".github/workflows/fix-test-warnings-once.yml",
        ".github/workflows/ci-recovery-once.yml",
        ".github/workflows/ci-recovery-export.yml",
        "scripts/apply_ci_fixture_fixes.py",
        "scripts/export_ci_recovery.py",
    ):
        Path(path_name).unlink(missing_ok=True)


if __name__ == "__main__":
    main()
