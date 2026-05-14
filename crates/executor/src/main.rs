use async_trait::async_trait;
use chrono::Utc;
use kias_common::KiasResult;
use kias_executor::runtime::TaskExecutor;
use kias_executor::{Task, TaskResult, TaskRuntime, TaskStatus};
use uuid::Uuid;

struct SimpleExecutor;

#[async_trait]
impl TaskExecutor for SimpleExecutor {
    async fn execute(&self, task: &Task) -> KiasResult<TaskResult> {
        let start = Utc::now();
        tracing::info!(task_id = %task.id, "Executing task");
        // 模拟任务执行
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let end = Utc::now();
        Ok(TaskResult {
            task_id: task.id.clone(),
            status: TaskStatus::Completed,
            output: Some(serde_json::json!({"result": "success"})),
            error: None,
            started_at: start,
            completed_at: end,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS Executor Service");

    let executor = Box::new(SimpleExecutor);
    let runtime = TaskRuntime::new(executor);

    let task = Task {
        id: Uuid::new_v4().to_string(),
        name: "test-task".to_string(),
        agent_id: "agent-1".to_string(),
        payload: serde_json::json!({"input": "test"}),
        created_at: Utc::now(),
        timeout: None,
    };

    let result = runtime.run_task(&task).await?;

    println!(
        "Task {} completed with status: {:?}",
        result.task_id, result.status
    );

    tracing::info!("KIAS Executor Service finished");
    Ok(())
}
