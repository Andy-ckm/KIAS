//! # Bufferable Session Service
//!
//! Write buffering + batch flushing to persistence, with WAL (Write-Ahead Log)
//! crash-recovery guarantee.
//!
//! ## Core types
//!
//! - [`SessionBuffer`] — generic buffered writer that wraps a persistence closure.
//! - [`FlushStrategy`] — enum selecting how/when flushes are triggered.
//! - [`WalEntry`] — one serialized record written to the WAL before it enters the buffer.
//! - [`recover_from_wal`] — reads WAL entries on startup and replays them.
//!
//! ## WAL semantics
//!
//! Every record pushed into the buffer is first serialized and appended to the WAL
//! file (on disk). Only after the WAL write succeeds does the record enter the
//! in-memory buffer. On startup the WAL is read and all un-flushed entries are
//! re-loaded into the buffer before any new writes are accepted. This gives
//! at-least-once durability: a crash between WAL write and flush will be recovered
//! by replaying the WAL.
//!
//! ## Flush strategies
//!
//! | Strategy | Trigger |
//! |----------|---------|
//! | `Timer`  | Every `N` seconds since last flush (background task) |
//! | `Size`   | Buffer reaches `N` entries |
//! | `Manual` | Caller invokes `flush()` |
//! | `TimerAndSize` | Either condition |

use kias_common::{KiasError, KiasResult};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fmt::Debug,
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::RwLock;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// A serializable WAL record with a sequence number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry<T> {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Serialized payload.
    pub data: Vec<u8>,
    #[serde(skip)]
    _marker: std::marker::PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> WalEntry<T> {
    /// Serialize a value into a WAL entry.
    pub fn new(seq: u64, value: &T) -> KiasResult<Self> {
        let data = serde_json::to_vec(value)
            .map_err(|e| KiasError::Config(format!("WAL encode error: {e}")))?;
        Ok(Self {
            seq,
            data,
            _marker: std::marker::PhantomData,
        })
    }

    /// Deserialize the payload back to `T`.
    pub fn deserialize(&self) -> KiasResult<T> {
        serde_json::from_slice(&self.data)
            .map_err(|e| KiasError::Config(format!("WAL decode error: {e}")))
    }
}

/// Controls when automatic flushes are triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushStrategy {
    /// Flush disabled; only manual `flush()` triggers persistence.
    Manual,
    /// Flush after `N` seconds since last flush.
    Timer { interval_secs: u64 },
    /// Flush when buffer reaches `N` entries.
    Size { threshold: usize },
    /// Flush when either timer fires or size threshold is hit.
    TimerAndSize {
        interval_secs: u64,
        threshold: usize,
    },
}

impl Default for FlushStrategy {
    fn default() -> Self {
        Self::TimerAndSize {
            interval_secs: 5,
            threshold: 100,
        }
    }
}

/// Statistics for a running `SessionBuffer`.
#[derive(Debug, Default, Clone)]
pub struct BufferStats {
    pub buffered_count: usize,
    pub total_flushed: u64,
    pub total_recovered: u64,
    pub flush_count: u64,
    pub wal_size_bytes: u64,
    pub last_flush_at: Option<Instant>,
}

/// The WAL file handle — append-only, created on first write.
#[derive(Debug)]
pub struct WalFile {
    path: PathBuf,
    end_offset: u64,
}

