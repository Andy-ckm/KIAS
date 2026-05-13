use std::sync::Arc;
use kias_common::KiasResult;
use super::strategy::ScheduleStrategy;

pub struct SchedulerEngine {
    strategy: Arc<dyn ScheduleStrategy>,
}

impl SchedulerEngine {
    pub fn new(strategy: Arc<dyn ScheduleStrategy>) -> Self {
        Self { strategy }
    }

    pub async fn schedule_task(&self, task_id: &str, available_nodes: &[String]) -> KiasResult<String> {
        tracing::info!(task_id = %task_id, "Scheduling task");
        let selected = self.strategy.select_node(available_nodes).await?;
        tracing::info!(task_id = %task_id, node = %selected, "Task scheduled");
        Ok(selected)
    }
}
