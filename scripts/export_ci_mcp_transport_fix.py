#!/usr/bin/env python3
"""Make the MCP HTTP transport consistently feature-gated, then self-delete."""

from pathlib import Path


def replace_once(path_name: str, old: str, new: str, label: str) -> None:
    path = Path(path_name)
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    transport = "crates/mcp-protocol/src/transport.rs"
    replace_once(
        transport,
        "/// Shared state for the HTTP transport.\nstruct HttpTransportState {\n",
        "/// Shared state for the HTTP transport.\n#[cfg(feature = \"http\")]\nstruct HttpTransportState {\n",
        "HTTP transport state feature gate",
    )
    replace_once(
        transport,
        "pub struct HttpTransport {\n",
        "#[cfg(feature = \"http\")]\npub struct HttpTransport {\n",
        "HTTP transport type feature gate",
    )
    replace_once(
        transport,
        "impl HttpTransport {\n",
        "#[cfg(feature = \"http\")]\nimpl HttpTransport {\n",
        "HTTP transport inherent impl feature gate",
    )
    replace_once(
        transport,
        "#[async_trait]\nimpl McpTransport for HttpTransport {\n",
        "#[cfg(feature = \"http\")]\n#[async_trait]\nimpl McpTransport for HttpTransport {\n",
        "HTTP transport trait impl feature gate",
    )

    lib = "crates/mcp-protocol/src/lib.rs"
    replace_once(
        lib,
        "pub use transport::{\n"
        "    HttpTransport, InMemoryTransport as ServerInMemoryTransport, McpTransport, StdioTransport,\n"
        "};\n",
        "pub use transport::{\n"
        "    InMemoryTransport as ServerInMemoryTransport, McpTransport, StdioTransport,\n"
        "};\n"
        "#[cfg(feature = \"http\")]\n"
        "pub use transport::HttpTransport;\n",
        "HTTP transport public export feature gate",
    )

    Path("scripts/export_ci_mcp_transport_fix.py").unlink()


if __name__ == "__main__":
    main()
