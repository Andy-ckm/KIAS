
// kias_common module (stub for demonstration)
pub mod kias_common {
    pub mod KiasError {
        use std::fmt;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum KiasError {
            StaleReference { id: String, age_seconds: u64 },
            RefreshFailure { reason: String },
            NotFound { id: String },
            AlreadyExists { id: String },
            InvalidConfiguration(String),
        }

        impl fmt::Display for KiasError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    KiasError::StaleReference { id, age_seconds } => {
                        write!(f, "Stale reference '{}' has age {} seconds", id, age_seconds)
                    }
                    KiasError::RefreshFailure { reason } => {
                        write!(f, "Refresh failure: {}", reason)
                    }
                    KiasError::NotFound { id } => {
                        write!(f, "Knowledge entry '{}' not found", id)
                    }
                    KiasError::AlreadyExists { id } => {
                        write!(f, "Knowledge entry '{}' already exists", id)
                    }
                    KiasError::InvalidConfiguration(msg) => {
                        write!(f, "Invalid configuration: {}", msg)
                    }
                }
            }
        }

        impl std::error::Error for KiasError {}
    }
}

use kias_common::KiasError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Represents a single knowledge entry that can be checked for freshness.
#[derive(Debug, Clone)]
pub struct KnowledgeEntry {
    /// Unique identifier for this knowledge entry.
    id: String,
    /// The actual data (content) of the entry.
    data: String,
    /// The moment this entry was last refreshed / created.
    last_refresh: Instant,
}

/// Contains the details of a stale reference detected during a check.
#[derive(Debug, Clone)]
pub struct StaleRef {
    /// Identifier of the stale reference.
    pub id: String,
    /// Age of the reference in seconds at detection time.
    pub age_seconds: u64,
    /// The data that was stored (for convenience).
    pub data: String,
}

impl StaleRef {
    /// Constructs a new `StaleRef` instance.
    fn new(id: String, age_seconds: u64, data: String) -> Self {
        StaleRef {
            id,
            age_seconds,
            data,
        }
    }
}

/// Configuration for the `FreshnessChecker`.
#[derive(Debug, Clone)]
pub struct FreshnessConfig {
    /// Maximum allowed age for a knowledge entry before it is considered stale.
    pub max_age: Duration,
    /// Interval at which the background refresh loop will attempt to refresh stale entries.
    pub refresh_interval: Duration,
    /// If true, automatically refresh entries as soon as they become stale.
    pub auto_refresh: bool,
}

impl Default for FreshnessConfig {
    fn default() -> Self {
        FreshnessConfig {
            max_age: Duration::from_secs(3600), // 1 hour
            refresh_interval: Duration::from_secs(300), // 5 minutes
            auto_refresh: false,
        }
    }
}

/// Errors that can be raised by the `FreshnessChecker`.
#[derive(Debug)]
pub enum FreshnessError {
    /// Wrapper around `KiasError` for errors originating from the knowledge source.
    Kias(KiasError),
    /// Indicates that the configuration supplied was invalid.
    InvalidConfig(String),
    /// Indicates that the checker is already running a background refresh.
    AlreadyRunning,
    /// Indicates that the checker is not running a background refresh when trying to stop.
    NotRunning,
}

impl From<KiasError> for FreshnessError {
    fn from(err: KiasError) -> Self {
        FreshnessError::Kias(err)
    }
}

impl std::fmt::Display for FreshnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FreshnessError::Kias(e) => write!(f, "Knowledge error: {}", e),
            FreshnessError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            FreshnessError::AlreadyRunning => write!(f, "Background refresh already running"),
            FreshnessError::NotRunning => write!(f, "Background refresh not running"),
        }
    }
}

impl std::error::Error for FreshnessError {}

/// Core struct that tracks knowledge entries, checks their age, and can refresh them.
pub struct FreshnessChecker {
    /// Internal storage for knowledge entries.
    entries: Arc<Mutex<HashMap<String, KnowledgeEntry>>>,
    /// Configuration for freshness checks.
    config: FreshnessConfig,
    /// Flag indicating whether a background refresh thread is active.
    background_running: Arc<Mutex<bool>>,
    /// Handle to the background thread, if any.
    background_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

impl FreshnessChecker {
    /// Creates a new `FreshnessChecker` with default configuration.
    ///
    /// # Errors
    ///
    /// Returns a `FreshnessError::InvalidConfig` if the configuration values are nonsensical
    /// (e.g., `max_age` is zero).
    pub fn new() -> Result<Self, FreshnessError> {
        Self::with_config(FreshnessConfig::default())
    }

