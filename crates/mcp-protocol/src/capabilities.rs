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

// ── Client Capabilities ──────────────────────────────────────────────────────

/// Capabilities advertised by an MCP client during initialization.
/// Sent in the `initialize` request `params` field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Protocol versions the client supports (newest first).
    /// Server picks the latest mutually-supported version.
    #[serde(default)]
    pub protocol_version: Vec<String>,

    /// Client supports tools.
    #[serde(default)]
    pub tools: bool,

    /// Client supports resources.
    #[serde(default)]
    pub resources: bool,

    /// Client supports prompts.
    #[serde(default)]
    pub prompts: bool,
}

impl ClientCapabilities {
    /// Create with a list of supported protocol versions.
    pub fn with_versions(versions: Vec<String>) -> Self {
        Self {
            protocol_version: versions,
            ..Default::default()
        }
    }

    /// Enable tools.
    pub fn with_tools(mut self) -> Self {
        self.tools = true;
        self
    }

    /// Enable resources.
    pub fn with_resources(mut self) -> Self {
        self.resources = true;
        self
    }

    /// Enable prompts.
    pub fn with_prompts(mut self) -> Self {
        self.prompts = true;
        self
    }
}

/// Negotiation result between client and server protocol versions.
#[derive(Debug, Clone)]
pub struct VersionNegotiation {
    /// The agreed-upon protocol version.
    pub agreed_version: String,
    /// Whether the client needs to send an `initialized` notification.
    pub needs_initialized_notification: bool,
}

impl ClientCapabilities {
    /// Negotiate a protocol version with the server's supported versions.
    /// Returns the highest mutually-supported version, or the latest server version
    /// as a fallback (MCP spec: server picks best match, not hard fail).
    pub fn negotiate(&self, server_versions: &[String]) -> VersionNegotiation {
        // Client preference order is its own list (newest first).
        for client_ver in &self.protocol_version {
            if server_versions.contains(client_ver) {
                return VersionNegotiation {
                    agreed_version: client_ver.clone(),
                    needs_initialized_notification: true,
                };
            }
        }
        // Fallback: use server's latest version (spec-compliant fallback)
        VersionNegotiation {
            agreed_version: server_versions
                .first()
                .cloned()
                .unwrap_or_else(|| "2024-11-05".to_string()),
            needs_initialized_notification: true,
        }
    }
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
