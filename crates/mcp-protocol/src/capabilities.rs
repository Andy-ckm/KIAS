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