impl WalFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            end_offset: 0,
        }
    }

    /// Append a serialized WAL entry and fsync it to disk.
    pub fn append<T: Serialize>(&mut self, entry: &WalEntry<T>) -> KiasResult<()> {
        use std::io::Write;
        let bytes = serde_json::to_vec(entry)
            .map_err(|e| KiasError::Config(format!("WAL encode error: {e}")))?;
        let len_bytes = (bytes.len() as u64).to_le_bytes();

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(KiasError::Io)?;

        file.write_all(&len_bytes).map_err(KiasError::Io)?;
        file.write_all(&bytes).map_err(KiasError::Io)?;
        file.flush().map_err(KiasError::Io)?;
        file.sync_all().map_err(KiasError::Io)?;

        self.end_offset += 8 + bytes.len() as u64;
        Ok(())
    }

    /// Truncate WAL to length 0 (called after a successful flush).
    pub fn truncate(&mut self) -> KiasResult<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&self.path)
            .map_err(KiasError::Io)?;
        self.end_offset = 0;
        Ok(())
    }

    /// Read all WAL entries from disk and return them in order.
    pub fn read_all<T: DeserializeOwned>(&self) -> KiasResult<Vec<WalEntry<T>>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut file = std::fs::File::open(&self.path).map_err(KiasError::Io)?;
        let mut entries = Vec::new();
        let mut len_buf = [0u8; 8];

        loop {
            match std::io::Read::read(&mut file, &mut len_buf) {
                Ok(0) => break, // EOF
                Ok(8) => {}
                Ok(n) => {
                    return Err(KiasError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("WAL: expected 8-byte header, got {n}"),
                    )));
                }
                Err(e) => return Err(KiasError::Io(e)),
            }
            let len = u64::from_le_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            std::io::Read::read_exact(&mut file, &mut data).map_err(KiasError::Io)?;
            let entry: WalEntry<T> = serde_json::from_slice(&data)
                .map_err(|e| KiasError::Config(format!("WAL corrupt: {e}")))?;
            entries.push(entry);
        }
        Ok(entries)
    }

    #[allow(dead_code)]
    pub fn size_bytes(&self) -> u64 {
        std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0)
    }
}

/// Recover un-flushed entries from the WAL file and return them.
pub fn recover_from_wal<T: Serialize + DeserializeOwned + Debug>(
    wal_path: &PathBuf,
) -> KiasResult<Vec<T>> {
    let wal = WalFile::new(wal_path.clone());
    let entries: Vec<WalEntry<T>> = wal.read_all().unwrap_or_else(|e| {
        warn!("WAL read failed on startup, starting empty: {}", e);
        Vec::new()
    });
    info!("WAL recovery: found {} un-flushed entries", entries.len());
    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        match entry.deserialize() {
            Ok(item) => items.push(item),
            Err(e) => {
                error!(
                    "WAL recovery deserialize failed for entry {}: {}",
                    entry.seq, e
                );
            }
        }
    }
    Ok(items)
}

/// Type alias for the persist future — boxed so it can be stored in Arc<dyn>.
type PersistFut = Pin<Box<dyn Future<Output = KiasResult<()>> + Send>>;

/// The shared state behind `Arc<SessionBufferInner>`.
struct SessionBufferInner<T>
where
    T: Serialize + DeserializeOwned + Debug + Send + 'static,
{
    buffer: RwLock<VecDeque<T>>,
    wal: RwLock<WalFile>,
    strategy: FlushStrategy,
    /// The persist function: takes a batch of items and returns a future.
    persist_fn: Arc<dyn Fn(&[T]) -> PersistFut + Send + Sync>,
    stats: RwLock<BufferStats>,
    /// Atomic flag set by Drop to signal the timer task to stop.
    shutdown: Arc<AtomicBool>,
    next_seq: RwLock<u64>,
}

/// Generic buffered writer with WAL crash-recovery support.
///
/// Type parameters:
/// - `T` — the domain record type stored in the buffer.
pub struct SessionBuffer<T>
where
    T: Serialize + DeserializeOwned + Debug + Send + 'static,
{
    inner: Arc<SessionBufferInner<T>>,
}

impl<T> Clone for SessionBuffer<T>
where
    T: Serialize + DeserializeOwned + Debug + Send + 'static,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> SessionBuffer<T>
