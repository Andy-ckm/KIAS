//! # KIAS Benchmarks
//!
//! Performance benchmarks and stress tests for the KIAS scheduling system.
//!
//! ## Benchmark Suites
//!
//! - `scheduler_bench` — Single-agent and batch scheduling across all 7 algorithms
//!   (round-robin, least-loaded, resource-aware, cache-aware, affinity,
//!   priority-aware, gpu-aware)
//! - `controller_bench` — Controller state operations throughput and reconciliation loop
//! - `concurrent_stress` — High-concurrency scheduling stress test
//! - `workflow_bench` — Workflow engine DAG execution (linear, wide, branching)
//!
//! ## Running
//!
//! ```bash
//! cargo bench -p kias-benchmarks           # All benchmarks
//! cargo bench -p kias-benchmarks -- scheduler  # Scheduler only
//! make bench                                 # Via Makefile
//! ```

pub mod fixture_gen;
pub mod fixtures;
