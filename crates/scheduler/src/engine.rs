use super::strategy::ScheduleStrategy;
use kias_common::KiasResult;
use std::sync::Arc;

pub struct SchedulerEngine {
    strategy: Arc<dyn ScheduleStrategy>,
}

impl SchedulerEngine {
    pub fn new(strategy: Arc<dyn ScheduleStrategy>) -> Self {
        Self { strategy }
    }

    pub async fn schedule_task(
        &self,
        task_id: &str,
        available_nodes: &[String],
    ) -> KiasResult<String> {
        tracing::info!(task_id = %task_id, "Scheduling task");
        let selected = self.strategy.select_node(available_nodes).await?;
        tracing::info!(task_id = %task_id, node = %selected, "Task scheduled");
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// Mock strategy that always picks the first node
    struct FirstNodeStrategy;

    #[async_trait]
    impl ScheduleStrategy for FirstNodeStrategy {
        async fn select_node(&self, nodes: &[String]) -> KiasResult<String> {
            nodes
                .first()
                .cloned()
                .ok_or_else(|| kias_common::KiasError::NotFound("no nodes available".into()))
        }
    }

    /// Mock strategy that always picks the last node
    struct LastNodeStrategy;

    #[async_trait]
    impl ScheduleStrategy for LastNodeStrategy {
        async fn select_node(&self, nodes: &[String]) -> KiasResult<String> {
            nodes
                .last()
                .cloned()
                .ok_or_else(|| kias_common::KiasError::NotFound("no nodes available".into()))
        }
    }

    /// Mock strategy that always fails
    struct FailStrategy;

    #[async_trait]
    impl ScheduleStrategy for FailStrategy {
        async fn select_node(&self, _nodes: &[String]) -> KiasResult<String> {
            Err(kias_common::KiasError::Internal(anyhow::anyhow!(
                "strategy failure"
            )))
        }
    }

    fn nodes() -> Vec<String> {
        vec!["node-a".into(), "node-b".into(), "node-c".into()]
    }

    #[tokio::test]
    async fn test_schedule_task_first_node() {
        let engine = SchedulerEngine::new(Arc::new(FirstNodeStrategy));
        let result = engine.schedule_task("task-1", &nodes()).await.unwrap();
        assert_eq!(result, "node-a");
    }

    #[tokio::test]
    async fn test_schedule_task_last_node() {
        let engine = SchedulerEngine::new(Arc::new(LastNodeStrategy));
        let result = engine.schedule_task("task-2", &nodes()).await.unwrap();
        assert_eq!(result, "node-c");
    }

    #[tokio::test]
    async fn test_schedule_task_empty_nodes() {
        let engine = SchedulerEngine::new(Arc::new(FirstNodeStrategy));
        let empty: Vec<String> = vec![];
        let result = engine.schedule_task("task-3", &empty).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_schedule_task_single_node() {
        let engine = SchedulerEngine::new(Arc::new(FirstNodeStrategy));
        let single = vec!["only-node".to_string()];
        let result = engine.schedule_task("task-4", &single).await.unwrap();
        assert_eq!(result, "only-node");
    }

    #[tokio::test]
    async fn test_schedule_task_strategy_failure() {
        let engine = SchedulerEngine::new(Arc::new(FailStrategy));
        let result = engine.schedule_task("task-5", &nodes()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_schedule_task_preserves_task_id() {
        // Verify the engine doesn't modify the task_id (just logs it)
        let engine = SchedulerEngine::new(Arc::new(FirstNodeStrategy));
        let id = "my-special-task-id-123";
        let result = engine.schedule_task(id, &nodes()).await;
        assert!(result.is_ok());
    }
}