where
    T: Serialize + DeserializeOwned + Debug + Send + 'static,
{
    /// Create a new buffered session writer.
    ///
    /// `persist_fn` is called with a slice of all buffered items during a flush.
    /// The WAL is recovered from `wal_path` on startup so any unflushed entries
    /// from a previous crash are re-loaded before new writes are accepted.
    ///
    /// Requires `T: Sync` because the background timer task holds a shared reference.
    pub async fn new<S>(
        strategy: FlushStrategy,
        wal_path: S,
        persist_fn: impl Fn(&[T]) -> PersistFut + Send + Sync + 'static,
    ) -> KiasResult<Self>
    where
        T: Sync,
        S: Into<PathBuf>,
    {
        let wal_path: PathBuf = wal_path.into();
        let wal = WalFile::new(wal_path.clone());

        // Recovery: load any un-flushed entries from WAL
        let recovered: Vec<T> = {
            let entries: Vec<WalEntry<T>> = wal.read_all().unwrap_or_else(|_| Vec::new());
            info!("SessionBuffer recovered {} entries from WAL", entries.len());
            let mut items = Vec::with_capacity(entries.len());
            for entry in entries {
                match entry.deserialize() {
                    Ok(item) => items.push(item),
                    Err(e) => {
                        error!("WAL recovery failed for entry {}: {}", entry.seq, e);
                    }
                }
            }
            items
        };

        let buffer_len = recovered.len();
        let persist_fn_arc = Arc::new(persist_fn);
        let shutdown = Arc::new(AtomicBool::new(false));

        let inner = Arc::new(SessionBufferInner {
            buffer: RwLock::new(recovered.into()),
            wal: RwLock::new(wal),
            strategy,
            persist_fn: persist_fn_arc,
            stats: RwLock::new(BufferStats {
                buffered_count: buffer_len,
                total_recovered: buffer_len as u64,
                ..Default::default()
            }),
            shutdown: shutdown.clone(),
            next_seq: RwLock::new(0),
        });

        let this = Self {
            inner: inner.clone(),
        };

        // Start background timer task if timer strategy is enabled.
        let needs_timer = matches!(
            inner.strategy,
            FlushStrategy::Timer { .. } | FlushStrategy::TimerAndSize { .. }
        );
        if needs_timer {
            let owner_timer = this.clone();
            tokio::spawn(async move {
                let interval_secs = match owner_timer.inner.strategy {
                    FlushStrategy::Timer { interval_secs } => interval_secs,
                    FlushStrategy::TimerAndSize { interval_secs, .. } => interval_secs,
                    _ => unreachable!(),
                };
                let mut ticker = interval(Duration::from_secs(interval_secs));
                loop {
                    tokio::select! {
                        _ = ticker.tick() => {
                            if owner_timer.inner.shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Err(e) = owner_timer.flush().await {
                                error!("Timer-triggered flush failed: {}", e);
                            }
                        }
                    }
                    if owner_timer.inner.shutdown.load(Ordering::SeqCst) {
                        debug!("SessionBuffer timer task shutting down");
                        break;
                    }
                }
            });
        }

        Ok(this)
    }

    /// Push a single item into the buffer.
    ///
    /// The item is first serialized and written to the WAL before being added
    /// to the in-memory buffer. If the size threshold is reached, a flush is
    /// automatically triggered.
    pub async fn push(&self, item: T) -> KiasResult<()> {
        // Assign sequence number
        let seq = {
            let mut next = self.inner.next_seq.write().await;
            let s = *next;
            *next += 1;
            s
        };

        // Serialize and write WAL entry first (crash guarantee)
        let wal_entry = WalEntry::new(seq, &item)?;
        {
            let mut wal = self.inner.wal.write().await;
            wal.append(&wal_entry)?;
        }

        // Add to in-memory buffer
        let new_len = {
            let mut buf = self.inner.buffer.write().await;
            buf.push_back(item);
            let len = buf.len();
            let mut stats = self.inner.stats.write().await;
            stats.buffered_count = len;
            len
        };

        // Check size threshold and auto-flush if needed
        match self.inner.strategy {
            FlushStrategy::Size { threshold } if new_len >= threshold => {
                let _ = self.do_flush().await?;
            }
            FlushStrategy::TimerAndSize { threshold, .. } if new_len >= threshold => {
                let _ = self.do_flush().await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Push multiple items in one transaction.
    pub async fn push_batch(&self, items: impl IntoIterator<Item = T>) -> KiasResult<()> {
        for item in items {
            self.push(item).await?;
        }
        Ok(())
    }

    /// Force a flush of all buffered items to persistence now.
    pub async fn flush(&self) -> KiasResult<usize> {
        self.do_flush().await
    }

    /// Return a snapshot of buffer statistics.
    pub async fn stats(&self) -> BufferStats {
        self.inner.stats.read().await.clone()
    }

    /// Return the number of items currently in the buffer.
    pub async fn len(&self) -> usize {
        self.inner.buffer.read().await.len()
    }

    /// Return true if the buffer is empty.
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Remove and return all items currently in the buffer without flushing.
    pub async fn drain(&self) -> Vec<T> {
        let mut buf = self.inner.buffer.write().await;
        let items: Vec<T> = buf.drain(..).collect();
        let mut stats = self.inner.stats.write().await;
        stats.buffered_count = 0;
        items
    }

    // ── Private helpers ─────────────────────────────────────────────────────────

    async fn do_flush(&self) -> KiasResult<usize> {
        // Drain buffer under write lock
        let (items, len): (Vec<T>, usize) = {
            let mut buf = self.inner.buffer.write().await;
            let items: Vec<T> = buf.drain(..).collect();
            let len = items.len();
            let mut stats = self.inner.stats.write().await;
            stats.buffered_count = 0;
            stats.flush_count += 1;
            stats.last_flush_at = Some(Instant::now());
            (items, len)
        };

        if items.is_empty() {
            return Ok(0);
        }

        // Call the persist function
        let persist_fn = self.inner.persist_fn.clone();
        (persist_fn)(&items).await?;

        // Update flush stats
        {
            let mut stats = self.inner.stats.write().await;
            stats.total_flushed += len as u64;
        }

        // Truncate WAL after successful persistence
        {
            let mut wal = self.inner.wal.write().await;
            wal.truncate()?;
        }

        debug!("SessionBuffer flushed {} items", len);
        Ok(len)
    }
}

impl<T> Drop for SessionBuffer<T>
where
    T: Serialize + DeserializeOwned + Debug + Send + 'static,
{
    fn drop(&mut self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    // Dummy record type for tests.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    struct Record {
        id: u64,
        payload: String,
    }

    // A persist function that counts flushed items via an atomic counter.
    fn make_counting_persist(
        counter: Arc<AtomicUsize>,
    ) -> impl Fn(&[Record]) -> PersistFut + Send + Sync + 'static {
        move |batch: &[Record]| {
            let n = batch.len();
            let counter = counter.clone();
            let fut = async move {
                counter.fetch_add(n, Ordering::SeqCst);
                Ok(())
            };
            Box::pin(fut) as PersistFut
        }
    }

    // A persist function that always fails.
    fn make_failing_persist() -> impl Fn(&[Record]) -> PersistFut + Send + Sync + 'static {
        |_: &[Record]| {
            let fut = async {
                Err(KiasError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "persist failed",
                )))
            };
            Box::pin(fut) as PersistFut
        }
    }

    #[tokio::test]
    async fn test_push_and_flush() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 1,
            payload: "a".into(),
        })
        .await
        .unwrap();
        buf.push(Record {
            id: 2,
            payload: "b".into(),
        })
        .await
        .unwrap();
        assert_eq!(buf.len().await, 2);

        let flushed = buf.flush().await.unwrap();
        assert_eq!(flushed, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert!(buf.is_empty().await);
    }

    #[tokio::test]
    async fn test_size_based_flush() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Size { threshold: 3 },
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 1,
            payload: "a".into(),
        })
        .await
        .unwrap();
        buf.push(Record {
            id: 2,
            payload: "b".into(),
        })
        .await
        .unwrap();
        // Not flushed yet
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        // Third push triggers automatic flush
        buf.push(Record {
            id: 3,
            payload: "c".into(),
        })
        .await
        .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 3);
        assert!(buf.is_empty().await);
    }

    #[tokio::test]
    async fn test_flush_empty_buffer() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        let flushed = buf.flush().await.unwrap();
        assert_eq!(flushed, 0);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_wal_recovery() {
        let tmp = TempDir::new().unwrap();
        let wal_path = tmp.path().join("wal");

        // Create buffer, push items, drop (no flush) — items stay in WAL.
        {
            let counter = Arc::new(AtomicUsize::new(0));
            let buf = SessionBuffer::new(
                FlushStrategy::Manual,
                wal_path.clone(),
                make_counting_persist(counter.clone()),
            )
            .await
            .unwrap();
            buf.push(Record {
                id: 10,
                payload: "recovered".into(),
            })
            .await
            .unwrap();
            buf.push(Record {
                id: 11,
                payload: "items".into(),
            })
            .await
            .unwrap();
            // Drop without flushing — WAL retains entries.
        }

        // Simulate fresh process: new buffer on same WAL path recovers entries.
        let counter2 = Arc::new(AtomicUsize::new(0));
        let buf2 = SessionBuffer::new(
            FlushStrategy::Manual,
            wal_path,
            make_counting_persist(counter2.clone()),
        )
        .await
        .unwrap();

        assert_eq!(buf2.len().await, 2);
        let stats = buf2.stats().await;
        assert_eq!(stats.total_recovered, 2);

        buf2.flush().await.unwrap();
        assert_eq!(counter2.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_stats() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        let stats0 = buf.stats().await;
        assert_eq!(stats0.buffered_count, 0);
        assert_eq!(stats0.total_recovered, 0);

        buf.push(Record {
            id: 5,
            payload: "x".into(),
        })
        .await
        .unwrap();
        buf.push(Record {
            id: 6,
            payload: "y".into(),
        })
        .await
        .unwrap();

        let stats1 = buf.stats().await;
        assert_eq!(stats1.buffered_count, 2);
        assert_eq!(stats1.total_recovered, 0);

        buf.flush().await.unwrap();
        let stats2 = buf.stats().await;
        assert_eq!(stats2.buffered_count, 0);
        assert_eq!(stats2.total_flushed, 2);
        assert_eq!(stats2.flush_count, 1);
    }

    #[tokio::test]
    async fn test_push_batch() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        let batch: Vec<Record> = (1..=5)
            .map(|i| Record {
                id: i,
                payload: format!("item{i}"),
            })
            .collect();
        buf.push_batch(batch).await.unwrap();
        // threshold is default (100), so all 5 stay in buffer
        assert_eq!(buf.len().await, 5);
        assert_eq!(counter.load(Ordering::SeqCst), 0); // no flush yet

        buf.flush().await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_drain() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 7,
            payload: "drained".into(),
        })
        .await
        .unwrap();
        buf.push(Record {
            id: 8,
            payload: "items".into(),
        })
        .await
        .unwrap();

        let drained = buf.drain().await;
        assert_eq!(drained.len(), 2);
        assert!(buf.is_empty().await);
        assert_eq!(counter.load(Ordering::SeqCst), 0); // drain does not flush
    }

    #[tokio::test]
    async fn test_empty_and_len() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        assert!(buf.is_empty().await);
        assert_eq!(buf.len().await, 0);

        buf.push(Record {
            id: 1,
            payload: "solo".into(),
        })
        .await
        .unwrap();
        assert!(!buf.is_empty().await);
        assert_eq!(buf.len().await, 1);

        buf.flush().await.unwrap();
        assert!(buf.is_empty().await);
    }

    #[tokio::test]
    async fn test_recover_from_wal_helper() {
        let tmp = TempDir::new().unwrap();
        let wal_path = tmp.path().join("wal2");

        // Pre-populate WAL using WalFile directly
        let mut wal = WalFile::new(wal_path.clone());
        wal.append(
            &WalEntry::new(
                1,
                &Record {
                    id: 100,
                    payload: "from_wal".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        wal.append(
            &WalEntry::new(
                2,
                &Record {
                    id: 101,
                    payload: "entries".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();

        let recovered = recover_from_wal::<Record>(&wal_path).unwrap();
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].id, 100);
        assert_eq!(recovered[1].id, 101);
    }

    // ── Enhanced buffer tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_flush_strategy_default() {
        let default = FlushStrategy::default();
        match default {
            FlushStrategy::TimerAndSize {
                interval_secs,
                threshold,
            } => {
                assert_eq!(interval_secs, 5);
                assert_eq!(threshold, 100);
            }
            _ => panic!("Expected TimerAndSize default"),
        }
    }

    #[tokio::test]
    async fn test_flush_strategy_timer() {
        let strategy = FlushStrategy::Timer { interval_secs: 10 };
        assert!(matches!(
            strategy,
            FlushStrategy::Timer { interval_secs: 10 }
        ));
    }

    #[tokio::test]
    async fn test_flush_strategy_manual() {
        let strategy = FlushStrategy::Manual;
        assert!(matches!(strategy, FlushStrategy::Manual));
    }

    #[tokio::test]
    async fn test_buffer_stats_default() {
        let stats = BufferStats::default();
        assert_eq!(stats.buffered_count, 0);
        assert_eq!(stats.total_flushed, 0);
        assert_eq!(stats.total_recovered, 0);
        assert_eq!(stats.flush_count, 0);
        assert_eq!(stats.wal_size_bytes, 0);
        assert!(stats.last_flush_at.is_none());
    }

    #[tokio::test]
    async fn test_wal_entry_serde_roundtrip() {
        let entry = WalEntry::new(
            42,
            &Record {
                id: 99,
                payload: "test".into(),
            },
        )
        .unwrap();
        let json = serde_json::to_vec(&entry).unwrap();
        let decoded: WalEntry<Record> = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.seq, 42);
        let val: Record = decoded.deserialize().unwrap();
        assert_eq!(val.id, 99);
        assert_eq!(val.payload, "test");
    }

    #[tokio::test]
    async fn test_wal_file_truncate() {
        let tmp = TempDir::new().unwrap();
        let mut wal = WalFile::new(tmp.path().join("truncate_wal"));
        wal.append(
            &WalEntry::new(
                1,
                &Record {
                    id: 1,
                    payload: "a".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        wal.append(
            &WalEntry::new(
                2,
                &Record {
                    id: 2,
                    payload: "b".into(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(tmp.path().join("truncate_wal").exists());
        wal.truncate().unwrap();
        assert_eq!(wal.end_offset, 0);
    }

    #[tokio::test]
    async fn test_push_batch_empty() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal_batch_empty"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push_batch(std::iter::empty::<Record>()).await.unwrap();
        assert_eq!(buf.len().await, 0);
        assert_eq!(counter.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_multiple_flush_calls() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal_multi_flush"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 1,
            payload: "a".into(),
        })
        .await
        .unwrap();
        buf.flush().await.unwrap();
        buf.push(Record {
            id: 2,
            payload: "b".into(),
        })
        .await
        .unwrap();
        buf.flush().await.unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert!(buf.is_empty().await);
    }

    #[tokio::test]
    async fn test_timer_and_size_strategy_size_trigger() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::TimerAndSize {
                interval_secs: 3600,
                threshold: 2,
            },
            tmp.path().join("wal_timersize"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 1,
            payload: "a".into(),
        })
        .await
        .unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 0);
        buf.push(Record {
            id: 2,
            payload: "b".into(),
        })
        .await
        .unwrap();
        // threshold=2 should trigger auto-flush
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        assert!(buf.is_empty().await);
    }

    #[tokio::test]
    async fn test_clone_buffer() {
        let tmp = TempDir::new().unwrap();
        let counter = Arc::new(AtomicUsize::new(0));
        let buf = SessionBuffer::new(
            FlushStrategy::Manual,
            tmp.path().join("wal_clone"),
            make_counting_persist(counter.clone()),
        )
        .await
        .unwrap();

        buf.push(Record {
            id: 1,
            payload: "original".into(),
        })
        .await
        .unwrap();
        let buf2 = buf.clone();
        buf2.push(Record {
            id: 2,
            payload: "clone".into(),
        })
        .await
        .unwrap();

        // Both see the same buffer (cloned Arc)
        assert_eq!(buf.len().await, 2);
        assert_eq!(buf2.len().await, 2);
    }
}
