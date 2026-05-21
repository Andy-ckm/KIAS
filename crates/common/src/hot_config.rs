use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use tracing::info;

/// Hot config reload — update configuration without restarting.
///
/// Inspired by EMQ's emqx_conf:
/// - Config handler registry (register callbacks for config sections)
/// - Atomic config updates with validation
/// - Config change history for audit trail
/// - Rollback support

/// Configuration value (JSON)
pub type ConfigValue = serde_json::Value;

/// Config change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    /// Config key path (e.g., "auth.jwt.secret")
    pub key_path: String,

    /// Old value (if existed)
    pub old_value: Option<ConfigValue>,

    /// New value
    pub new_value: ConfigValue,

    /// When the change happened
    pub timestamp: DateTime<Utc>,

    /// Who made the change
    pub changed_by: String,

    /// Whether the change was validated
    pub validated: bool,
}

/// Config validation result
#[derive(Debug, Clone)]
pub struct ConfigValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Config handler callback
pub type ConfigHandler = Box<dyn Fn(&ConfigValue) -> ConfigValidationResult + Send + Sync>;

/// Hot config manager
pub struct HotConfig {
    /// Current configuration (key_path -> value)
    config: Arc<RwLock<HashMap<String, ConfigValue>>>,

    /// Registered handlers (key_path -> handler)
    handlers: Arc<RwLock<HashMap<String, Arc<ConfigHandler>>>>,

    /// Change history for audit
    history: Arc<RwLock<Vec<ConfigChangeEvent>>>,

    /// Max history entries
    max_history: usize,
}

impl HotConfig {
    pub fn new(max_history: usize) -> Self {
        Self {
            config: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            max_history,
        }
    }

    /// Register a config handler for a key path
    pub async fn register_handler(
        &self,
        key_path: &str,
        handler: impl Fn(&ConfigValue) -> ConfigValidationResult + Send + Sync + 'static,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(key_path.to_string(), Arc::new(Box::new(handler)));
        info!(key_path = %key_path, "Config handler registered");
    }

    /// Get a config value
    pub async fn get(&self, key_path: &str) -> Option<ConfigValue> {
        let config = self.config.read().await;
        config.get(key_path).cloned()
    }

    /// Get all config
    pub async fn get_all(&self) -> HashMap<String, ConfigValue> {
        let config = self.config.read().await;
        config.clone()
    }

    /// Update a config value (with validation and audit)
    pub async fn update(
        &self,
        key_path: &str,
        new_value: ConfigValue,
        changed_by: &str,
    ) -> Result<ConfigChangeEvent, ConfigError> {
        // Validate if handler exists
        let handlers = self.handlers.read().await;
        if let Some(handler) = handlers.get(key_path) {
            let validation = handler(&new_value);
            if !validation.valid {
                return Err(ConfigError::ValidationFailed {
                    errors: validation.errors,
                });
            }
        }
        drop(handlers);

        // Get old value
        let old_value = {
            let config = self.config.read().await;
            config.get(key_path).cloned()
        };

        // Apply new value
        {
            let mut config = self.config.write().await;
            config.insert(key_path.to_string(), new_value.clone());
        }

        // Record change event
        let event = ConfigChangeEvent {
            key_path: key_path.to_string(),
            old_value,
            new_value,
            timestamp: Utc::now(),
            changed_by: changed_by.to_string(),
            validated: true,
        };

        // Add to history
        {
            let mut history = self.history.write().await;
            if history.len() >= self.max_history {
                history.drain(..self.max_history / 10);
            }
            history.push(event.clone());
        }

        info!(
            key_path = %key_path,
            changed_by = %changed_by,
            "Config updated"
        );

        Ok(event)
    }

