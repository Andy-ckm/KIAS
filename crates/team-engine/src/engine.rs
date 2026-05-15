use super::state::{Agent, AgentRole, AgentStatus, Task, TaskStatus, TeamState};
use chrono::Utc;
use kias_common::KiasResult;
use uuid::Uuid;

/// Team Engine - 确定性代码逻辑驱动（借鉴 MiniMax 设计）
///
/// 核心设计：
/// 1. 确定性状态机驱动，不依赖模型自由判断
/// 2. Worker-Verifier 对抗机制
/// 3. 上下文隔离
/// 4. Agent 间通讯
pub struct TeamEngine {
    state: TeamState,
}

impl TeamEngine {
    pub fn new(owner_name: &str) -> Self {
        let owner = Agent {
            id: Uuid::new_v4().to_string(),
            name: owner_name.to_string(),
            role: AgentRole::Owner,
            status: AgentStatus::Idle,
            current_task: None,
            created_at: Utc::now(),
        };

        Self {
            state: TeamState {
                team_id: Uuid::new_v4().to_string(),
                owner,
                workers: Vec::new(),
                verifiers: Vec::new(),
                tasks: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        }
    }

    /// 添加 Worker
    pub fn add_worker(&mut self, name: &str) -> String {
        let worker = Agent {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            role: AgentRole::Worker,
            status: AgentStatus::Idle,
            current_task: None,
            created_at: Utc::now(),
        };
        let id = worker.id.clone();
        self.state.workers.push(worker);
        id
    }

    /// 添加 Verifier
    pub fn add_verifier(&mut self, name: &str) -> String {
        let verifier = Agent {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            role: AgentRole::Verifier,
            status: AgentStatus::Idle,
            current_task: None,
            created_at: Utc::now(),
        };
        let id = verifier.id.clone();
        self.state.verifiers.push(verifier);
        id
    }

    /// Owner 创建任务（确定性状态机）
    pub fn create_task(&mut self, name: &str, description: &str) -> String {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: description.to_string(),
            assigned_to: None,
            verified_by: None,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            retry_count: 0,
            max_retries: 3,
            context: serde_json::json!({}),
        };
        let id = task.id.clone();
        self.state.tasks.push(task);
        id
    }

    /// 分配任务给 Worker（确定性调度）
    pub fn assign_task(&mut self, task_id: &str, worker_id: &str) -> KiasResult<()> {
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

        let worker = self
            .state
            .workers
            .iter_mut()
            .find(|w| w.id == worker_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Worker not found".to_string())
            })?;

        if worker.status != AgentStatus::Idle {
            return Err(kias_common::error::KiasError::Scheduler(
                "Worker is busy".to_string(),
            ));
        }

        task.assigned_to = Some(worker_id.to_string());
        task.status = TaskStatus::Assigned;
        task.updated_at = Utc::now();

        worker.status = AgentStatus::Busy;
        worker.current_task = Some(task_id.to_string());

