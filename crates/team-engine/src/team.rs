use kias_common::KiasResult;
use super::state::TeamState;
use super::owner::Owner;
use super::worker::Worker;
use super::verifier::Verifier;
use super::engine::TeamEngine;

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
}

impl Team {
    pub fn new(owner: Box<dyn Owner>) -> Self {
        Self {
            engine: TeamEngine::new("default-owner"),
            owner,
            workers: Vec::new(),
            verifiers: Vec::new(),
        }
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
        let worker_ids: Vec<String> = self.engine.get_state().workers.iter().map(|w| w.id.clone()).collect();
        let verifier_ids: Vec<String> = self.engine.get_state().verifiers.iter().map(|v| v.id.clone()).collect();

        if worker_ids.is_empty() {
            return Err(kias_common::KiasError::Scheduler("No workers available".to_string()));
        }
        if verifier_ids.is_empty() {
            return Err(kias_common::KiasError::Scheduler("No verifiers available".to_string()));
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
            let task = self.engine.get_state().tasks.iter().find(|t| t.id == *task_id)
                .cloned()
                .ok_or_else(|| kias_common::KiasError::Scheduler("Task not found".to_string()))?;
            let result = self.workers[worker_idx].execute(&task).await?;
            results.push(result);
            self.engine.complete_task(task_id)?;
        }

        // 7. 验证任务（Verifier - 对抗机制）
        for (idx, task_id) in task_ids.iter().enumerate() {
            let verifier_idx = idx % self.verifiers.len();
            let task = self.engine.get_state().tasks.iter().find(|t| t.id == *task_id)
                .cloned()
                .ok_or_else(|| kias_common::KiasError::Scheduler("Task not found".to_string()))?;
            let verification = self.verifiers[verifier_idx].verify(&task, &results[idx]).await?;

            if verification.passed {
                self.engine.verify_task(task_id, &verifier_ids[verifier_idx], true)?;
            } else {
                self.engine.verify_task(task_id, &verifier_ids[verifier_idx], false)?;
                // 重试逻辑
                self.engine.retry_task(task_id)?;
                // TODO: 重新执行任务
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
}