    /// Remove a config key
    pub async fn remove(
        &self,
        key_path: &str,
        changed_by: &str,
    ) -> Result<Option<ConfigChangeEvent>, ConfigError> {
        let old_value = {
            let mut config = self.config.write().await;
            config.remove(key_path)
        };

        if let Some(ref old) = old_value {
            let event = ConfigChangeEvent {
                key_path: key_path.to_string(),
                old_value: Some(old.clone()),
                new_value: serde_json::Value::Null,
                timestamp: Utc::now(),
                changed_by: changed_by.to_string(),
                validated: true,
            };

            let mut history = self.history.write().await;
            if history.len() >= self.max_history {
                history.drain(..self.max_history / 10);
            }
            history.push(event.clone());

            info!(key_path = %key_path, changed_by = %changed_by, "Config removed");
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    /// Rollback to a previous value
    pub async fn rollback(
        &self,
        key_path: &str,
        changed_by: &str,
    ) -> Result<Option<ConfigChangeEvent>, ConfigError> {
        let old_value = {
            let history = self.history.read().await;
            history
                .iter()
                .rev()
                .find(|e| e.key_path == key_path && e.old_value.is_some())
                .and_then(|e| e.old_value.clone())
        };

        if let Some(old_value) = old_value {
            self.update(key_path, old_value, changed_by).await.map(Some)
        } else {
            Err(ConfigError::NoRollbackTarget {
                key_path: key_path.to_string(),
            })
        }
    }

    /// Get change history for a key path
    pub async fn history(&self, key_path: &str) -> Vec<ConfigChangeEvent> {
        let history = self.history.read().await;
        history
            .iter()
            .filter(|e| e.key_path == key_path)
            .cloned()
            .collect()
    }

    /// Get total change count
    pub async fn change_count(&self) -> usize {
        let history = self.history.read().await;
        history.len()
    }

    /// Bulk update (atomic — all or nothing)
    pub async fn bulk_update(
        &self,
        updates: Vec<(String, ConfigValue)>,
        changed_by: &str,
    ) -> Result<Vec<ConfigChangeEvent>, ConfigError> {
        let mut events = Vec::new();

        // Validate all first
        let handlers = self.handlers.read().await;
        for (key_path, value) in &updates {
            if let Some(handler) = handlers.get(key_path) {
                let validation = handler(value);
                if !validation.valid {
                    return Err(ConfigError::ValidationFailed {
                        errors: validation.errors,
                    });
                }
            }
        }
        drop(handlers);

        // Apply all
        for (key_path, value) in updates {
            let event = self.update(&key_path, value, changed_by).await?;
            events.push(event);
        }

        Ok(events)
    }
}

impl Default for HotConfig {
    fn default() -> Self {
        Self::new(10_000)
    }
}

/// Config errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },

    #[error("No rollback target for key: {key_path}")]
    NoRollbackTarget { key_path: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_basic_update_and_get() {
        let config = HotConfig::default();

        config.update("app.name", json!("AgentGuard"), "admin").await.unwrap();

        assert_eq!(config.get("app.name").await, Some(json!("AgentGuard")));
    }

    #[tokio::test]
    async fn test_handler_validation() {
        let config = HotConfig::default();

        config.register_handler("app.port", |value| {
            if let Some(port) = value.as_u64() {
                if port > 0 && port < 65536 {
                    return ConfigValidationResult { valid: true, errors: vec![] };
                }
            }
            ConfigValidationResult {
                valid: false,
                errors: vec!["Port must be 1-65535".to_string()],
            }
        }).await;

        // Valid port
        let result = config.update("app.port", json!(8080), "admin").await;
        assert!(result.is_ok());

        // Invalid port
        let result = config.update("app.port", json!(99999), "admin").await;
        assert!(matches!(result, Err(ConfigError::ValidationFailed { .. })));
    }

    #[tokio::test]
    async fn test_rollback() {
        let config = HotConfig::default();

        config.update("app.debug", json!(false), "admin").await.unwrap();
        config.update("app.debug", json!(true), "dev").await.unwrap();

        assert_eq!(config.get("app.debug").await, Some(json!(true)));

        config.rollback("app.debug", "admin").await.unwrap();

        assert_eq!(config.get("app.debug").await, Some(json!(false)));
    }

    #[tokio::test]
    async fn test_history() {
        let config = HotConfig::default();

        config.update("key1", json!("v1"), "admin").await.unwrap();
        config.update("key1", json!("v2"), "admin").await.unwrap();
        config.update("key2", json!("v3"), "admin").await.unwrap();

        let history = config.history("key1").await;
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].new_value, json!("v1"));
        assert_eq!(history[1].new_value, json!("v2"));
    }

    #[tokio::test]
    async fn test_remove() {
        let config = HotConfig::default();

        config.update("key1", json!("value"), "admin").await.unwrap();
        assert!(config.get("key1").await.is_some());

        config.remove("key1", "admin").await.unwrap();
        assert!(config.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_bulk_update() {
        let config = HotConfig::default();

        let updates = vec![
            ("a".to_string(), json!(1)),
            ("b".to_string(), json!(2)),
            ("c".to_string(), json!(3)),
        ];

        let events = config.bulk_update(updates, "admin").await.unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(config.get("a").await, Some(json!(1)));
        assert_eq!(config.get("b").await, Some(json!(2)));
    }

    #[tokio::test]
    async fn test_bulk_update_rollback_on_validation_failure() {
        let config = HotConfig::default();

        config.register_handler("b", |value| {
            if value.as_i64() == Some(2) {
                ConfigValidationResult {
                    valid: false,
                    errors: vec!["2 is not allowed".to_string()],
                }
            } else {
                ConfigValidationResult { valid: true, errors: vec![] }
            }
        }).await;

        let updates = vec![
            ("a".to_string(), json!(1)),
            ("b".to_string(), json!(2)), // This will fail
        ];

        let result = config.bulk_update(updates, "admin").await;
        assert!(result.is_err());
        // a should not be updated because bulk is atomic
        assert!(config.get("a").await.is_none());
    }

    #[tokio::test]
    async fn test_change_count() {
        let config = HotConfig::default();

        config.update("a", json!(1), "admin").await.unwrap();
        config.update("b", json!(2), "admin").await.unwrap();

        assert_eq!(config.change_count().await, 2);
    }
}
