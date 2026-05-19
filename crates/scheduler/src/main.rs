use kias_scheduler::{RoundRobin, SchedulerEngine};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting AgentGuard Scheduler");

    let strategy = Arc::new(RoundRobin::new());
    let engine = SchedulerEngine::new(strategy);

    let nodes = vec![
        "node-1".to_string(),
        "node-2".to_string(),
        "node-3".to_string(),
    ];

    // 测试调度
    for i in 0..5 {
        let task_id = format!("task-{}", i);
        let selected = engine.schedule_task(&task_id, &nodes).await?;
        println!("Task {} -> {}", task_id, selected);
    }

    tracing::info!("AgentGuard Scheduler finished");
    Ok(())
}
