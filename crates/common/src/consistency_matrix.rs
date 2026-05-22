//! Consistency Matrix — multi-dimensional consistency level control for distributed systems.
//!
//! Provides a matrix-based approach to manage consistency levels across different dimensions:
//! - **Read/Write consistency**: tunable per-operation
//! - **Node consistency**: per-node quorum requirements
//! - **Temporal consistency**: time-bounded staleness windows
//! - **Partition tolerance**: CP vs AP hybrid modes
//!
//! # Consistency Levels
//!
//! | Level | Description | Use Case |
//! |-------|-------------|----------|
//! | `Strong` | Linearizable, all nodes see same data | Financial transactions |
//! | `Session` | Monotonic reads/writes per client | User sessions |
//! | `Causal` | Respect happens-before relationships | Collaborative editing |
//! | `Eventual` | Guaranteed convergence, no staleness bound | Analytics, logging |
//!
//! # Example
//!
//! ```
//! use kias_common::consistency_matrix::{ConsistencyMatrix, ConsistencyLevel, MatrixDimension};
//!
//! let matrix = ConsistencyMatrix::default()
//!     .with_read_level(ConsistencyLevel::Session)
//!     .with_write_level(ConsistencyLevel::Strong)
//!     .with_staleness_bound_ms(500)
//!     .with_quorum_size(3);
//!
//! assert_eq!(matrix.get_read_level(), ConsistencyLevel::Session);
//! assert_eq!(matrix.get_quorum_size(), 3);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock as StdRwLock};

/// Consistency level enumeration — ordered from strongest to weakest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConsistencyLevel {
    /// Linearizable — all operations appear atomic, real-time ordered.
    Strong = 5,
    /// Sequential — all nodes see writes in same order, but not necessarily real-time.
    Sequential = 4,
    /// Causal — respects happens-before relationships only.
    Causal = 3,
    /// Session — monotonic reads/writes per client, eventual across clients.
    Session = 2,
    /// Eventual — convergence guaranteed, no staleness bound.
    Eventual = 1,
}

impl fmt::Display for ConsistencyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConsistencyLevel::Strong => write!(f, "Strong"),
            ConsistencyLevel::Sequential => write!(f, "Sequential"),
            ConsistencyLevel::Causal => write!(f, "Causal"),
            ConsistencyLevel::Session => write!(f, "Session"),
            ConsistencyLevel::Eventual => write!(f, "Eventual"),
        }
    }
}

impl ConsistencyLevel {
    /// Returns true if this level guarantees read-your-writes session guarantee.
    pub fn guarantees_read_your_writes(&self) -> bool {
        matches!(
            self,
            ConsistencyLevel::Strong | ConsistencyLevel::Sequential | ConsistencyLevel::Session
        )
    }

    /// Returns true if this level guarantees monotonic reads.
    pub fn guarantees_monotonic_reads(&self) -> bool {
        matches!(
            self,
            ConsistencyLevel::Strong | ConsistencyLevel::Sequential | ConsistencyLevel::Session
        )
    }

    /// Returns the approximate replication factor required to achieve this level.
    pub fn min_replication_factor(&self) -> usize {
        match self {
            ConsistencyLevel::Strong => 3,
            ConsistencyLevel::Sequential => 3,
            ConsistencyLevel::Causal => 2,
            ConsistencyLevel::Session => 2,
            ConsistencyLevel::Eventual => 1,
        }
    }
}

/// Dimensions of the consistency matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MatrixDimension {
    /// Read consistency dimension.
    Read,
    /// Write consistency dimension.
    Write,
    /// Metadata consistency dimension.
    Metadata,
    /// Heartbeat/keepalive consistency.
    Heartbeat,
}

impl fmt::Display for MatrixDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatrixDimension::Read => write!(f, "Read"),
            MatrixDimension::Write => write!(f, "Write"),
            MatrixDimension::Metadata => write!(f, "Metadata"),
            MatrixDimension::Heartbeat => write!(f, "Heartbeat"),
        }
    }
}

/// Hybrid consistency mode for partition tolerance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConsistencyMode {
    /// CP (Consistency/Partition tolerance) — sacrifice availability.
    Cp,
    /// AP (Availability/Partition tolerance) — sacrifice strong consistency.
    Ap,
    /// Tunable consistency — per-dimension settings.
    #[default]
    Tunable,
}

