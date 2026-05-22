//! SDK 协议统一
//!
//! 提供统一的 CLI/API/SDK 接口定义与版本管理。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SDK 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SdkType {
    Cli,
    Api,
    Sdk,
}

/// 协议版本
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl ProtocolVersion {
    pub fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// 解析版本字符串
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some(Self::new(major, minor, patch))
    }

    /// 是否与另一个版本兼容
    pub fn is_compatible_with(&self, other: &ProtocolVersion) -> bool {
        self.major == other.major
    }

    /// 检查向后兼容
    pub fn is_backward_compatible_with(&self, older: &ProtocolVersion) -> bool {
        self.major == older.major && self.minor >= older.minor
    }
}

/// SDK 方法定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkMethod {
    pub name: String,
    pub description: String,
    pub parameters: Vec<MethodParameter>,
    pub return_type: String,
}

/// 方法参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodParameter {
    pub name: String,
    pub param_type: String,
    pub required: bool,
    pub description: String,
}

/// SDK 协议定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkProtocol {
    pub sdk_type: SdkType,
    pub version: ProtocolVersion,
    pub methods: Vec<SdkMethod>,
    pub endpoints: Vec<ApiEndpoint>,
    pub auth_required: bool,
}

/// API 端点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoint {
    pub path: String,
    pub method: String,
    pub description: String,
    pub auth_required: bool,
}

impl SdkProtocol {
    pub fn new_cli(version: ProtocolVersion) -> Self {
        Self {
            sdk_type: SdkType::Cli,
            version,
            methods: Vec::new(),
            endpoints: Vec::new(),
            auth_required: true,
        }
    }

    pub fn new_api(version: ProtocolVersion) -> Self {
        Self {
            sdk_type: SdkType::Api,
            version,
            methods: Vec::new(),
            endpoints: Vec::new(),
            auth_required: true,
        }
    }

    pub fn new_sdk(version: ProtocolVersion) -> Self {
        Self {
            sdk_type: SdkType::Sdk,
            version,
            methods: Vec::new(),
            endpoints: Vec::new(),
            auth_required: true,
        }
    }

    pub fn add_method(&mut self, method: SdkMethod) {
        self.methods.push(method);
    }

    pub fn add_endpoint(&mut self, endpoint: ApiEndpoint) {
        self.endpoints.push(endpoint);
    }
}

/// 兼容性检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityCheck {
    pub client_version: ProtocolVersion,
    pub server_version: ProtocolVersion,
    pub is_compatible: bool,
    pub warnings: Vec<String>,
    pub breaking_changes: Vec<String>,
}

/// SDK 协议管理器
pub struct SdkProtocolManager {
    protocols: HashMap<SdkType, SdkProtocol>,
}

impl Default for SdkProtocolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SdkProtocolManager {
    pub fn new() -> Self {
        Self {
            protocols: HashMap::new(),
        }
    }

    /// 注册协议
    pub fn register(&mut self, protocol: SdkProtocol) {
        self.protocols.insert(protocol.sdk_type, protocol);
    }

    /// 获取协议
    pub fn get(&self, sdk_type: SdkType) -> Option<&SdkProtocol> {
        self.protocols.get(&sdk_type)
    }

    /// 检查兼容性
    pub fn check_compatibility(
        &self,
        client_version: ProtocolVersion,
        server_version: ProtocolVersion,
    ) -> CompatibilityCheck {
        let mut warnings = Vec::new();
        let mut breaking_changes = Vec::new();
        let is_compatible = client_version.is_backward_compatible_with(&server_version);

        if client_version.major != server_version.major {
            breaking_changes.push("Major version mismatch - incompatible protocols".to_string());
        }
        if client_version.minor < server_version.minor {
            warnings.push(
                "Client version is older than server - some features may be unavailable"
                    .to_string(),
            );
        }
        if client_version.patch < server_version.patch {
            warnings.push("Client version is behind server - consider upgrading".to_string());
        }

        CompatibilityCheck {
            client_version,
            server_version,
            is_compatible,
            warnings,
            breaking_changes,
        }
    }
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_display() {
        let v = ProtocolVersion::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_protocol_version_parse() {
        let v = ProtocolVersion::parse("2.0.1").unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 1);
    }

    #[test]
    fn test_protocol_version_parse_invalid() {
        assert!(ProtocolVersion::parse("invalid").is_none());
        assert!(ProtocolVersion::parse("1.2").is_none());
    }

    #[test]
    fn test_protocol_version_compatibility() {
        let v1 = ProtocolVersion::new(1, 0, 0);
        let v2 = ProtocolVersion::new(1, 0, 0);
        assert!(v1.is_compatible_with(&v2));

        let v3 = ProtocolVersion::new(2, 0, 0);
        assert!(!v1.is_compatible_with(&v3));
    }

    #[test]
    fn test_protocol_version_backward_compatibility() {
        let newer = ProtocolVersion::new(1, 2, 0);
        let older = ProtocolVersion::new(1, 1, 0);
        assert!(newer.is_backward_compatible_with(&older));
    }

    #[test]
    fn test_sdk_protocol_manager_register_and_get() {
        let mut manager = SdkProtocolManager::new();
        let protocol = SdkProtocol::new_cli(ProtocolVersion::new(1, 0, 0));
        manager.register(protocol);
        assert!(manager.get(SdkType::Cli).is_some());
    }

    #[test]
    fn test_sdk_protocol_manager_compatibility_check() {
        let mut manager = SdkProtocolManager::new();
        let protocol = SdkProtocol::new_api(ProtocolVersion::new(2, 0, 0));
        manager.register(protocol);

        let check = manager
            .check_compatibility(ProtocolVersion::new(2, 0, 0), ProtocolVersion::new(2, 0, 0));
        assert!(check.is_compatible);
        assert!(check.breaking_changes.is_empty());
    }

    #[test]
    fn test_sdk_protocol_manager_compatibility_check_major_mismatch() {
        let manager = SdkProtocolManager::new();
        let check = manager
            .check_compatibility(ProtocolVersion::new(1, 0, 0), ProtocolVersion::new(2, 0, 0));
        assert!(!check.is_compatible);
        assert!(!check.breaking_changes.is_empty());
    }
}
