use async_trait::async_trait;
use super::ScheduleStrategy;
use kias_common::KiasResult;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct LeastLoaded {
    fallback_idx: AtomicUsize,
}

impl LeastLoaded {
    pub fn new() -> Self {
        Self {
            fallback_idx: AtomicUsize::new(0),
        }
    }
}

impl Default for LeastLoaded {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScheduleStrategy for LeastLoaded {
    async fn select_node(&self, nodes: &[String]) -> KiasResult<String> {
        if nodes.is_empty() {
            return Err(kias_common::KiasError::NoAvailableNodes);
        }
        // Simplified: round-robin fallback (real impl would query node metrics)
        let idx = self.fallback_idx.fetch_add(1, Ordering::Relaxed) % nodes.len();
        Ok(nodes[idx].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_least_loaded_picks_valid() {
        let ll = LeastLoaded::new();
        let nodes = vec!["n1".into(), "n2".into()];
        let result = ll.select_node(&nodes).await.unwrap();
        assert!(nodes.contains(&result));
    }

    #[tokio::test]
    async fn test_least_loaded_empty() {
        let ll = LeastLoaded::new();
        assert!(ll.select_node(&[]).await.is_err());
    }
}