/// Per-dimension consistency configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionConfig {
    /// Consistency level for this dimension.
    pub level: ConsistencyLevel,
    /// Quorum size (0 = use level default).
    pub quorum_size: usize,
    /// Staleness bound in milliseconds (0 = no bound).
    pub staleness_bound_ms: u64,
    /// Timeout for operations on this dimension.
    pub timeout_ms: u64,
}

impl Default for DimensionConfig {
    fn default() -> Self {
        Self {
            level: ConsistencyLevel::Eventual,
            quorum_size: 0,
            staleness_bound_ms: 0,
            timeout_ms: 5000,
        }
    }
}

impl DimensionConfig {
    /// Effective quorum size (uses level default if not set).
    pub fn effective_quorum(&self, cluster_size: usize) -> usize {
        if self.quorum_size > 0 {
            self.quorum_size
        } else {
            (cluster_size / 2) + 1
        }
    }
}

/// Global consistency matrix configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyMatrixConfig {
    /// Cluster size for quorum calculations.
    pub cluster_size: usize,
    /// Default consistency mode.
    pub mode: ConsistencyMode,
    /// Per-dimension overrides.
    pub dimension_overrides: HashMap<MatrixDimension, DimensionConfig>,
    /// Global staleness bound in ms (0 = no bound).
    pub global_staleness_bound_ms: u64,
    /// Enable strict quorum enforcement.
    pub strict_quorum: bool,
}

impl Default for ConsistencyMatrixConfig {
    fn default() -> Self {
        Self {
            cluster_size: 3,
            mode: ConsistencyMode::Tunable,
            dimension_overrides: HashMap::new(),
            global_staleness_bound_ms: 1000,
            strict_quorum: true,
        }
    }
}

/// Consistency Matrix — manages multi-dimensional consistency levels.
#[derive(Debug, Clone)]
pub struct ConsistencyMatrix {
    config: ConsistencyMatrixConfig,
    /// Current read level.
    read_level: ConsistencyLevel,
    /// Current write level.
    write_level: ConsistencyLevel,
    /// Current metadata level.
    metadata_level: ConsistencyLevel,
    /// Current heartbeat level.
    heartbeat_level: ConsistencyLevel,
    /// Computed effective quorum size.
    quorum_size: usize,
    /// Staleness bound in ms.
    staleness_bound_ms: u64,
}

impl Default for ConsistencyMatrix {
    fn default() -> Self {
        Self {
            config: ConsistencyMatrixConfig::default(),
            read_level: ConsistencyLevel::Session,
            write_level: ConsistencyLevel::Strong,
            metadata_level: ConsistencyLevel::Sequential,
            heartbeat_level: ConsistencyLevel::Eventual,
            quorum_size: 2,
            staleness_bound_ms: 1000,
        }
    }
}

impl ConsistencyMatrix {
    /// Create a new matrix with default settings.
    pub fn new(config: ConsistencyMatrixConfig) -> Self {
        let mut matrix = Self {
            config: config.clone(),
            read_level: ConsistencyLevel::Session,
            write_level: ConsistencyLevel::Strong,
            metadata_level: ConsistencyLevel::Sequential,
            heartbeat_level: ConsistencyLevel::Eventual,
            quorum_size: (config.cluster_size / 2) + 1,
            staleness_bound_ms: config.global_staleness_bound_ms,
        };
        matrix.recompute();
        matrix
    }

    /// Create from a simple preset.
    pub fn from_preset(preset: &str) -> Self {
        match preset {
            "strong" => Self {
                config: ConsistencyMatrixConfig {
                    cluster_size: 3,
                    mode: ConsistencyMode::Cp,
                    ..Default::default()
                },
                read_level: ConsistencyLevel::Strong,
                write_level: ConsistencyLevel::Strong,
                metadata_level: ConsistencyLevel::Strong,
                heartbeat_level: ConsistencyLevel::Sequential,
                quorum_size: 2,
                staleness_bound_ms: 0,
            },
            "session" => Self::default(),
            "causal" => Self {
                config: ConsistencyMatrixConfig {
                    cluster_size: 3,
                    ..Default::default()
                },
                read_level: ConsistencyLevel::Causal,
                write_level: ConsistencyLevel::Causal,
                metadata_level: ConsistencyLevel::Causal,
                heartbeat_level: ConsistencyLevel::Session,
                quorum_size: 2,
                staleness_bound_ms: 2000,
            },
            "eventual" => Self {
                config: ConsistencyMatrixConfig {
                    cluster_size: 3,
                    mode: ConsistencyMode::Ap,
                    strict_quorum: false,
                    ..Default::default()
                },
                read_level: ConsistencyLevel::Eventual,
                write_level: ConsistencyLevel::Eventual,
                metadata_level: ConsistencyLevel::Eventual,
                heartbeat_level: ConsistencyLevel::Eventual,
                quorum_size: 1,
                staleness_bound_ms: 5000,
            },
            _ => Self::default(),
        }
    }

