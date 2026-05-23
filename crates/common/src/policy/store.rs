//! In-memory policy store implementation

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;

use super::engine::{PolicyEngine, PolicyEvaluationResult};
use super::rule::Effect;
use super::rule::PolicyRule;
use super::PolicyAuditLog;

/// In-memory policy store with read-write lock
pub struct InMemoryPolicyStore {
    rules: RwLock<HashMap<String, PolicyRule>>,
    audit_log: RwLock<PolicyAuditLog>,
}

impl InMemoryPolicyStore {
    /// Creates a new empty InMemoryPolicyStore
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(HashMap::new()),
            audit_log: RwLock::new(PolicyAuditLog::new()),
        }
    }

    /// Returns a read-only view of the audit log
    pub fn get_audit_log(&self) -> PolicyAuditLog {
        self.audit_log.read().unwrap().clone()
    }

    /// Returns all rules for a specific resource pattern
    fn get_applicable_rules(&self, resource: &str) -> Vec<PolicyRule> {
        let rules = self.rules.read().unwrap();
        rules
            .values()
            .filter(|r| self.resource_matches(&r.resource, resource))
            .cloned()
            .collect()
    }

    /// Simple glob-style matching for resource patterns
    fn resource_matches(&self, pattern: &str, resource: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern == resource {
            return true;
        }
        // Support simple patterns like "agent:*" matching "agent:worker-1"
        if let Some((p_prefix, p_suffix)) = pattern.split_once('*') {
            if p_suffix.is_empty() {
                return resource.starts_with(p_prefix);
            }
            if p_prefix.is_empty() {
                return resource.ends_with(p_suffix);
            }
            return resource.starts_with(p_prefix) && resource.ends_with(p_suffix);
        }
        false
    }
}

impl Default for InMemoryPolicyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PolicyEngine for InMemoryPolicyStore {
    async fn evaluate(
        &self,
        resource: &str,
        action: &str,
        context: std::collections::HashMap<String, String>,
    ) -> PolicyEvaluationResult {
        let applicable_rules = self.get_applicable_rules(resource);

        // Default deny if no rules match
        if applicable_rules.is_empty() {
            let result = PolicyEvaluationResult {
                allowed: false,
                matched_rule_id: None,
                reason: "No applicable policy found".to_string(),
            };
            self.audit_log
                .write()
                .unwrap()
                .append(resource, action, &result);
            return result;
        }

        // Evaluate rules in order (Deny takes precedence)
        for rule in &applicable_rules {
            if rule.evaluate_conditions(&context) {
                let allowed = rule.effect == Effect::Allow;
                let result = PolicyEvaluationResult {
                    allowed,
                    matched_rule_id: Some(rule.id.clone()),
                    reason: if allowed {
                        format!("Allowed by rule: {}", rule.name)
                    } else {
                        format!("Denied by rule: {}", rule.name)
                    },
                };
                self.audit_log
                    .write()
                    .unwrap()
                    .append(resource, action, &result);
                return result;
            }
        }

        // No conditions matched -> deny
        let result = PolicyEvaluationResult {
            allowed: false,
            matched_rule_id: None,
            reason: "No conditions matched".to_string(),
        };
        self.audit_log
            .write()
            .unwrap()
            .append(resource, action, &result);
        result
    }

    async fn add_rule(&self, rule: PolicyRule) -> Result<(), String> {
        let mut rules = self.rules.write().unwrap();
        rules.insert(rule.id.clone(), rule);
        Ok(())
    }

    async fn remove_rule(&self, rule_id: &str) -> Result<(), String> {
        let mut rules = self.rules.write().unwrap();
        if rules.remove(rule_id).is_some() {
            Ok(())
        } else {
            Err(format!("Rule not found: {}", rule_id))
        }
    }

    async fn list_rules(&self) -> Vec<PolicyRule> {
        let rules = self.rules.read().unwrap();
        rules.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_store() -> InMemoryPolicyStore {
        let store = InMemoryPolicyStore::new();

        // Add test rules synchronously
        let rule1 = PolicyRule::new(
            "allow-admin",
            "Allow admin actions",
            "agent:*",
            Effect::Allow,
            vec![Condition::new_string("role", ConditionOperator::Equals, "admin".to_string())],
            1,
        );
        let rule2 = PolicyRule::new(
            "deny-dangerous",
            "Deny dangerous tools",
            "tool:*",
            Effect::Deny,
            vec![Condition::new_string("tool_name", ConditionOperator::Contains, "dangerous".to_string())],
            1,
        );

        // Manually insert for testing
        {
            let mut rules = store.rules.write().unwrap();
            rules.insert(rule1.id.clone(), rule1);
            rules.insert(rule2.id.clone(), rule2);
        }

        store
    }

    #[tokio::test]
    async fn test_evaluate_allow() {
        let store = create_test_store();
        let mut ctx = HashMap::new();
        ctx.insert("role".to_string(), "admin".to_string());

        let result = store.evaluate("agent:worker-1", "start", ctx).await;
        assert!(result.allowed);
        assert_eq!(result.matched_rule_id, Some("allow-admin".to_string()));
    }

    #[tokio::test]
    async fn test_evaluate_deny() {
        let store = create_test_store();
        let mut ctx = HashMap::new();
        ctx.insert("tool_name".to_string(), "dangerous_tool".to_string());

        let result = store.evaluate("tool:exec", "run", ctx).await;
        assert!(!result.allowed);
        assert_eq!(result.matched_rule_id, Some("deny-dangerous".to_string()));
    }

    #[tokio::test]
    async fn test_evaluate_no_match() {
        let store = create_test_store();
        let ctx = HashMap::new();

        let result = store.evaluate("unknown:resource", "action", ctx).await;
        assert!(!result.allowed);
        assert_eq!(result.matched_rule_id, None);
    }

    #[tokio::test]
    async fn test_add_remove_rule() {
        let store = InMemoryPolicyStore::new();

        let rule = PolicyRule::new(
            "test-rule",
            "Test Rule",
            "test:*",
            Effect::Allow,
            vec![],
            1,
        );

        store.add_rule(rule.clone()).await.unwrap();
        let rules = store.list_rules().await;
        assert_eq!(rules.len(), 1);

        store.remove_rule("test-rule").await.unwrap();
        let rules = store.list_rules().await;
        assert!(rules.is_empty());
    }

    #[tokio::test]
    async fn test_audit_log_created() {
        let store = InMemoryPolicyStore::new();
        let ctx = HashMap::new();
        store.evaluate("test:resource", "action", ctx).await;

        let audit = store.get_audit_log();
        assert_eq!(audit.len(), 1);
    }
}
