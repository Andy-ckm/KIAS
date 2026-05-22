//! 契约测试框架
//!
//! 提供 API/事件/策略契约定义与验证功能。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 契约类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractType {
    Api,
    Event,
    Policy,
}

/// API 契约定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContract {
    pub name: String,
    pub version: String,
    pub endpoint: String,
    pub method: String,
    pub request_schema: HashMap<String, String>,
    pub response_schema: HashMap<String, String>,
}

/// 事件契约定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventContract {
    pub name: String,
    pub version: String,
    pub topic: String,
    pub payload_schema: HashMap<String, String>,
}

/// 策略契约定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyContract {
    pub name: String,
    pub version: String,
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub description: String,
    pub condition: String,
}

/// 契约定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub contract_type: ContractType,
    pub name: String,
    pub version: String,
    /// API/Event/Policy 契约的具体定义
    pub api_contract: Option<ApiContract>,
    pub event_contract: Option<EventContract>,
    pub policy_contract: Option<PolicyContract>,
}

impl Contract {
    pub fn api(api: ApiContract) -> Self {
        Self {
            contract_type: ContractType::Api,
            name: api.name.clone(),
            version: api.version.clone(),
            api_contract: Some(api),
            event_contract: None,
            policy_contract: None,
        }
    }

    pub fn event(event: EventContract) -> Self {
        Self {
            contract_type: ContractType::Event,
            name: event.name.clone(),
            version: event.version.clone(),
            api_contract: None,
            event_contract: Some(event),
            policy_contract: None,
        }
    }

    pub fn policy(policy: PolicyContract) -> Self {
        Self {
            contract_type: ContractType::Policy,
            name: policy.name.clone(),
            version: policy.version.clone(),
            api_contract: None,
            event_contract: None,
            policy_contract: Some(policy),
        }
    }
}

