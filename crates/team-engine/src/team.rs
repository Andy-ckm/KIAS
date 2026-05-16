use super::engine::TeamEngine;
use super::owner::Owner;
use super::state::TeamState;
use super::verifier::Verifier;
use super::worker::Worker;
use kias_common::KiasResult;

/// 最大重试次数
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Team - 主 Agent 牵头的任务团队（借鉴 MiniMax 设计）
///
/// 三类核心角色：
/// 1. Owner - 控制面
/// 2. Worker - 执行面
/// 3. Verifier - 质量门禁
pub struct Team {
    engine: TeamEngine,
    owner: Box<dyn Owner>,
    workers: Vec<Box<dyn Worker>>,
    verifiers: Vec<Box<dyn Verifier>>,
    max_retries: u32,
}

impl Team {
    pub fn new(owner: Box<dyn Owner>) -> Self {
        Self {
            engine: TeamEngine::new("default-owner"),
            owner,
            workers: Vec::new(),
            verifiers: Vec::new(),
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }

    /// 设置最大重试次数
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// 添加 Worker
    pub fn add_worker(&mut self, worker: Box<dyn Worker>) {
        self.engine.add_worker(worker.name());
        self.workers.push(worker);
    }

    /// 添加 Verifier
    pub fn add_verifier(&mut self, verifier: Box<dyn Verifier>) {
        self.engine.add_verifier(verifier.name());
        self.verifiers.push(verifier);
    }

    /// 执行任务流程（确定性状态机驱动）
    pub async fn execute(&mut self, user_input: &str) -> KiasResult<String> {
        tracing::info!("Team execution started");

        // 1. Owner 理解目标
        let goal = self.owner.understand_goal(user_input).await?;
        tracing::info!(goal = %goal, "Goal understood");

        // 2. Owner 拆分子任务
        let task_names = self.owner.decompose_tasks(&goal).await?;
        tracing::info!(task_count = task_names.len(), "Tasks decomposed");

        // 3. 创建任务
        let mut task_ids = Vec::new();
        for name in &task_names {
            let task_id = self.engine.create_task(name, &format!("Task: {}", name));
            task_ids.push(task_id);
        }

        // 4. 收集 Worker IDs (避免借用冲突)
        let worker_ids: Vec<String> = self
            .engine
            .get_state()
            .workers
            .iter()
            .map(|w| w.id.clone())
            .collect();
        let verifier_ids: Vec<String> = self
            .engine
            .get_state()
            .verifiers
            .iter()
            .map(|v| v.id.clone())
            .collect();

        if worker_ids.is_empty() {
            return Err(kias_common::KiasError::Scheduler(
                "No workers available".to_string(),
            ));
        }
        if verifier_ids.is_empty() {
            return Err(kias_common::KiasError::Scheduler(
                "No verifiers available".to_string(),
            ));
        }

        // 5. 分配任务给 Worker（按顺序）
        for (idx, task_id) in task_ids.iter().enumerate() {
            let worker_idx = idx % worker_ids.len();
            self.engine.assign_task(task_id, &worker_ids[worker_idx])?;
        }

        // 6. 执行任务（Worker）
        let mut results = Vec::new();
        for (idx, task_id) in task_ids.iter().enumerate() {
            let worker_idx = idx % self.workers.len();
            // 获取任务的只读副本
            let task = self
                .engine
                .get_state()
                .tasks
                .iter()
                .find(|t| t.id == *task_id)
                .cloned()
                .ok_or_else(|| kias_common::KiasError::Scheduler("Task not found".to_string()))?;
            let result = self.workers[worker_idx].execute(&task).await?;
            results.push(result);
            self.engine.complete_task(task_id)?;
        }

        // 7. 验证任务（Verifier - 对抗机制）+ 重试逻辑
        for (idx, task_id) in task_ids.iter().enumerate() {
            let verifier_idx = idx % self.verifiers.len();
            let task = self
                .engine
                .get_state()
                .tasks
                .iter()
                .find(|t| t.id == *task_id)
                .cloned()
                .ok_or_else(|| kias_common::KiasError::Scheduler("Task not found".to_string()))?;
            let verification = self.verifiers[verifier_idx]
                .verify(&task, &results[idx])
                .await?;

            if verification.passed {
                self.engine
                    .verify_task(task_id, &verifier_ids[verifier_idx], true)?;
            } else {
                self.engine
                    .verify_task(task_id, &verifier_ids[verifier_idx], false)?;

                // 重试循环：重新执行 + 重新验证
                let mut retry_count = 0u32;
                while retry_count < self.max_retries {
                    retry_count += 1;
                    tracing::warn!(
                        task_id = %task_id,
                        attempt = retry_count,
                        max = self.max_retries,
                        "Verification failed, retrying task"
                    );

                    // 将任务状态重置为 Assigned，分配给新 Worker
                    self.engine.retry_task(task_id)?;

                    // 获取新分配的 Worker
                    let new_worker_id =
                        self.engine.get_task_assigned_to(task_id).ok_or_else(|| {
                            kias_common::KiasError::Scheduler(
                                "No worker available for retry".to_string(),
                            )
                        })?;
                    let new_worker_idx = worker_ids
                        .iter()
                        .position(|wid| *wid == new_worker_id)
                        .unwrap_or(0);

                    // 重新执行任务
                    let task = self
                        .engine
                        .get_state()
                        .tasks
                        .iter()
                        .find(|t| t.id == *task_id)
                        .cloned()
                        .ok_or_else(|| {
                            kias_common::KiasError::Scheduler("Task not found".to_string())
                        })?;
                    let new_result = self.workers[new_worker_idx].execute(&task).await?;
                    self.engine.complete_task(task_id)?;

                    // 更新结果
                    if let Some(r) = results.get_mut(idx) {
                        *r = new_result;
                    }

                    // 重新验证
                    let task = self
                        .engine
                        .get_state()
                        .tasks
                        .iter()
                        .find(|t| t.id == *task_id)
                        .cloned()
                        .ok_or_else(|| {
                            kias_common::KiasError::Scheduler("Task not found".to_string())
                        })?;
                    let re_verification = self.verifiers[verifier_idx]
                        .verify(&task, &results[idx])
                        .await?;

                    if re_verification.passed {
                        self.engine
                            .verify_task(task_id, &verifier_ids[verifier_idx], true)?;
                        tracing::info!(
                            task_id = %task_id,
                            attempt = retry_count,
                            "Task passed verification after retry"
                        );
                        break;
                    } else {
                        self.engine
                            .verify_task(task_id, &verifier_ids[verifier_idx], false)?;
                    }

                    if retry_count >= self.max_retries {
                        tracing::error!(
                            task_id = %task_id,
                            retries = retry_count,
                            "Task failed after all retries"
                        );
                    }
                }
            }
        }

        // 8. Owner 合并结果
        let final_result = self.owner.merge_results(&results).await?;

        tracing::info!("Team execution completed");
        Ok(final_result)
    }

    /// 获取 Team 状态
    pub fn get_state(&self) -> &TeamState {
        self.engine.get_state()
    }

    /// 获取最大重试次数
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // --- Mock Owner ---
    struct MockOwner;

    #[async_trait]
    impl Owner for MockOwner {
        async fn understand_goal(&self, input: &str) -> KiasResult<String> {
            Ok(format!("Goal: {}", input))
        }
        async fn decompose_tasks(&self, _goal: &str) -> KiasResult<Vec<String>> {
            Ok(vec!["task-1".to_string()])
        }
        async fn determine_order(&self, tasks: &[String]) -> KiasResult<Vec<usize>> {
            Ok((0..tasks.len()).collect())
        }
        async fn merge_results(&self, results: &[String]) -> KiasResult<String> {
            Ok(results.join("; "))
        }
        fn should_stop(&self, _state: &TeamState) -> bool {
            false
        }
    }

    // --- Mock Worker ---
    struct MockWorker {
        name_str: String,
    }

    impl MockWorker {
        fn new(name: &str) -> Self {
            Self {
                name_str: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Worker for MockWorker {
        fn name(&self) -> &str {
            &self.name_str
        }
        fn capabilities(&self) -> Vec<String> {
            vec!["general".to_string()]
        }
        async fn execute(&self, task: &super::super::state::Task) -> KiasResult<String> {
            Ok(format!("Result from {} for {}", self.name_str, task.name))
        }
    }

    // --- Mock Verifier ---
    struct MockVerifier {
        name_str: String,
        /// 第几次调用时通过
        pass_on_attempt: u32,
        call_count: std::sync::atomic::AtomicU32,
    }

    impl MockVerifier {
        fn new(name: &str, pass_on_attempt: u32) -> Self {
            Self {
                name_str: name.to_string(),
                pass_on_attempt,
                call_count: std::sync::atomic::AtomicU32::new(0),
            }
        }
    }

    #[async_trait]
    impl Verifier for MockVerifier {
        fn name(&self) -> &str {
            &self.name_str
        }
        async fn verify(
            &self,
            _task: &super::super::state::Task,
            _result: &str,
        ) -> KiasResult<super::super::verifier::VerificationResult> {
            let count = self
                .call_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            let passed = count >= self.pass_on_attempt;
            Ok(super::super::verifier::VerificationResult {
                passed,
                issues: if passed {
                    vec![]
                } else {
                    vec!["Not yet".to_string()]
                },
                suggestions: vec![],
            })
        }
    }

    #[test]
    fn test_team_creation() {
        let team = Team::new(Box::new(MockOwner));
        assert_eq!(team.max_retries(), DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_team_with_custom_max_retries() {
        let team = Team::new(Box::new(MockOwner)).with_max_retries(5);
        assert_eq!(team.max_retries(), 5);
    }

    #[tokio::test]
    async fn test_team_execute_success_first_try() {
        let mut team = Team::new(Box::new(MockOwner));
        team.add_worker(Box::new(MockWorker::new("w1")));
        team.add_verifier(Box::new(MockVerifier::new("v1", 1))); // 一次通过

        let result = team.execute("test input").await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Result from w1"));
    }

    #[tokio::test]
    async fn test_team_execute_with_retry() {
        let mut team = Team::new(Box::new(MockOwner)).with_max_retries(3);
        team.add_worker(Box::new(MockWorker::new("w1")));
        team.add_verifier(Box::new(MockVerifier::new("v1", 3))); // 第3次通过

        let result = team.execute("test input").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_team_no_workers() {
        let mut team = Team::new(Box::new(MockOwner));
        team.add_verifier(Box::new(MockVerifier::new("v1", 1)));

        let result = team.execute("test input").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_team_no_verifiers() {
        let mut team = Team::new(Box::new(MockOwner));
        team.add_worker(Box::new(MockWorker::new("w1")));

        let result = team.execute("test input").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_team_state_tracking() {
        let mut team = Team::new(Box::new(MockOwner));
        team.add_worker(Box::new(MockWorker::new("w1")));
        team.add_verifier(Box::new(MockVerifier::new("v1", 1)));

        let _ = team.execute("test").await;
        let state = team.get_state();
        assert!(!state.tasks.is_empty());
    }
}
