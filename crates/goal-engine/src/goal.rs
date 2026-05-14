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