    /// Set read consistency level.
    pub fn with_read_level(mut self, level: ConsistencyLevel) -> Self {
        self.read_level = level;
        self.recompute();
        self
    }

    /// Set write consistency level.
    pub fn with_write_level(mut self, level: ConsistencyLevel) -> Self {
        self.write_level = level;
        self.recompute();
        self
    }

    /// Set staleness bound in milliseconds.
    pub fn with_staleness_bound_ms(mut self, ms: u64) -> Self {
        self.staleness_bound_ms = ms;
        self
    }

    /// Set quorum size.
    pub fn with_quorum_size(mut self, size: usize) -> Self {
        self.quorum_size = size;
        self
    }

    /// Set a specific dimension's level.
    pub fn with_dimension_level(mut self, dim: MatrixDimension, level: ConsistencyLevel) -> Self {
        match dim {
            MatrixDimension::Read => self.read_level = level,
            MatrixDimension::Write => self.write_level = level,
            MatrixDimension::Metadata => self.metadata_level = level,
            MatrixDimension::Heartbeat => self.heartbeat_level = level,
        }
        self.recompute();
        self
    }

    /// Set the consistency mode.
    pub fn with_mode(mut self, mode: ConsistencyMode) -> Self {
        self.config.mode = mode;
        self.recompute();
        self
    }

    fn recompute(&mut self) {
        // Quorum is the max of read/write levels
        let max_level = self.read_level.max(self.write_level);
        if self.quorum_size == 0 {
            self.quorum_size = (self.config.cluster_size / 2) + 1;
            if max_level == ConsistencyLevel::Strong || max_level == ConsistencyLevel::Sequential {
                self.quorum_size = self.config.cluster_size;
            }
        }
    }

    /// Get the read consistency level.
    pub fn get_read_level(&self) -> ConsistencyLevel {
        self.read_level
    }

    /// Get the write consistency level.
    pub fn get_write_level(&self) -> ConsistencyLevel {
        self.write_level
    }

    /// Get the metadata consistency level.
    pub fn get_metadata_level(&self) -> ConsistencyLevel {
        self.metadata_level
    }

    /// Get the heartbeat consistency level.
    pub fn get_heartbeat_level(&self) -> ConsistencyLevel {
        self.heartbeat_level
    }

    /// Get effective quorum size for cluster.
    pub fn get_quorum_size(&self) -> usize {
        self.quorum_size
    }

    /// Get staleness bound in ms.
    pub fn get_staleness_bound_ms(&self) -> u64 {
        self.staleness_bound_ms
    }

    /// Get level for a specific dimension.
    pub fn get_dimension_level(&self, dim: MatrixDimension) -> ConsistencyLevel {
        match dim {
            MatrixDimension::Read => self.read_level,
            MatrixDimension::Write => self.write_level,
            MatrixDimension::Metadata => self.metadata_level,
            MatrixDimension::Heartbeat => self.heartbeat_level,
        }
    }

    /// Check if a read operation at given timestamp is fresh enough.
    pub fn is_read_fresh(&self, timestamp_ms: u64, now_ms: u64) -> bool {
        if self.staleness_bound_ms == 0 {
            return true; // No bound = always fresh at strong level
        }
        (now_ms - timestamp_ms) <= self.staleness_bound_ms
    }

    /// Get the consistency mode.
    pub fn get_mode(&self) -> ConsistencyMode {
        self.config.mode
    }

    /// Check if we can tolerate network partitions.
    pub fn tolerates_partition(&self) -> bool {
        self.config.mode == ConsistencyMode::Ap
    }

    /// Verify quorum for a given number of acks.
    pub fn verify_quorum(&self, acks: usize) -> bool {
        if self.config.strict_quorum {
            acks >= self.quorum_size
        } else {
            acks >= 1
        }
    }

    /// Get a summary of all dimension levels.
    pub fn summary(&self) -> HashMap<MatrixDimension, ConsistencyLevel> {
        HashMap::from([
            (MatrixDimension::Read, self.read_level),
            (MatrixDimension::Write, self.write_level),
            (MatrixDimension::Metadata, self.metadata_level),
            (MatrixDimension::Heartbeat, self.heartbeat_level),
        ])
    }

