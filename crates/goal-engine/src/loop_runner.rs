use kias_common::KiasResult;
use super::goal::{Goal, GoalState, GoalStatus, EvaluationResult};
use super::evaluator::GoalEvaluator;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Round executor callback - executes one round of work
/// This is the "Worker" in the goal loop
#[async_trait::async_trait]
pub trait RoundExecutor: Send + Sync {
    /// Execute one round given the goal context and feedback from previous round
    async fn execute_round(
        &self,
        goal: &Goal,
        round: u32,
        previous_feedback: Option<&EvaluationResult>,
    ) -> KiasResult<String>;
}

/// Simple executor that returns placeholder output (for testing/demo)
pub struct SimpleExecutor;

#[async_trait::async_trait]
impl RoundExecutor for SimpleExecutor {
    async fn execute_round(
        &self,
        _goal: &Goal,
        round: u32,
        _previous_feedback: Option<&EvaluationResult>,
    ) -> KiasResult<String> {
        Ok(format!("Round {} output", round))
    }
}

/// Goal loop runner state for observability
#[derive(Debug, Clone)]
pub struct LoopMetrics {
    pub total_rounds: u32,
    pub total_duration_ms: u64,
    pub achieved_on_round: Option<u32>,
    pub feedback_chain: Vec<String>,
}

/// Checkpoint for goal loop persistence (crash recovery)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCheckpoint {
    pub goal_id: String,
    pub state: GoalState,
    pub last_evaluation: Option<EvaluationResult>,
    pub checkpointed_at: DateTime<Utc>,
}

impl GoalCheckpoint {
    pub fn new(goal_id: &str, state: GoalState, last_evaluation: Option<EvaluationResult>) -> Self {
        Self {
            goal_id: goal_id.to_string(),
            state,
            last_evaluation,
            checkpointed_at: Utc::now(),
        }
    }

