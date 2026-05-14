use super::ScheduleStrategy;
use async_trait::async_trait;
use kias_common::KiasResult;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RoundRobin {
    counter: AtomicUsize,
}

impl RoundRobin {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for RoundRobin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScheduleStrategy for RoundRobin {
    async fn select_node(&self, nodes: &[String]) -> KiasResult<String> {
        if nodes.is_empty() {
            return Err(kias_common::KiasError::NoAvailableNodes);
        }
        let idx = self.counter.fetch_add(1, Ordering::Relaxed) % nodes.len();
        Ok(nodes[idx].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_round_robin_cycle() {
        let rr = RoundRobin::new();
        let nodes = vec!["n1".into(), "n2".into(), "n3".into()];
        assert_eq!(rr.select_node(&nodes).await.unwrap(), "n1");
        assert_eq!(rr.select_node(&nodes).await.unwrap(), "n2");
        assert_eq!(rr.select_node(&nodes).await.unwrap(), "n3");
        assert_eq!(rr.select_node(&nodes).await.unwrap(), "n1"); // wraps
    }

    #[tokio::test]
    async fn test_round_robin_empty() {
        let rr = RoundRobin::new();
        let nodes: Vec<String> = vec![];
        assert!(rr.select_node(&nodes).await.is_err());
    }
}
