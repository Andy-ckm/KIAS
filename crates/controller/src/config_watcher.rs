
//! config_watcher.rs
//!
//! A lightweight, thread‑safe configuration watcher that:
//! * monitors a single file for changes,
//! * hot‑reloads the file when it changes,
//! * validates the new configuration,
//! * rolls back to the last known‑good version on validation failure.
//!
//! The crate depends on `notify` (file system events) and `serde`/`serde_json`
//! for serialization/deserialization of the configuration.
//!
//!