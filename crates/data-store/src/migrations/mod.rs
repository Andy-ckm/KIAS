//! # Migration System
//!
//! Versioned schema management for SQLite. Each migration has a version number,
//! description, and SQL statements that are applied atomically.

use kias_common::{KiasError, KiasResult};
use sqlx::SqlitePool;
use tracing::info;

/// Represents a single migration step.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Monotonically increasing version number.
    pub version: i32,
    /// Human-readable description.
    pub description: &'static str,
    /// SQL statements to apply (executed in order, each as a separate statement).
    pub up_sql: &'static str,
    /// SQL statements to roll back (for future use).
    pub down_sql: &'static str,
}

/// Built-in migrations for the KIAS data store.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "Create core tables: agents, tasks, workflows, configs, skills, components",
        up_sql: include_str!("001_core_tables.sql"),
        down_sql: "DROP TABLE IF EXISTS components; DROP TABLE IF EXISTS skills; DROP TABLE IF EXISTS configs; DROP TABLE IF EXISTS workflow_steps; DROP TABLE IF EXISTS workflows; DROP TABLE IF EXISTS tasks; DROP TABLE IF EXISTS agents;",
    },
    Migration {
        version: 2,
        description: "Create vector storage tables",
        up_sql: include_str!("002_vector_tables.sql"),
        down_sql: "DROP TABLE IF EXISTS vector_entries; DROP TABLE IF EXISTS vector_indices;",
    },
    Migration {
        version: 3,
        description: "Create cache storage table",
        up_sql: include_str!("003_cache_table.sql"),
        down_sql: "DROP TABLE IF EXISTS cache_entries;",
    },
    Migration {
        version: 4,
        description: "Create experience replay and prefix cache tables",
        up_sql: include_str!("004_experience_replay.sql"),
        down_sql: "DROP TABLE IF EXISTS prefix_cache; DROP TABLE IF EXISTS experience_replay;",
    },
    Migration {
        version: 5,
        description: "Create audit log and dead letter queue tables",
        up_sql: include_str!("005_audit_dlq.sql"),
        down_sql: "DROP TABLE IF EXISTS dead_letter_queue; DROP TABLE IF EXISTS audit_log;",
    },
    Migration {
        version: 6,
        description: "Create HNSW graph snapshot table for fast restart",
        up_sql: include_str!("006_hnsw_graphs.sql"),
        down_sql: "DROP TABLE IF EXISTS hnsw_graphs;",
    },
    Migration {
        version: 7,
        description: "Create token budget management tables",
        up_sql: include_str!("007_token_budgets.sql"),
        down_sql: "DROP TABLE IF EXISTS spend_alerts; DROP TABLE IF EXISTS token_usage; DROP TABLE IF EXISTS token_budgets;",
    },
];

/// Manages database migrations.
pub struct MigrationRunner {
    pool: SqlitePool,
}

impl MigrationRunner {
    /// Create a new migration runner.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the migration tracking table if it doesn't exist.
    pub async fn init(&self) -> KiasResult<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| KiasError::Config(format!("Failed to create migrations table: {e}")))?;
        Ok(())
    }

    /// Get the current schema version (highest applied migration).
    pub async fn current_version(&self) -> KiasResult<i32> {
        let row: (i32,) = sqlx::query_as("SELECT COALESCE(MAX(version), 0) FROM _migrations")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to query migration version: {e}")))?;
        Ok(row.0)
    }

    /// Apply all pending migrations.
    pub async fn run_all(&self) -> KiasResult<Vec<i32>> {
        self.init().await?;
        let current = self.current_version().await?;
        let mut applied = Vec::new();

        for migration in MIGRATIONS {
            if migration.version > current {
                info!(
                    "Applying migration v{}: {}",
                    migration.version, migration.description
                );
                self.apply(migration).await?;
                applied.push(migration.version);
            }
        }

        if applied.is_empty() {
            info!("Database schema is up to date (v{})", current);
        } else {
            info!("Applied {} migration(s)", applied.len());
        }

        Ok(applied)
    }

    /// Apply a single migration atomically.
    async fn apply(&self, migration: &Migration) -> KiasResult<()> {
        // Split SQL by semicolons and execute each statement
        let statements: Vec<&str> = migration
            .up_sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to begin transaction: {e}")))?;

        for stmt in statements {
            sqlx::query(stmt).execute(&mut *tx).await.map_err(|e| {
                KiasError::Config(format!("Migration v{} failed: {e}", migration.version))
            })?;
        }

        // Record the migration
        sqlx::query("INSERT INTO _migrations (version, description) VALUES (?, ?)")
            .bind(migration.version)
            .bind(migration.description)
            .execute(&mut *tx)
            .await
            .map_err(|e| KiasError::Config(format!("Failed to record migration: {e}")))?;

        tx.commit()
            .await
            .map_err(|e| KiasError::Config(format!("Failed to commit migration: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_init_and_version() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let runner = MigrationRunner::new(pool);
        runner.init().await.expect("Failed to init migrations");

        let version = runner
            .current_version()
            .await
            .expect("Failed to get version");
        assert_eq!(version, 0, "Fresh DB should have version 0");
    }

    #[tokio::test]
    async fn test_run_all_migrations() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let runner = MigrationRunner::new(pool.clone());
        let applied = runner.run_all().await.expect("Failed to run migrations");

        assert_eq!(applied.len(), 6, "Should apply 6 migrations");
        assert_eq!(applied, vec![1, 2, 3, 4, 5, 6]);

        let version = runner
            .current_version()
            .await
            .expect("Failed to get version");
        assert_eq!(version, 6, "Should be at version 6");

        // Running again should be a no-op
        let applied_again = runner
            .run_all()
            .await
            .expect("Failed to run migrations again");
        assert!(applied_again.is_empty(), "Should not re-apply migrations");
    }

    #[tokio::test]
    async fn test_tables_created() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to in-memory SQLite");

        let runner = MigrationRunner::new(pool.clone());
        runner.run_all().await.expect("Failed to run migrations");

        // Verify all core tables exist
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&pool)
                .await
                .expect("Failed to query tables");

        let table_names: Vec<&str> = tables.iter().map(|(name,)| name.as_str()).collect();
        assert!(table_names.contains(&"agents"), "agents table missing");
        assert!(table_names.contains(&"tasks"), "tasks table missing");
        assert!(
            table_names.contains(&"workflows"),
            "workflows table missing"
        );
        assert!(table_names.contains(&"configs"), "configs table missing");
        assert!(table_names.contains(&"skills"), "skills table missing");
        assert!(
            table_names.contains(&"components"),
            "components table missing"
        );
        assert!(
            table_names.contains(&"vector_entries"),
            "vector_entries table missing"
        );
        assert!(
            table_names.contains(&"cache_entries"),
            "cache_entries table missing"
        );
        assert!(
            table_names.contains(&"experience_replay"),
            "experience_replay table missing"
        );
        assert!(
            table_names.contains(&"prefix_cache"),
            "prefix_cache table missing"
        );
    }
}
