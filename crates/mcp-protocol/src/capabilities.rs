use serde::{Deserialize, Serialize};

/// Capabilities advertised by an MCP server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Server supports tool listing and invocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,

    /// Server supports resource listing and reading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,

    /// Server supports prompt listing and retrieval.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

/// Tools capability marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    /// Whether the server supports list_changed notifications.
    #[serde(default)]
    pub list_changed: bool,
}

/// Resources capability marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    /// Whether the server supports subscribe on resources.
    #[serde(default)]
    pub subscribe: bool,
    /// Whether the server supports list_changed notifications.
    #[serde(default)]
    pub list_changed: bool,
}

/// Prompts capability marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    /// Whether the server supports list_changed notifications.
    #[serde(default)]
    pub list_changed: bool,
}

impl ServerCapabilities {
    /// Create an empty capabilities set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable tools capability.
    pub fn with_tools(mut self) -> Self {
        self.tools = Some(ToolsCapability {
            list_changed: false,
        });
        self
    }

    /// Enable resources capability.
    pub fn with_resources(mut self) -> Self {
        self.resources = Some(ResourcesCapability {
            subscribe: false,
            list_changed: false,
        });
        self
    }

    /// Enable prompts capability.
    pub fn with_prompts(mut self) -> Self {
        self.prompts = Some(PromptsCapability {
            list_changed: false,
        });
        self
    }

    /// Returns `true` if tools are supported.
    pub fn has_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Returns `true` if resources are supported.
    pub fn has_resources(&self) -> bool {
        self.resources.is_some()
    }

    /// Returns `true` if prompts are supported.
    pub fn has_prompts(&self) -> bool {
        self.prompts.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_capabilities_empty() {
        let caps = ServerCapabilities::new();
        assert!(!caps.has_tools());
        assert!(!caps.has_resources());
        assert!(!caps.has_prompts());
    }

    #[test]
    fn test_with_tools() {
        let caps = ServerCapabilities::new().with_tools();
        assert!(caps.has_tools());
        assert!(!caps.has_resources());
        assert!(!caps.has_prompts());
    }

    #[test]
    fn test_with_resources() {
        let caps = ServerCapabilities::new().with_resources();
        assert!(!caps.has_tools());
        assert!(caps.has_resources());
        assert!(!caps.has_prompts());
    }

    #[test]
    fn test_with_prompts() {
        let caps = ServerCapabilities::new().with_prompts();
        assert!(!caps.has_tools());
        assert!(!caps.has_resources());
        assert!(caps.has_prompts());
    }

    #[test]
    fn test_all_capabilities() {
        let caps = ServerCapabilities::new()
            .with_tools()
            .with_resources()
            .with_prompts();
        assert!(caps.has_tools());
        assert!(caps.has_resources());
        assert!(caps.has_prompts());
    }

    #[test]
    fn test_capabilities_serialize_roundtrip() {
        let caps = ServerCapabilities::new().with_tools().with_resources();
        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: ServerCapabilities = serde_json::from_str(&json).unwrap();
        assert!(deserialized.has_tools());
        assert!(deserialized.has_resources());
        assert!(!deserialized.has_prompts());
    }

    #[test]
    fn test_capabilities_skip_none() {
        let caps = ServerCapabilities::new().with_tools();
        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("tools"));
        assert!(!json.contains("resources"));
        assert!(!json.contains("prompts"));
    }

    #[test]
    fn test_tools_capability_list_changed_default() {
        let caps = ServerCapabilities::new().with_tools();
        assert!(!caps.tools.unwrap().list_changed);
    }

    #[test]
    fn test_resources_capability_defaults() {
        let caps = ServerCapabilities::new().with_resources();
        let res = caps.resources.unwrap();
        assert!(!res.subscribe);
        assert!(!res.list_changed);
    }
}