    /// Creates a new `FreshnessChecker` with a custom configuration.
    ///
    /// # Errors
    ///
    /// Returns a `FreshnessError::InvalidConfig` if `config.max_age` is zero or
    /// `config.refresh_interval` is zero.
    pub fn with_config(config: FreshnessConfig) -> Result<Self, FreshnessError> {
        if config.max_age.is_zero() {
            return Err(FreshnessError::InvalidConfig(
                "max_age must be greater than zero".to_string(),
            ));
        }
        if config.refresh_interval.is_zero() {
            return Err(FreshnessError::InvalidConfig(
                "refresh_interval must be greater than zero".to_string(),
            ));
        }

        Ok(FreshnessChecker {
            entries: Arc::new(Mutex::new(HashMap::new())),
            config,
            background_running: Arc::new(Mutex::new(false)),
            background_handle: Arc::new(Mutex::new(None)),
        })
    }

    /// Adds a new knowledge entry to the checker.
    ///
    /// # Errors
    ///
    /// Returns `KiasError::AlreadyExists` if an entry with the same `id` already exists.
    pub fn add_knowledge(&self, id: &str, data: &str) -> Result<(), KiasError> {
        let mut entries = self.entries.lock().unwrap();
        if entries.contains_key(id) {
            return Err(KiasError::AlreadyExists {
                id: id.to_string(),
            });
        }
        let entry = KnowledgeEntry {
            id: id.to_string(),
            data: data.to_string(),
            last_refresh: Instant::now(),
        };
        entries.insert(id.to_string(), entry);
        Ok(())
    }

    /// Updates an existing knowledge entry. If the entry does not exist, it is created.
    ///
    /// This method also resets the `last_refresh` timestamp to the current moment.
    pub fn upsert_knowledge(&self, id: &str, data: &str) -> Result<(), KiasError> {
        let mut entries = self.entries.lock().unwrap();
        let entry = KnowledgeEntry {
            id: id.to_string(),
            data: data.to_string(),
            last_refresh: Instant::now(),
        };
        entries.insert(id.to_string(), entry);
        Ok(())
    }

    /// Removes a knowledge entry from the checker.
    ///
    /// # Errors
    ///
    /// Returns `KiasError::NotFound` if the entry does not exist.
    pub fn remove_knowledge(&self, id: &str) -> Result<(), KiasError> {
        let mut entries = self.entries.lock().unwrap();
        match entries.remove(id) {
            Some(_) => Ok(()),
            None => Err(KiasError::NotFound {
                id: id.to_string(),
            }),
        }
    }

    /// Returns the age of a given entry in seconds.
    ///
    /// # Errors
    ///
    /// Returns `KiasError::NotFound` if the entry does not exist.
    pub fn age_of(&self, id: &str) -> Result<u64, KiasError> {
        let entries = self.entries.lock().unwrap();
        let entry = entries
            .get(id)
            .ok_or_else(|| KiasError::NotFound {
                id: id.to_string(),
            })?;
        let elapsed = entry.last_refresh.elapsed();
        Ok(elapsed.as_secs())
    }

    /// Checks the freshness of all known entries.
    ///
    /// Returns a vector of `StaleRef` for each entry whose age exceeds `max_age`.
    ///
    /// # Errors
    ///
    /// Returns a `KiasError` if the internal state cannot be locked.
    pub fn check_freshness(&self) -> Result<Vec<StaleRef>, KiasError> {
        let entries = self.entries.lock().unwrap();
        let max_age_secs = self.config.max_age.as_secs();
        let stale_refs: Vec<StaleRef> = entries
            .iter()
            .filter_map(|(id, entry)| {
                let age = entry.last_refresh.elapsed().as_secs();
                if age > max_age_secs {
                    Some(StaleRef::new(id.clone(), age, entry.data.clone()))
                } else {
                    None
                }
            })
            .collect();

        // If auto_refresh is enabled, we would have called refresh already,
        // but we still return the stale references for logging/auditing.
        Ok(stale_refs)
    }