        tracing::info!(task_id = %task_id, worker_id = %worker_id, "Task assigned");
        Ok(())
    }

    /// Worker 完成任务
    pub fn complete_task(&mut self, task_id: &str) -> KiasResult<()> {
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

        if task.status != TaskStatus::Assigned && task.status != TaskStatus::InProgress {
            return Err(kias_common::error::KiasError::Scheduler(
                "Invalid task status".to_string(),
            ));
        }

        task.status = TaskStatus::Completed;
        task.updated_at = Utc::now();

        // 释放 Worker
        if let Some(worker_id) = &task.assigned_to {
            if let Some(worker) = self.state.workers.iter_mut().find(|w| w.id == *worker_id) {
                worker.status = AgentStatus::Idle;
                worker.current_task = None;
            }
        }

        tracing::info!(task_id = %task_id, "Task completed, ready for verification");
        Ok(())
    }

    /// Verifier 验证任务（对抗机制）
    pub fn verify_task(
        &mut self,
        task_id: &str,
        verifier_id: &str,
        passed: bool,
    ) -> KiasResult<()> {
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

        if task.status != TaskStatus::Completed {
            return Err(kias_common::error::KiasError::Scheduler(
                "Task not completed".to_string(),
            ));
        }

        task.verified_by = Some(verifier_id.to_string());
        task.updated_at = Utc::now();

        if passed {
            task.status = TaskStatus::Verified;
            tracing::info!(task_id = %task_id, "Task verified");
        } else {
            task.retry_count += 1;
            if task.retry_count >= task.max_retries {
                task.status = TaskStatus::Failed;
                tracing::warn!(task_id = %task_id, "Task failed after max retries");
            } else {
                task.status = TaskStatus::Rejected;
                tracing::info!(task_id = %task_id, retry = task.retry_count, "Task rejected, will retry");
            }
        }

        Ok(())
    }

    /// 重试被拒绝的任务
    pub fn retry_task(&mut self, task_id: &str) -> KiasResult<()> {
        // First, validate and get the previous assigned worker (if any)
        let previous_worker_id = self
            .state
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?
            .assigned_to
            .clone();

        // Set status to Assigned first
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

        if task.status != TaskStatus::Rejected {
            return Err(kias_common::error::KiasError::Scheduler(
                "Task not rejected".to_string(),
            ));
        }

        task.status = TaskStatus::Assigned;
        task.updated_at = Utc::now();
        tracing::info!(task_id = %task_id, "Task status reset to Assigned for retry");

        // Now reassign to an available worker (prefer different worker)
        self.reassign_task(task_id, previous_worker_id.as_deref())?;

        Ok(())
    }

    /// Reassign a task to an available worker (optionally preferring a different worker
    /// than the one specified by `exclude_worker_id`).
    pub fn reassign_task(
        &mut self,
        task_id: &str,
        exclude_worker_id: Option<&str>,
    ) -> KiasResult<()> {
        // Find an available Idle worker, preferring one different from exclude_worker_id
        let worker_id = self
            .state
            .workers
            .iter()
            .filter(|w| w.status == AgentStatus::Idle)
            .min_by_key(|w| {
                // Prefer a different worker for diversity, then fall back to any idle
                if Some(w.id.as_str()) == exclude_worker_id {
                    1
                } else {
                    0
                }
            })
            .map(|w| w.id.clone())
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler(
                    "No idle workers available for reassignment".to_string(),
                )
            })?;

        // Set the worker to Busy with this task
        let worker = self
            .state
            .workers
            .iter_mut()
            .find(|w| w.id == worker_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Worker not found".to_string())
            })?;

        worker.status = AgentStatus::Busy;
        worker.current_task = Some(task_id.to_string());

        // Update task assigned_to and set status to InProgress
        let task = self
            .state
            .tasks
            .iter_mut()
            .find(|t| t.id == task_id)
            .ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

        task.assigned_to = Some(worker_id.clone());
        task.status = TaskStatus::InProgress;
        task.updated_at = Utc::now();

        tracing::info!(task_id = %task_id, worker_id = %worker_id, "Task reassigned to worker");
        Ok(())
    }

    /// Execute a task with automatic retry loop.
    ///
    /// The flow is: Assign -> Complete -> Verify (via callback) -> If rejected, retry.
    /// The `should_pass` closure receives the attempt number (0-indexed) and returns
    /// `true` if the verifier accepts, `false` to reject.
    /// Returns the final task status.
    pub fn execute_with_retry(
        &mut self,
        task_id: &str,
        worker_id: &str,
        verifier_id: &str,
        mut should_pass: impl FnMut(u32) -> bool,
    ) -> KiasResult<TaskStatus> {
        // Initial assignment
        self.assign_task(task_id, worker_id)?;

        let mut attempt = 0u32;
        loop {
            // Worker completes the task
            self.complete_task(task_id)?;

            // Verifier checks the task
            let passed = should_pass(attempt);
            self.verify_task(task_id, verifier_id, passed)?;

            let status = self.get_task_status(task_id).ok_or_else(|| {
                kias_common::error::KiasError::Scheduler("Task not found".to_string())
            })?;

            match status {
                TaskStatus::Verified => return Ok(TaskStatus::Verified),
                TaskStatus::Failed => return Ok(TaskStatus::Failed),
                TaskStatus::Rejected => {
                    // Retry: reassign to a (possibly different) worker
                    attempt += 1;
                    self.retry_task(task_id)?;
                    // Continue the loop - worker will complete again
                }
                _ => return Ok(status),
            }
        }
    }

    /// 获取 Team 状态
    pub fn get_state(&self) -> &TeamState {
        &self.state
    }

    /// 获取任务状态
    pub fn get_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.state
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .map(|t| t.status.clone())
    }

    /// 获取任务的 assigned_to agent id
    pub fn get_task_assigned_to(&self, task_id: &str) -> Option<String> {
        self.state
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .and_then(|t| t.assigned_to.clone())
    }

    /// 获取任务的 retry_count
    pub fn get_task_retry_count(&self, task_id: &str) -> Option<u32> {
        self.state
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .map(|t| t.retry_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_engine_create() {
        let engine = TeamEngine::new("owner-1");
        let state = engine.get_state();
        assert_eq!(state.owner.name, "owner-1");
        assert_eq!(state.workers.len(), 0);
        assert_eq!(state.verifiers.len(), 0);
        assert_eq!(state.tasks.len(), 0);
    }

    #[test]
    fn test_add_worker() {
        let mut engine = TeamEngine::new("owner");
        let id = engine.add_worker("worker-1");
        assert!(!id.is_empty());
        assert_eq!(engine.get_state().workers.len(), 1);
        assert_eq!(engine.get_state().workers[0].name, "worker-1");
    }

    #[test]
    fn test_add_verifier() {
        let mut engine = TeamEngine::new("owner");
        let id = engine.add_verifier("verifier-1");
        assert!(!id.is_empty());
        assert_eq!(engine.get_state().verifiers.len(), 1);
    }

    #[test]
    fn test_create_task() {
        let mut engine = TeamEngine::new("owner");
        let task_id = engine.create_task("task-1", "do something");
        assert!(!task_id.is_empty());
        assert_eq!(engine.get_state().tasks.len(), 1);
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Pending));
    }

    #[test]
    fn test_assign_task() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("worker-1");
        let task_id = engine.create_task("task-1", "desc");

        engine.assign_task(&task_id, &worker_id).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Assigned));
        assert_eq!(engine.get_state().workers[0].status, AgentStatus::Busy);
    }

    #[test]
    fn test_assign_task_to_busy_worker_fails() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("worker-1");
        let task1 = engine.create_task("t1", "d1");
        let task2 = engine.create_task("t2", "d2");

        engine.assign_task(&task1, &worker_id).unwrap();
        let result = engine.assign_task(&task2, &worker_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_assign_nonexistent_task_fails() {
        let mut engine = TeamEngine::new("owner");
        let result = engine.assign_task("nope", "nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_task() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("worker-1");
        let task_id = engine.create_task("task-1", "desc");

        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();

        assert_eq!(
            engine.get_task_status(&task_id),
            Some(TaskStatus::Completed)
        );
        // Worker should be freed
        assert_eq!(engine.get_state().workers[0].status, AgentStatus::Idle);
    }

    #[test]
    fn test_verify_task_pass() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, true).unwrap();

        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Verified));
    }

    #[test]
    fn test_verify_task_reject_then_retry() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();

        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));

        engine.retry_task(&task_id).unwrap();
        // retry_task now reassigns to a worker, setting status to InProgress
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::InProgress));
        // Worker should be Busy
        assert_eq!(engine.get_state().workers[0].status, AgentStatus::Busy);
    }

    #[test]
    fn test_verify_task_max_retries() {
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // Initial assignment + reject
        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));

        // Retry 1 -> reject (retry_count becomes 2)
        engine.retry_task(&task_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));

        // Retry 2 -> reject (retry_count becomes 3, which >= max_retries=3 -> Failed)
        engine.retry_task(&task_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Failed));
    }

    #[test]
    fn test_complete_unassigned_task_fails() {
        let mut engine = TeamEngine::new("owner");
        let task_id = engine.create_task("t", "d");
        let result = engine.complete_task(&task_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_role_display() {
        assert_eq!(format!("{:?}", AgentRole::Owner), "Owner");
        assert_eq!(format!("{:?}", AgentRole::Worker), "Worker");
        assert_eq!(format!("{:?}", AgentRole::Verifier), "Verifier");
    }

    #[test]
    fn test_task_status_transitions() {
        let mut engine = TeamEngine::new("owner");
        let wid = engine.add_worker("w");
        let vid = engine.add_verifier("v");
        let tid = engine.create_task("t", "d");

        // Pending -> Assigned -> Completed -> Verified
        assert_eq!(engine.get_task_status(&tid), Some(TaskStatus::Pending));
        engine.assign_task(&tid, &wid).unwrap();
        assert_eq!(engine.get_task_status(&tid), Some(TaskStatus::Assigned));
        engine.complete_task(&tid).unwrap();
        assert_eq!(engine.get_task_status(&tid), Some(TaskStatus::Completed));
        engine.verify_task(&tid, &vid, true).unwrap();
        assert_eq!(engine.get_task_status(&tid), Some(TaskStatus::Verified));
    }

    #[test]
    fn test_retry_triggers_re_execution() {
        // Rejected task gets reassigned to a worker (not stuck in Assigned)
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // Assign -> complete -> reject
        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));

        // Retry should reassign to worker with InProgress status
        engine.retry_task(&task_id).unwrap();
        assert_eq!(
            engine.get_task_status(&task_id),
            Some(TaskStatus::InProgress)
        );

        // Worker should be Busy (task is actually being worked on)
        let assigned_worker = engine.get_task_assigned_to(&task_id).unwrap();
        let worker = engine
            .get_state()
            .workers
            .iter()
            .find(|w| w.id == assigned_worker)
            .unwrap();
        assert_eq!(worker.status, AgentStatus::Busy);
        assert_eq!(worker.current_task, Some(task_id.clone()));
    }

    #[test]
    fn test_retry_exhaustion() {
        // Fails after max_retries
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // max_retries is 3, so 3 rejections should lead to Failed
        // Attempt 0 (initial): reject -> retry_count=1
        engine.assign_task(&task_id, &worker_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));
        assert_eq!(engine.get_task_retry_count(&task_id), Some(1));

        // Attempt 1: retry -> complete -> reject -> retry_count=2
        engine.retry_task(&task_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Rejected));
        assert_eq!(engine.get_task_retry_count(&task_id), Some(2));

        // Attempt 2: retry -> complete -> reject -> retry_count=3 >= max_retries -> Failed
        engine.retry_task(&task_id).unwrap();
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();
        assert_eq!(engine.get_task_status(&task_id), Some(TaskStatus::Failed));
        assert_eq!(engine.get_task_retry_count(&task_id), Some(3));

        // Cannot retry a Failed task
        let result = engine.retry_task(&task_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_retry_with_different_worker() {
        // Prefers different worker on retry
        let mut engine = TeamEngine::new("owner");
        let worker1_id = engine.add_worker("w1");
        let worker2_id = engine.add_worker("w2");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // Assign to worker1 -> complete -> reject
        engine.assign_task(&task_id, &worker1_id).unwrap();
        assert_eq!(
            engine.get_task_assigned_to(&task_id),
            Some(worker1_id.clone())
        );
        engine.complete_task(&task_id).unwrap();
        engine.verify_task(&task_id, &verifier_id, false).unwrap();

        // After complete_task, worker1 is Idle again. Both workers are Idle.
        // retry_task should prefer worker2 (different from worker1)
        engine.retry_task(&task_id).unwrap();
        assert_eq!(
            engine.get_task_assigned_to(&task_id),
            Some(worker2_id.clone())
        );

        // Worker2 should be Busy, Worker1 should still be Idle
        let w1 = engine
            .get_state()
            .workers
            .iter()
            .find(|w| w.id == worker1_id)
            .unwrap();
        let w2 = engine
            .get_state()
            .workers
            .iter()
            .find(|w| w.id == worker2_id)
            .unwrap();
        assert_eq!(w1.status, AgentStatus::Idle);
        assert_eq!(w2.status, AgentStatus::Busy);
    }

    #[test]
    fn test_execute_with_retry_full_loop() {
        // End-to-end retry flow using execute_with_retry
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // Reject the first 2 attempts, accept on the 3rd
        let result = engine
            .execute_with_retry(&task_id, &worker_id, &verifier_id, |attempt| attempt >= 2)
            .unwrap();

        assert_eq!(result, TaskStatus::Verified);
        assert_eq!(engine.get_task_retry_count(&task_id), Some(2));
    }

    #[test]
    fn test_execute_with_retry_exhaustion() {
        // execute_with_retry fails when verifier always rejects
        let mut engine = TeamEngine::new("owner");
        let worker_id = engine.add_worker("w");
        let verifier_id = engine.add_verifier("v");
        let task_id = engine.create_task("t", "d");

        // Always reject -> should fail after max_retries (3)
        let result = engine
            .execute_with_retry(&task_id, &worker_id, &verifier_id, |_attempt| false)
            .unwrap();

        assert_eq!(result, TaskStatus::Failed);
        assert_eq!(engine.get_task_retry_count(&task_id), Some(3));
    }
}