    /// Convert to config for serialization/persistence.
    pub fn to_config(&self) -> ConsistencyMatrixConfig {
        let mut overrides = HashMap::new();
        overrides.insert(
            MatrixDimension::Read,
            DimensionConfig {
                level: self.read_level,
                quorum_size: self.quorum_size,
                staleness_bound_ms: self.staleness_bound_ms,
                timeout_ms: 5000,
            },
        );
        overrides.insert(
            MatrixDimension::Write,
            DimensionConfig {
                level: self.write_level,
                quorum_size: 0,
                staleness_bound_ms: 0,
                timeout_ms: 5000,
            },
        );
        overrides.insert(
            MatrixDimension::Metadata,
            DimensionConfig {
                level: self.metadata_level,
                quorum_size: 0,
                staleness_bound_ms: 0,
                timeout_ms: 3000,
            },
        );
        overrides.insert(
            MatrixDimension::Heartbeat,
            DimensionConfig {
                level: self.heartbeat_level,
                quorum_size: 1,
                staleness_bound_ms: 0,
                timeout_ms: 1000,
            },
        );

        ConsistencyMatrixConfig {
            cluster_size: self.config.cluster_size,
            mode: self.config.mode,
            dimension_overrides: overrides,
            global_staleness_bound_ms: self.staleness_bound_ms,
            strict_quorum: self.config.strict_quorum,
        }
    }
}

/// Thread-safe wrapper for async environments.
pub type SharedConsistencyMatrix = Arc<StdRwLock<ConsistencyMatrix>>;