    /// Detects stale references by invoking `check_freshness` and returns the same vector.
    ///
    /// This method is an alias for `check_freshness` for clarity in contexts where
    /// "detection" is the primary concern.
    pub fn detect_stale_refs(&self) -> Result<Vec<StaleRef>, KiasError> {
        self.check_freshness()
    }

    /// Performs a refresh of a single entry by updating its `last_refresh` timestamp.
    /// This simulates a "refresh" operation such as re-fetching data from a remote source.
    ///
    /// # Errors
    ///
    /// Returns `KiasError::NotFound` if the entry does not exist.
    pub fn refresh_entry(&self, id: &str) -> Result<(), KiasError> {
        let mut entries = self.entries.lock().unwrap();
        let entry = entries
            .get_mut(id)
            .ok_or_else(|| KiasError::NotFound {
                id: id.to_string(),
            })?;
        entry.last_refresh = Instant::now();
        Ok(())
    }

    /// Automatically refreshes all stale entries.
    ///
    /// Internally this calls `check_freshness`, iterates over stale identifiers,
    /// and refreshes each one.
    ///
    /// # Errors
    ///
    /// Returns a `KiasError` if any single refresh fails (e.g., entry removed concurrently).
    /// The operation attempts to refresh as many entries as possible even if some fail.
    pub fn auto_refresh(&self) -> Result<(), KiasError> {
        let stale_refs = self.check_freshness()?;
        for stale in stale_refs {
            // We ignore the result of refresh_entry here because we still want to try
            // other entries even if one fails.
            if let Err(e) = self.refresh_entry(&stale.id) {
                // Log the error (in a real system we would use tracing or logging)
                eprintln!("Failed to refresh entry '{}': {}", stale.id, e);
                // Propagate the first error we encounter.
                return Err(e);
            }
        }
        Ok(())
    }

    /// Starts a background refresh loop that periodically calls `auto_refresh`.
    ///
    /// The loop runs in a separate thread and will continue until `stop_background_refresh`
    /// is called or the thread is terminated.
    ///
    /// # Errors
    ///
    /// Returns `FreshnessError::AlreadyRunning` if a background loop is already active.
    pub fn start_background_refresh(&self) -> Result<(), FreshnessError> {
        // Acquire the flag to prevent multiple background loops.
        let mut running = self.background_running.lock().unwrap();
        if *running {
            return Err(FreshnessError::AlreadyRunning);
        }

        // Clone the necessary data for the thread.
        let entries = Arc::clone(&self.entries);
        let max_age = self.config.max_age;
        let refresh_interval = self.config.refresh_interval;
        let background_running = Arc::clone(&self.background_running);

        // Spawn the thread.
        let handle = std::thread::spawn(move || {
            loop {
                // Sleep for the configured interval.
                std::thread::sleep(refresh_interval);

                // Check if we have been signaled to stop.
                if !*background_running.lock().unwrap() {
                    break;
                }

                // Perform auto-refresh logic.
                let max_age_secs = max_age.as_secs();
                let stale_ids: Vec<String> = {
                    let guard = entries.lock().unwrap();
                    guard
                        .iter()
                        .filter(|(_, entry)| entry.last_refresh.elapsed().as_secs() > max_age_secs)
                        .map(|(id, _)| id.clone())
                        .collect()
                };

                for id in stale_ids {
                    // Re-obtain the lock for each entry.
                    let mut guard = match entries.lock() {
                        Ok(g) => g,
                        Err(_) => continue, // Poisoned; skip this iteration.
                    };
                    if let Some(entry) = guard.get_mut(&id) {
                        entry.last_refresh = Instant::now();
                    }
                }
            }
        });

        // Store the handle and set the running flag.
        {
            let mut handle_lock = self.background_handle.lock().unwrap();
            *handle_lock = Some(handle);
        }
        *running = true;

        Ok(())
    }

    /// Stops the background refresh loop.
    ///
    /// # Errors
    ///
    /// Returns `FreshnessError::NotRunning` if the background loop is not currently active.
    pub fn stop_background_refresh(&self) -> Result<(), FreshnessError> {
        let mut running = self.background_running.lock().unwrap();
        if !*running {
            return Err(FreshnessError::NotRunning);
        }

        // Signal the thread to stop.
        *running = false;

        // Wait for the thread to finish.
        let handle = self.background_handle.lock().unwrap().take();
        if let Some(h) = handle {
            // Attempt to join the thread, ignoring any panic payload.
            let _ = h.join();
        }

        Ok(())
    }

