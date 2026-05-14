use chrono::Utc;
use kias_cache::hub::CacheEntry;
use kias_cache::{CacheHub, LRUStrategy};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS Cache Service");

    let strategy = Box::new(LRUStrategy::new());
    let hub = CacheHub::new(strategy);

    // 测试缓存
    let entry = CacheEntry {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        created_at: Utc::now(),
        ttl: None,
    };

    hub.set(entry).await?;

    if let Some(cached) = hub.get("test-key").await? {
        println!("Cache hit: {:?}", String::from_utf8_lossy(&cached.value));
    }

    tracing::info!("KIAS Cache Service finished");
    Ok(())
}
