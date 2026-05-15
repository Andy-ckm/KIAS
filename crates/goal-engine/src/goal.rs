use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 目标定义（借鉴 Claude Code /goal）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub description: String,
    pub conditions: Vec<GoalCondition>,
    pub constraints: Vec<Constraint>,
    pub max_rounds: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 目标条件（好目标三要素之一：可衡量的终态）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalCondition {
    pub name: String,
    pub description: String,
    pub verification_method: String, // 验证方式
    pub expected_result: String,
}

/// 约束（好目标三要素之一：不能破坏的约束）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    pub description: String,
    pub check_method: String,
}

/// 目标状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GoalStatus {
    /// 待开始
    Pending,
    /// 进行中
    InProgress,
    /// 已达成
    Achieved,
    /// 未达成（继续）
    NotAchieved,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 目标运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalState {
    pub goal_id: String,
    pub status: GoalStatus,
    pub current_round: u32,
    pub total_tokens: u64,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub evaluation_history: Vec<EvaluationResult>,
}

/// 评估结果（裁判分离）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub round: u32,
    pub achieved: bool,
    pub reason: String,
    pub suggestions: Vec<String>,
    pub evaluated_at: DateTime<Utc>,
}

impl Goal {
    pub fn new(description: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description: description.to_string(),
            conditions: Vec::new(),
            constraints: Vec::new(),
            max_rounds: Some(20), // 默认20轮
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// 添加条件
    pub fn add_condition(
        &mut self,
        name: &str,
        description: &str,
        verification: &str,
        expected: &str,
    ) {
        self.conditions.push(GoalCondition {
            name: name.to_string(),
            description: description.to_string(),
            verification_method: verification.to_string(),
            expected_result: expected.to_string(),
        });
        self.updated_at = Utc::now();
    }

    /// 添加约束
    pub fn add_constraint(&mut self, name: &str, description: &str, check_method: &str) {
        self.constraints.push(Constraint {
            name: name.to_string(),
            description: description.to_string(),
            check_method: check_method.to_string(),
        });
        self.updated_at = Utc::now();
    }

    /// 设置最大轮数
    pub fn set_max_rounds(&mut self, max_rounds: u32) {
        self.max_rounds = Some(max_rounds);
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_new_defaults() {
        let goal = Goal::new("Build a REST API");
        assert_eq!(goal.description, "Build a REST API");
        assert!(goal.conditions.is_empty());
        assert!(goal.constraints.is_empty());
        assert_eq!(goal.max_rounds, Some(20));
        assert!(!goal.id.is_empty());
    }

    #[test]
    fn test_goal_new_generates_unique_ids() {
        let g1 = Goal::new("task 1");
        let g2 = Goal::new("task 2");
        assert_ne!(g1.id, g2.id);
    }

    #[test]
    fn test_add_condition() {
        let mut goal = Goal::new("test");
        goal.add_condition("c1", "must compile", "cargo build", "exit 0");
        assert_eq!(goal.conditions.len(), 1);
        assert_eq!(goal.conditions[0].name, "c1");
        assert_eq!(goal.conditions[0].verification_method, "cargo build");
    }

    #[test]
    fn test_add_multiple_conditions() {
        let mut goal = Goal::new("test");
        goal.add_condition("c1", "compiles", "cargo build", "ok");
        goal.add_condition("c2", "tests pass", "cargo test", "ok");
        assert_eq!(goal.conditions.len(), 2);
    }

    #[test]
    fn test_add_constraint() {
        let mut goal = Goal::new("test");
        goal.add_constraint("no-unsafe", "No unsafe code", "grep");
        assert_eq!(goal.constraints.len(), 1);
        assert_eq!(goal.constraints[0].name, "no-unsafe");
    }

    #[test]
    fn test_set_max_rounds() {
        let mut goal = Goal::new("test");
        goal.set_max_rounds(50);
        assert_eq!(goal.max_rounds, Some(50));
    }

    #[test]
    fn test_goal_status_partial_eq() {
        assert_eq!(GoalStatus::Pending, GoalStatus::Pending);
        assert_ne!(GoalStatus::Pending, GoalStatus::Achieved);
        assert_ne!(GoalStatus::InProgress, GoalStatus::Failed);
    }

    #[test]
    fn test_goal_clone() {
        let mut goal = Goal::new("original");
        goal.add_condition("c1", "desc", "method", "expected");
        goal.add_constraint("k1", "desc", "method");
        let cloned = goal.clone();
        assert_eq!(cloned.description, "original");
        assert_eq!(cloned.conditions.len(), 1);
        assert_eq!(cloned.constraints.len(), 1);
    }

    #[test]
    fn test_goal_state_defaults() {
        let state = GoalState {
            goal_id: "g1".to_string(),
            status: GoalStatus::Pending,
            current_round: 0,
            total_tokens: 0,
            started_at: Utc::now(),
            updated_at: Utc::now(),
            evaluation_history: Vec::new(),
        };
        assert_eq!(state.status, GoalStatus::Pending);
        assert_eq!(state.current_round, 0);
        assert!(state.evaluation_history.is_empty());
    }

    #[test]
    fn test_evaluation_result() {
        let result = EvaluationResult {
            round: 1,
            achieved: true,
            reason: "All tests pass".to_string(),
            suggestions: vec!["Add more edge cases".to_string()],
            evaluated_at: Utc::now(),
        };
        assert!(result.achieved);
        assert_eq!(result.round, 1);
        assert_eq!(result.suggestions.len(), 1);
    }

    #[test]
    fn test_goal_updated_at_changes_on_add_condition() {
        let mut goal = Goal::new("test");
        let before = goal.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        goal.add_condition("c1", "d", "v", "e");
        assert!(goal.updated_at > before);
    }

    #[test]
    fn test_goal_updated_at_changes_on_add_constraint() {
        let mut goal = Goal::new("test");
        let before = goal.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        goal.add_constraint("k1", "d", "c");
        assert!(goal.updated_at > before);
    }

    #[test]
    fn test_goal_updated_at_changes_on_set_max_rounds() {
        let mut goal = Goal::new("test");
        let before = goal.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        goal.set_max_rounds(10);
        assert!(goal.updated_at > before);
    }
}