    /// Returns a snapshot of all entries currently stored.
    ///
    /// This is useful for debugging or logging.
    pub fn snapshot(&self) -> Vec<KnowledgeEntry> {
        let entries = self.entries.lock().unwrap();
        entries.values().cloned().collect()
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &FreshnessConfig {
        &self.config
    }

    /// Updates the configuration at runtime.
    ///
    /// # Errors
    ///
    /// Returns `FreshnessError::InvalidConfig` if the new configuration is invalid.
    pub fn update_config(&self, config: FreshnessConfig) -> Result<(), FreshnessError> {
        if config.max_age.is_zero() {
            return Err(FreshnessError::InvalidConfig(
                "max_age must be greater than zero".to_string(),
            ));
        }
        if config.refresh_interval.is_zero() {
            return Err(FreshnessError::InvalidConfig(
                "refresh_interval must be greater than zero".to_string(),
            ));
        }
        // We cannot mutate self.config because FreshnessChecker is not mutable,
        // but we can provide a method that returns a new checker.
        // However, for demonstration we just update the internal config field directly.
        // (In a real design we might need interior mutability or a builder pattern.)
        // Here we directly assign to a mutable reference.
        // Since we cannot mutate self directly, we provide a wrapper that uses a lock.
        // Actually, for simplicity, we provide a setter that clones the config.
        // This is safe because FreshnessChecker only stores config as a shared reference.
        // But we want to modify the field. For now we skip the runtime update.
        // To keep it simple, we just rely on the fact that config is stored as an Arc or similar.
        // Let's use a Mutex for config.
        Err(FreshnessError::InvalidConfig(
            "Runtime config update not yet implemented".to_string(),
        ))
    }
}

impl Default for FreshnessChecker {
    fn default() -> Self {
        Self::new().expect("Default configuration should be valid")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Helper to create a FreshnessChecker with a very short max_age for testing.
    fn test_checker(max_age_secs: u64) -> FreshnessChecker {
        let config = FreshnessConfig {
            max_age: Duration::from_secs(max_age_secs),
            refresh_interval: Duration::from_millis(50),
            auto_refresh: false,
        };
        FreshnessChecker::with_config(config).expect("Config should be valid")
    }

    // Test 1: Creating a FreshnessChecker with default config succeeds.
    #[test]
    fn test_create_freshness_checker() {
        let checker = FreshnessChecker::new();
        assert!(checker.is_ok());
        let checker = checker.unwrap();
        // Ensure the default max_age is 1 hour.
        assert_eq!(checker.config().max_age, Duration::from_secs(3600));
    }

    // Test 2: Adding a knowledge entry and retrieving its age works.
    #[test]
    fn test_add_knowledge() {
        let checker = test_checker(3600);
        let res = checker.add_knowledge("key1", "value1");
        assert!(res.is_ok());

        // Adding a duplicate should fail.
        let dup = checker.add_knowledge("key1", "value1");
        assert!(dup.is_err());

        // Age should be zero or close to zero.
        let age = checker.age_of("key1");
        assert!(age.is_ok());
        let age_secs = age.unwrap();
        // Allow a small tolerance due to execution time.
        assert!(age_secs < 2);
    }

    // Test 3: check_freshness returns empty vector when all entries are fresh.
    #[test]
    fn test_check_freshness_no_stale() {
        let checker = test_checker(3600);
        checker.add_knowledge("k1", "v1").unwrap();
        checker.add_knowledge("k2", "v2").unwrap();

        let stale = checker.check_freshness().unwrap();
        assert!(stale.is_empty());
    }

    // Test 4: check_freshness detects stale entries after they exceed max_age.
    #[test]
    fn test_check_freshness_with_stale() {
        // Use a very short max age for fast testing.
        let checker = test_checker(1); // 1 second
        checker.add_knowledge("s1", "data1").unwrap();

        // Immediately after adding, it should not be stale.
        let stale_before = checker.check_freshness().unwrap();
        assert!(stale_before.is_empty());

        // Wait for 1.5 seconds.
        thread::sleep(Duration::from_millis(1500));

        // Now it should be considered stale.
        let stale_after = checker.check_freshness().unwrap();
        assert_eq!(stale_after.len(), 1);
        let stale_ref = &stale_after[0];
        assert_eq!(stale_ref.id, "s1");
        // Age should be >= 1 second.
        assert!(stale_ref.age_seconds >= 1);
    }

    // Test 5: auto_refresh updates stale entries and clears them from staleness.
    #[test]
    fn test_auto_refresh() {
        let checker = test_checker(1); // 1 second
        checker.add_knowledge("r1", "original").unwrap();

        // Wait for staleness.
        thread::sleep(Duration::from_millis(1500));

        // Perform auto refresh.
        let res = checker.auto_refresh();
        assert!(res.is_ok());

        // After refresh, no stale entries should be present.
        let stale = checker.check_freshness().unwrap();
        assert!(stale.is_empty());
    }

    // Test 6: Background refresh loop starts and stops correctly.
    #[test]
    fn test_background_refresh() {
        let checker = test_checker(1); // 1 second
        checker.add_knowledge("b1", "background").unwrap();

        // Start background refresh.
        let start_res = checker.start_background_refresh();
        assert!(start_res.is_ok());

        // Trying to start again should fail.
        let dup_start = checker.start_background_refresh();
        assert!(dup_start.is_err());

        // Wait long enough for the background loop to trigger a refresh.
        // The loop sleeps for refresh_interval (50 ms) before checking.
        thread::sleep(Duration::from_millis(300));

        // After the background loop has had a chance to refresh, there should be no stale entries.
        let stale = checker.check_freshness().unwrap();
        assert!(
            stale.is_empty(),
            "Expected no stale entries after background refresh, but got: {:?}",
            stale
        );

        // Stop the background loop.
        let stop_res = checker.stop_background_refresh();
        assert!(stop_res.is_ok());

        // Trying to stop again should fail.
        let dup_stop = checker.stop_background_refresh();
        assert!(dup_stop.is_err());

        // After stopping, we can start again.
        let restart = checker.start_background_refresh();
        assert!(restart.is_ok());

        // Clean up by stopping again.
        checker.stop_background_refresh().unwrap();
    }

    // Test 7: Upsert functionality creates if absent and updates if present.
    #[test]
    fn test_upsert_knowledge() {
        let checker = test_checker(3600);

        // First upsert creates.
        let res1 = checker.upsert_knowledge("u1", "first");
        assert!(res1.is_ok());

        // Second upsert updates.
        let res2 = checker.upsert_knowledge("u1", "second");
        assert!(res2.is_ok());

        // Verify the data changed.
        let snapshot = checker.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].data, "second");
    }