/// Create a shared matrix.
pub fn shared(matrix: ConsistencyMatrix) -> SharedConsistencyMatrix {
    Arc::new(StdRwLock::new(matrix))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_matrix_levels() {
        let matrix = ConsistencyMatrix::default();
        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Session);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Strong);
        assert!(matrix.get_read_level() < matrix.get_write_level());
    }

    #[test]
    fn test_preset_strong() {
        let matrix = ConsistencyMatrix::from_preset("strong");
        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Strong);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Strong);
        assert_eq!(matrix.get_mode(), ConsistencyMode::Cp);
    }

    #[test]
    fn test_preset_eventual() {
        let matrix = ConsistencyMatrix::from_preset("eventual");
        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Eventual);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Eventual);
        assert_eq!(matrix.get_mode(), ConsistencyMode::Ap);
        assert!(!matrix.config.strict_quorum);
    }

    #[test]
    fn test_preset_causal() {
        let matrix = ConsistencyMatrix::from_preset("causal");
        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Causal);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Causal);
        assert_eq!(matrix.get_heartbeat_level(), ConsistencyLevel::Session);
    }

    #[test]
    fn test_fluent_builder() {
        let matrix = ConsistencyMatrix::default()
            .with_read_level(ConsistencyLevel::Strong)
            .with_write_level(ConsistencyLevel::Sequential)
            .with_staleness_bound_ms(500)
            .with_quorum_size(3);

        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Strong);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Sequential);
        assert_eq!(matrix.get_staleness_bound_ms(), 500);
        assert_eq!(matrix.get_quorum_size(), 3);
    }

    #[test]
    fn test_dimension_levels() {
        let matrix = ConsistencyMatrix::default()
            .with_dimension_level(MatrixDimension::Heartbeat, ConsistencyLevel::Eventual);

        assert_eq!(matrix.get_heartbeat_level(), ConsistencyLevel::Eventual);
        assert_eq!(
            matrix.get_dimension_level(MatrixDimension::Read),
            ConsistencyLevel::Session
        );
    }

    #[test]
    fn test_read_freshness_check() {
        let matrix = ConsistencyMatrix::default().with_staleness_bound_ms(1000);
        // Fresh: within bound
        assert!(matrix.is_read_fresh(9000, 9500));
        // Stale: beyond bound
        assert!(!matrix.is_read_fresh(8000, 9500));
        // No bound
        let no_bound = ConsistencyMatrix::default().with_staleness_bound_ms(0);
        assert!(no_bound.is_read_fresh(0, 999999999));
    }

    #[test]
    fn test_quorum_verification() {
        let matrix = ConsistencyMatrix::default().with_quorum_size(3);
        assert!(matrix.verify_quorum(3));
        assert!(matrix.verify_quorum(4));
        assert!(!matrix.verify_quorum(2));

        // Non-strict mode
        let matrix2 = ConsistencyMatrix::new(ConsistencyMatrixConfig {
            cluster_size: 5,
            mode: ConsistencyMode::Ap,
            strict_quorum: false,
            ..Default::default()
        })
        .with_quorum_size(3);
        assert!(matrix2.verify_quorum(1));
    }

    #[test]
    fn test_consistency_level_ordering() {
        assert!(ConsistencyLevel::Strong > ConsistencyLevel::Sequential);
        assert!(ConsistencyLevel::Sequential > ConsistencyLevel::Causal);
        assert!(ConsistencyLevel::Causal > ConsistencyLevel::Session);
        assert!(ConsistencyLevel::Session > ConsistencyLevel::Eventual);
    }

    #[test]
    fn test_level_guarantees() {
        assert!(ConsistencyLevel::Strong.guarantees_read_your_writes());
        assert!(ConsistencyLevel::Session.guarantees_read_your_writes());
        assert!(ConsistencyLevel::Sequential.guarantees_read_your_writes());
        assert!(!ConsistencyLevel::Eventual.guarantees_read_your_writes());
        assert!(!ConsistencyLevel::Eventual.guarantees_monotonic_reads());
        assert!(ConsistencyLevel::Strong.guarantees_monotonic_reads());
    }

    #[test]
    fn test_replication_factor() {
        assert_eq!(ConsistencyLevel::Strong.min_replication_factor(), 3);
        assert_eq!(ConsistencyLevel::Causal.min_replication_factor(), 2);
        assert_eq!(ConsistencyLevel::Eventual.min_replication_factor(), 1);
    }

    #[test]
    fn test_summary() {
        let summary = ConsistencyMatrix::default().summary();
        assert_eq!(summary.len(), 4);
        assert_eq!(summary[&MatrixDimension::Read], ConsistencyLevel::Session);
        assert_eq!(summary[&MatrixDimension::Write], ConsistencyLevel::Strong);
    }

    #[test]
    fn test_to_config_roundtrip() {
        let matrix = ConsistencyMatrix::default()
            .with_read_level(ConsistencyLevel::Causal)
            .with_quorum_size(2);

        let config = matrix.to_config();
        assert_eq!(config.global_staleness_bound_ms, 1000);
        assert!(config.strict_quorum);
    }

    #[test]
    fn test_tolerates_partition() {
        let cp = ConsistencyMatrix::new(ConsistencyMatrixConfig {
            mode: ConsistencyMode::Cp,
            ..Default::default()
        });
        let ap = ConsistencyMatrix::new(ConsistencyMatrixConfig {
            mode: ConsistencyMode::Ap,
            ..Default::default()
        });

        assert!(!cp.tolerates_partition());
        assert!(ap.tolerates_partition());
    }

    #[test]
    fn test_shared_matrix() {
        let matrix = shared(ConsistencyMatrix::default());
        assert_eq!(
            matrix.read().unwrap().get_read_level(),
            ConsistencyLevel::Session
        );
    }

    #[test]
    fn test_matrix_display() {
        let matrix = ConsistencyMatrix::default();
        let summary = format!("{:?}", matrix);
        assert!(summary.contains("ConsistencyMatrix"));
    }

    #[test]
    fn test_consistency_level_display() {
        assert_eq!(format!("{}", ConsistencyLevel::Strong), "Strong");
        assert_eq!(format!("{}", ConsistencyLevel::Eventual), "Eventual");
    }

    #[test]
    fn test_dimension_display() {
        assert_eq!(format!("{}", MatrixDimension::Read), "Read");
        assert_eq!(format!("{}", MatrixDimension::Heartbeat), "Heartbeat");
    }

    #[test]
    fn test_dimension_config_effective_quorum() {
        let cfg = DimensionConfig::default();
        assert_eq!(cfg.effective_quorum(5), 3); // (5/2)+1 = 3

        let custom = DimensionConfig {
            quorum_size: 2,
            ..Default::default()
        };
        assert_eq!(custom.effective_quorum(5), 2);
    }

    #[test]
    fn test_unknown_preset_falls_back_to_default() {
        let matrix = ConsistencyMatrix::from_preset("unknown_preset");
        assert_eq!(matrix.get_read_level(), ConsistencyLevel::Session);
        assert_eq!(matrix.get_write_level(), ConsistencyLevel::Strong);
    }

    #[test]
    fn test_cluster_size_affects_quorum() {
        let matrix = ConsistencyMatrix::new(ConsistencyMatrixConfig {
            cluster_size: 5,
            ..Default::default()
        });
        // Default quorum should be (5/2)+1 = 3
        assert!(matrix.get_quorum_size() >= 3);
    }
}
