use serde::{Deserialize, Serialize};

/// 工作流边（连接节点）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub condition: Option<Condition>,
}

/// 条件表达式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub expression: String,
    pub description: String,
}

impl Edge {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            condition: None,
        }
    }

    pub fn with_condition(mut self, expression: &str, description: &str) -> Self {
        self.condition = Some(Condition {
            expression: expression.to_string(),
            description: description.to_string(),
        });
        self
    }
}
