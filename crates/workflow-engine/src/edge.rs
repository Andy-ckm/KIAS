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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Edge::new ──────────────────────────────────────────────

    #[test]
    fn test_edge_new_basic() {
        let e = Edge::new("a", "b");
        assert_eq!(e.from, "a");
        assert_eq!(e.to, "b");
        assert!(e.condition.is_none());
    }

    #[test]
    fn test_edge_new_with_empty_strings() {
        let e = Edge::new("", "");
        assert_eq!(e.from, "");
        assert_eq!(e.to, "");
    }

    #[test]
    fn test_edge_new_with_same_from_to() {
        let e = Edge::new("loop", "loop");
        assert_eq!(e.from, e.to);
    }

    // ── Edge::with_condition ───────────────────────────────────

    #[test]
    fn test_edge_with_condition() {
        let e = Edge::new("start", "end").with_condition("status == 'ok'", "check status");
        let cond = e.condition.as_ref().unwrap();
        assert_eq!(cond.expression, "status == 'ok'");
        assert_eq!(cond.description, "check status");
    }

    #[test]
    fn test_edge_with_condition_chains_fields() {
        let e = Edge::new("x", "y").with_condition("true", "always");
        assert_eq!(e.from, "x");
        assert_eq!(e.to, "y");
        assert!(e.condition.is_some());
    }

    #[test]
    fn test_edge_builder_returns_self() {
        let e = Edge::new("a", "b")
            .with_condition("1", "one")
            .with_condition("2", "two");
        // Second call overwrites first
        let cond = e.condition.as_ref().unwrap();
        assert_eq!(cond.expression, "2");
    }

    // ── Condition ──────────────────────────────────────────────

    #[test]
    fn test_condition_fields() {
        let c = Condition {
            expression: "x > 0".into(),
            description: "positive check".into(),
        };
        assert_eq!(c.expression, "x > 0");
        assert_eq!(c.description, "positive check");
    }

    // ── Serialization ──────────────────────────────────────────

    #[test]
    fn test_edge_serialize_roundtrip() {
        let e = Edge::new("s", "t").with_condition("ok", "desc");
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.contains("s"));
        assert!(json.contains("ok"));
        let roundtrip: Edge = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.from, e.from);
        assert_eq!(roundtrip.to, e.to);
        assert_eq!(roundtrip.condition.as_ref().unwrap().expression, "ok");
    }

    #[test]
    fn test_edge_no_condition_serializes_null() {
        let e = Edge::new("a", "b");
        let json = serde_json::to_string(&e).unwrap();
        let roundtrip: Edge = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.condition.is_none());
    }

    #[test]
    fn test_edge_clone() {
        let e = Edge::new("a", "b").with_condition("c", "d");
        let e2 = e.clone();
        assert_eq!(e.from, e2.from);
        assert_eq!(
            e.condition.unwrap().expression,
            e2.condition.unwrap().expression
        );
    }

    #[test]
    fn test_edge_debug() {
        let e = Edge::new("a", "b");
        let dbg = format!("{:?}", e);
        assert!(dbg.contains("Edge"));
        assert!(dbg.contains("a"));
    }
}