/// 违约报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractViolation {
    pub contract_name: String,
    pub violation_type: ViolationType,
    pub message: String,
    pub severity: ViolationSeverity,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationType {
    MissingField,
    TypeMismatch,
    SchemaViolation,
    VersionMismatch,
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViolationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// 契约验证器
pub struct ContractValidator {
    contracts: HashMap<String, Contract>,
}

impl Default for ContractValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl ContractValidator {
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
        }
    }

    /// 注册契约
    pub fn register(&mut self, contract: Contract) -> Result<(), ContractError> {
        let key = format!("{}:{}", contract.name, contract.version);
        if self.contracts.contains_key(&key) {
            return Err(ContractError::AlreadyRegistered(contract.name.clone()));
        }
        self.contracts.insert(key, contract);
        Ok(())
    }

    /// 验证 API 契约
    pub fn validate_api(
        &self,
        name: &str,
        version: &str,
        data: &HashMap<String, String>,
    ) -> Vec<ContractViolation> {
        let key = format!("{}:{}", name, version);
        let mut violations = Vec::new();
        if let Some(contract) = self.contracts.get(&key) {
            if let Some(api) = &contract.api_contract {
                for (field, expected_type) in &api.request_schema {
                    if !data.contains_key(field) {
                        violations.push(ContractViolation {
                            contract_name: name.to_string(),
                            violation_type: ViolationType::MissingField,
                            message: format!("Missing required field: {}", field),
                            severity: ViolationSeverity::High,
                            timestamp: chrono::Utc::now(),
                        });
                    } else if let Some(value) = data.get(field) {
                        if !Self::type_matches(value, expected_type) {
                            violations.push(ContractViolation {
                                contract_name: name.to_string(),
                                violation_type: ViolationType::TypeMismatch,
                                message: format!(
                                    "Field {} type mismatch, expected {}",
                                    field, expected_type
                                ),
                                severity: ViolationSeverity::Medium,
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }
                }
            }
        }
        violations
    }

    fn type_matches(value: &str, expected_type: &str) -> bool {
        match expected_type {
            "string" => !value.is_empty(),
            "number" => value.parse::<f64>().is_ok(),
            "boolean" => value == "true" || value == "false",
            _ => true,
        }
    }

    /// 验证事件契约
    pub fn validate_event(
        &self,
        name: &str,
        version: &str,
        payload: &HashMap<String, String>,
    ) -> Vec<ContractViolation> {
        let key = format!("{}:{}", name, version);
        let mut violations = Vec::new();
        if let Some(contract) = self.contracts.get(&key) {
            if let Some(event) = &contract.event_contract {
                for (field, expected_type) in &event.payload_schema {
                    if !payload.contains_key(field) {
                        violations.push(ContractViolation {
                            contract_name: name.to_string(),
                            violation_type: ViolationType::MissingField,
                            message: format!("Missing event field: {}", field),
                            severity: ViolationSeverity::High,
                            timestamp: chrono::Utc::now(),
                        });
                    }
                }
            }
        }
        violations
    }

    /// 获取契约
    pub fn get(&self, name: &str, version: &str) -> Option<&Contract> {
        let key = format!("{}:{}", name, version);
        self.contracts.get(&key)
    }

    /// 列出所有契约
    pub fn list_all(&self) -> Vec<&Contract> {
        self.contracts.values().collect()
    }

    /// 按类型查找契约
    pub fn find_by_type(&self, contract_type: ContractType) -> Vec<&Contract> {
        self.contracts
            .values()
            .filter(|c| c.contract_type == contract_type)
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    #[error("Contract `{0}` already registered")]
    AlreadyRegistered(String),
    #[error("Contract `{0}` not found")]
    NotFound(String),
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_api_contract() -> Contract {
        Contract::api(ApiContract {
            name: "test-api".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "/api/test".to_string(),
            method: "POST".to_string(),
            request_schema: HashMap::from([
                ("id".to_string(), "string".to_string()),
                ("value".to_string(), "number".to_string()),
            ]),
            response_schema: HashMap::new(),
        })
    }

    fn create_test_event_contract() -> Contract {
        Contract::event(EventContract {
            name: "test-event".to_string(),
            version: "1.0.0".to_string(),
            topic: "test.topic".to_string(),
            payload_schema: HashMap::from([("event_id".to_string(), "string".to_string())]),
        })
    }

    #[test]
    fn test_contract_validator_register() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        assert_eq!(validator.list_all().len(), 1);
    }

    #[test]
    fn test_contract_validator_register_duplicate() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        let result = validator.register(create_test_api_contract());
        assert!(result.is_err());
    }

    #[test]
    fn test_contract_validator_validate_api_success() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        let data = HashMap::from([
            ("id".to_string(), "123".to_string()),
            ("value".to_string(), "42.5".to_string()),
        ]);
        let violations = validator.validate_api("test-api", "1.0.0", &data);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_contract_validator_validate_api_missing_field() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        let data = HashMap::from([("id".to_string(), "123".to_string())]);
        let violations = validator.validate_api("test-api", "1.0.0", &data);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, ViolationType::MissingField);
    }

    #[test]
    fn test_contract_validator_validate_api_type_mismatch() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        let data = HashMap::from([
            ("id".to_string(), "123".to_string()),
            ("value".to_string(), "not-a-number".to_string()),
        ]);
        let violations = validator.validate_api("test-api", "1.0.0", &data);
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_contract_validator_find_by_type() {
        let mut validator = ContractValidator::new();
        validator.register(create_test_api_contract()).unwrap();
        validator.register(create_test_event_contract()).unwrap();
        let apis = validator.find_by_type(ContractType::Api);
        assert_eq!(apis.len(), 1);
        let events = validator.find_by_type(ContractType::Event);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_contract_type_serialization() {
        let ct = ContractType::Api;
        let json = serde_json::to_string(&ct).unwrap();
        let deserialized: ContractType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ct);
    }
}
