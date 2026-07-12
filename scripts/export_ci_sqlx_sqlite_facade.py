#!/usr/bin/env python3
"""Replace the SQLx umbrella crate with a SQLite-only compatibility facade.

The public crate alias remains `sqlx`, so application code keeps the familiar
runtime query API. The package graph, however, contains only sqlx-core and
sqlx-sqlite; unused MySQL/PostgreSQL/macro/RSA packages are no longer locked.
"""

from __future__ import annotations

import re
from pathlib import Path

SQLX_DEP_RE = re.compile(r"^(?P<indent>\s*)sqlx\s*=\s*.+$", re.MULTILINE)
SQLX_MACRO_RE = re.compile(
    r"sqlx::(?:query|query_as|query_scalar|migrate|test)!"
)
EXPECTED_DECLARATIONS = 4

FACADE_MANIFEST = '''[package]
name = "kias-sqlx-lite"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
sqlx-core = { version = "=0.8.6", default-features = false, features = ["_rt-tokio", "chrono", "uuid"] }
sqlx-sqlite = { version = "=0.8.6", default-features = false, features = ["bundled", "chrono", "uuid"] }
'''

FACADE_LIB = '''//! SQLite-only SQLx compatibility surface for KIAS.
//!
//! KIAS uses runtime-checked SQLite queries and does not use SQLx's compile-time
//! macros or its MySQL/PostgreSQL drivers. Re-exporting the required stable
//! surface from the driver crates prevents unused database and RSA packages from
//! entering `Cargo.lock` while keeping call sites concise.

pub use sqlx_core::acquire::Acquire;
pub use sqlx_core::arguments::{Arguments, IntoArguments};
pub use sqlx_core::column::{Column, ColumnIndex};
pub use sqlx_core::connection::{ConnectOptions, Connection};
pub use sqlx_core::database::{self, Database};
pub use sqlx_core::describe::Describe;
pub use sqlx_core::error::{self, Error, Result};
pub use sqlx_core::executor::{Execute, Executor};
pub use sqlx_core::from_row::FromRow;
pub use sqlx_core::pool::{self, Pool};
pub use sqlx_core::query::{query, query_with};
pub use sqlx_core::query_as::{query_as, query_as_with};
pub use sqlx_core::query_builder::{self, QueryBuilder};
pub use sqlx_core::query_scalar::{query_scalar, query_scalar_with};
pub use sqlx_core::raw_sql::{raw_sql, RawSql};
pub use sqlx_core::row::Row;
pub use sqlx_core::statement::Statement;
pub use sqlx_core::transaction::{Transaction, TransactionManager};
pub use sqlx_core::type_info::TypeInfo;
pub use sqlx_core::types::Type;
pub use sqlx_core::value::{Value, ValueRef};
pub use sqlx_core::{Either, Url};
pub use sqlx_sqlite::{
    self as sqlite, Sqlite, SqliteConnection, SqliteExecutor, SqlitePool,
    SqliteTransaction,
};

/// SQLite and core type integrations exposed under the same path as SQLx.
pub mod types {
    pub use sqlx_core::types::*;
    pub use sqlx_sqlite::types::*;
}
'''


def reject_compile_time_macros() -> None:
    matches: list[str] = []
    for path in sorted(Path("crates").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for match in SQLX_MACRO_RE.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            matches.append(f"{path}:{line}:{match.group(0)}")
    if matches:
        raise SystemExit(
            "SQLite-only facade cannot retain SQLx compile-time macros: "
            + ", ".join(matches)
        )


def rewrite_manifests() -> list[str]:
    declarations = 0
    changed: list[str] = []

    for path in sorted(Path(".").rglob("Cargo.toml")):
        if path == Path("crates/sqlx-lite/Cargo.toml"):
            continue
        text = path.read_text(encoding="utf-8")
        matches = list(SQLX_DEP_RE.finditer(text))
        if not matches:
            continue

        declarations += len(matches)
        replacement = (
            'sqlx = { package = "kias-sqlx-lite", path = "crates/sqlx-lite" }'
            if path == Path("Cargo.toml")
            else 'sqlx = { workspace = true }'
        )
        updated = SQLX_DEP_RE.sub(replacement, text)
        path.write_text(updated, encoding="utf-8")
        changed.append(str(path))

    if declarations != EXPECTED_DECLARATIONS:
        raise SystemExit(
            f"expected {EXPECTED_DECLARATIONS} SQLx dependency declarations, "
            f"rewrote {declarations}"
        )
    return changed


def main() -> None:
    reject_compile_time_macros()
    changed = rewrite_manifests()

    crate = Path("crates/sqlx-lite")
    crate.mkdir(parents=True, exist_ok=True)
    (crate / "Cargo.toml").write_text(FACADE_MANIFEST, encoding="utf-8")
    (crate / "src").mkdir(exist_ok=True)
    (crate / "src/lib.rs").write_text(FACADE_LIB, encoding="utf-8")

    print("Created SQLite-only SQLx facade at crates/sqlx-lite")
    print("Rewired manifests: " + ", ".join(changed))
    Path("scripts/export_ci_sqlx_sqlite_facade.py").unlink()


if __name__ == "__main__":
    main()