    /// Serialize to JSON for persistence
    pub fn to_json(&self) -> KiasResult<String> {
        serde_json::to_string(self).map_err(|e| kias_common::error::KiasError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON
    pub fn from_json(json: &str) -> KiasResult<Self> {
        serde_json::from_str(json).map_err(|e| kias_common::error::KiasError::Serialization(e.to_string()))
    }
}

/// Cancellation token for goal loops
#[derive(Debug, Clone)]
pub struct GoalCancelToken {
    cancelled: Arc<AtomicBool>,
}

impl GoalCancelToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for GoalCancelToken {
    fn default() -> Self {
        Self::new()
    }
}

/// 目标循环运行器（借鉴 Claude Code /goal）
///
/// 核心公式：model.fit() = /goal
///
/// 训练循环自动化：
/// 1. 定义优化目标（Goal）
/// 2. 定义验证标准（GoalCondition）
/// 3. 定义约束（Constraint）
/// 4. 运行训练循环（GoalLoopRunner）
type CheckpointCallback = Box<dyn Fn(&GoalCheckpoint) + Send + Sync>;

pub struct GoalLoopRunner {
    evaluator: Box<dyn GoalEvaluator>,
    executor: Box<dyn RoundExecutor>,
    cancel_token: Option<GoalCancelToken>,
    checkpoint_callback: Option<CheckpointCallback>,
}

impl GoalLoopRunner {
    pub fn new(evaluator: Box<dyn GoalEvaluator>, executor: Box<dyn RoundExecutor>) -> Self {
        Self {
            evaluator,
            executor,
            cancel_token: None,
            checkpoint_callback: None,
        }
    }

    /// Create with default executor
    pub fn with_default_executor(evaluator: Box<dyn GoalEvaluator>) -> Self {
        Self::new(evaluator, Box::new(SimpleExecutor))
    }

    /// Set a cancellation token
    pub fn with_cancel_token(mut self, token: GoalCancelToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// Set a checkpoint callback (called after each round)
    pub fn with_checkpoint_callback(mut self, callback: Box<dyn Fn(&GoalCheckpoint) + Send + Sync>) -> Self {
        self.checkpoint_callback = Some(callback);
        self
    }

    /// Resume from a checkpoint
    pub async fn resume(&self, goal: Goal, checkpoint: GoalCheckpoint) -> KiasResult<GoalState> {
        tracing::info!(
            goal_id = %goal.id,
            resume_round = checkpoint.state.current_round,
            "Resuming goal loop from checkpoint"
        );

        let mut state = checkpoint.state;
        let mut last_evaluation = checkpoint.last_evaluation;

        // Continue the loop from where we left off
        self.run_inner(goal, &mut state, &mut last_evaluation).await
    }

    /// 运行目标循环（训练循环自动化）
    pub async fn run(&self, goal: Goal) -> KiasResult<GoalState> {
        let mut state = GoalState {
            goal_id: goal.id.clone(),
            status: GoalStatus::InProgress,
            current_round: 0,
            total_tokens: 0,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            evaluation_history: Vec::new(),
        };

        let mut last_evaluation: Option<EvaluationResult> = None;

        self.run_inner(goal, &mut state, &mut last_evaluation).await
    }

    /// Internal run loop (shared between run and resume)
    async fn run_inner(
        &self,
        goal: Goal,
        state: &mut GoalState,
        last_evaluation: &mut Option<EvaluationResult>,
    ) -> KiasResult<GoalState> {
        let start = Utc::now();
        tracing::info!(goal_id = %goal.id, description = %goal.description, "Starting goal loop");

        // 训练循环
        loop {
            // Check cancellation
            if let Some(ref token) = self.cancel_token {
                if token.is_cancelled() {
                    tracing::info!(goal_id = %goal.id, "Goal loop cancelled");
                    state.status = GoalStatus::Cancelled;
                    break;
                }
            }

            state.current_round += 1;
            state.updated_at = Utc::now();

            tracing::info!(round = state.current_round, "Starting round");

            // 检查是否超过最大轮数
            if let Some(max_rounds) = goal.max_rounds {
                if state.current_round > max_rounds {
                    tracing::warn!(max_rounds = max_rounds, "Max rounds reached");
                    state.status = GoalStatus::Failed;
                    break;
                }
            }

            // 执行一轮（Worker via executor callback）
            let round_output = self.executor.execute_round(
                &goal,
                state.current_round,
                last_evaluation.as_ref(),
            ).await?;

            // 评估目标是否达成（裁判分离 - 独立评估模型）
            let mut evaluation = self.evaluator.evaluate(&goal, &round_output).await?;
            evaluation.round = state.current_round;

            // 记录评估历史
            state.evaluation_history.push(evaluation.clone());
            *last_evaluation = Some(evaluation.clone());

            // Save checkpoint
            if self.checkpoint_callback.is_some() {
                let checkpoint = GoalCheckpoint::new(
                    &goal.id,
                    state.clone(),
                    Some(evaluation.clone()),
                );
                if let Some(ref callback) = self.checkpoint_callback {
                    callback(&checkpoint);
                }
            }

            // 判断是否达成
            if evaluation.achieved {
                tracing::info!(round = state.current_round, "Goal achieved!");
                state.status = GoalStatus::Achieved;
                break;
            } else {
                tracing::info!(
                    round = state.current_round,
                    reason = %evaluation.reason,
                    "Goal not achieved, continuing"
                );
                state.status = GoalStatus::NotAchieved;
            }
        }

        let duration = Utc::now().signed_duration_since(start);
        tracing::info!(
            goal_id = %goal.id,
            status = ?state.status,
            rounds = state.current_round,
            duration_ms = duration.num_milliseconds(),
            "Goal loop completed"
        );

        Ok(state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goal::Goal;

    struct FeedbackTrackingExecutor {
        outputs: std::sync::Mutex<Vec<String>>,
    }

    impl FeedbackTrackingExecutor {
        fn new_from_vec(outputs: Vec<String>) -> Self {
            Self {
                outputs: std::sync::Mutex::new(outputs),
            }
        }
    }

    #[async_trait::async_trait]
    impl RoundExecutor for FeedbackTrackingExecutor {
        async fn execute_round(
            &self,
            _goal: &Goal,
            round: u32,
            _previous_feedback: Option<&EvaluationResult>,
        ) -> KiasResult<String> {
            // Yield to allow cancellation tasks to run
            tokio::task::yield_now().await;
            let outputs = self.outputs.lock().unwrap();
            let idx = (round - 1) as usize;
            if idx < outputs.len() {
                Ok(outputs[idx].clone())
            } else {
                Ok(format!("Round {} default", round))
            }
        }
    }

    #[tokio::test]
    async fn test_goal_loop_achieved_first_round() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let executor = FeedbackTrackingExecutor::new_from_vec(vec!["success".to_string()]);
        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(executor));

        let mut goal = Goal::new("test");
        goal.add_condition("done", "achieve success", "contains", "success");
        goal.set_max_rounds(5);

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.status, GoalStatus::Achieved);
        assert_eq!(state.current_round, 1);
    }

    #[tokio::test]
    async fn test_goal_loop_achieved_later_round() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let executor = FeedbackTrackingExecutor::new_from_vec(vec![
            "fail".to_string(),
            "fail".to_string(),
            "success".to_string(),
        ]);
        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(executor));

        let mut goal = Goal::new("test");
        goal.add_condition("done", "achieve success", "contains", "success");
        goal.set_max_rounds(10);

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.status, GoalStatus::Achieved);
        assert_eq!(state.current_round, 3);
    }

    #[tokio::test]
    async fn test_goal_loop_max_rounds_exceeded() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let executor = FeedbackTrackingExecutor::new_from_vec(vec![
            "fail".to_string(); 10
        ]);
        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(executor));

        let mut goal = Goal::new("test");
        goal.add_condition("done", "achieve success", "contains", "success");
        goal.set_max_rounds(3);

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.status, GoalStatus::Failed);
        assert_eq!(state.current_round, 4); // went one past max
    }

    #[tokio::test]
    async fn test_goal_loop_cancellation() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let cancel_token = GoalCancelToken::new();
        let cancel_clone = cancel_token.clone();

        // Use a slow executor that gives time for cancellation
        struct SlowExecutor;
        #[async_trait::async_trait]
        impl RoundExecutor for SlowExecutor {
            async fn execute_round(&self, _: &Goal, _: u32, _: Option<&EvaluationResult>) -> KiasResult<String> {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                Ok("fail".to_string())
            }
        }

        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(SlowExecutor))
            .with_cancel_token(cancel_token);

        // Cancel after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });

        let mut goal = Goal::new("test");
        goal.add_condition("done", "impossible", "contains", "never");
        goal.set_max_rounds(1000);

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.status, GoalStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_goal_checkpoint_serialization() {
        let state = GoalState {
            goal_id: "test-goal".to_string(),
            status: GoalStatus::InProgress,
            current_round: 3,
            total_tokens: 1000,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            evaluation_history: vec![],
        };

        let checkpoint = GoalCheckpoint::new("test-goal", state, None);
        let json = checkpoint.to_json().unwrap();
        let restored = GoalCheckpoint::from_json(&json).unwrap();

        assert_eq!(restored.goal_id, "test-goal");
        assert_eq!(restored.state.current_round, 3);
    }

    #[tokio::test]
    async fn test_goal_loop_checkpoint_callback() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let executor = FeedbackTrackingExecutor::new_from_vec(vec![
            "fail".to_string(),
            "success".to_string(),
        ]);

        let checkpoints = Arc::new(std::sync::Mutex::new(Vec::new()));
        let checkpoints_clone = checkpoints.clone();

        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(executor))
            .with_checkpoint_callback(Box::new(move |cp| {
                checkpoints_clone.lock().unwrap().push(cp.clone());
            }));

        let mut goal = Goal::new("test");
        goal.add_condition("done", "achieve success", "contains", "success");

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.status, GoalStatus::Achieved);

        let saved = checkpoints.lock().unwrap();
        assert_eq!(saved.len(), 2); // Two rounds = two checkpoints
    }

    #[tokio::test]
    async fn test_goal_loop_evaluation_history() {
        let evaluator = crate::evaluator::DefaultEvaluator::new();
        let executor = FeedbackTrackingExecutor::new_from_vec(vec![
            "fail".to_string(),
            "fail".to_string(),
            "success".to_string(),
        ]);
        let runner = GoalLoopRunner::new(Box::new(evaluator), Box::new(executor));

        let mut goal = Goal::new("test");
        goal.add_condition("done", "achieve success", "contains", "success");

        let state = runner.run(goal).await.unwrap();
        assert_eq!(state.evaluation_history.len(), 3);
        assert!(!state.evaluation_history[0].achieved);
        assert!(!state.evaluation_history[1].achieved);
        assert!(state.evaluation_history[2].achieved);
    }

    #[test]
    fn test_cancel_token() {
        let token = GoalCancelToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }
}