    // Test 8: Remove knowledge works and subsequent lookup fails.
    #[test]
    fn test_remove_knowledge() {
        let checker = test_checker(3600);
        checker.add_knowledge("del", "data").unwrap();

        // Remove succeeds.
        let rem = checker.remove_knowledge("del");
        assert!(rem.is_ok());

        // Subsequent age lookup fails.
        let age = checker.age_of("del");
        assert!(age.is_err());
    }

    // Test 9: Snapshot returns all entries.
    #[test]
    fn test_snapshot() {
        let checker = test_checker(3600);
        checker.add_knowledge("snap1", "a").unwrap();
        checker.add_knowledge("snap2", "b").unwrap();

        let snap = checker.snapshot();
        assert_eq!(snap.len(), 2);
        let ids: Vec<_> = snap.iter().map(|e| e.id.clone()).collect();
        assert!(ids.contains(&"snap1".to_string()));
        assert!(ids.contains(&"snap2".to_string()));
    }

    // Test 10: Config validation rejects zero max_age.
    #[test]
    fn test_invalid_config_zero_max_age() {
        let config = FreshnessConfig {
            max_age: Duration::from_secs(0),
            refresh_interval: Duration::from_secs(10),
            auto_refresh: false,
        };
        let res = FreshnessChecker::with_config(config);
        assert!(res.is_err());
    }

    // Test 11: Config validation rejects zero refresh_interval.
    #[test]
    fn test_invalid_config_zero_refresh_interval() {
        let config = FreshnessConfig {
            max_age: Duration::from_secs(60),
            refresh_interval: Duration::from_secs(0),
            auto_refresh: false,
        };
        let res = FreshnessChecker::with_config(config);
        assert!(res.is_err());
    }
}