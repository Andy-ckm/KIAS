use serde::{Deserialize, Serialize};

/// Represents an MCP resource definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// URI that uniquely identifies this resource.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description of the resource.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional MIME type of the resource content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl Resource {
    /// Create a new resource.
    pub fn new(
        uri: impl Into<String>,
        name: impl Into<String>,
        description: Option<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description,
            mime_type: None,
        }
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_creation() {
        let res = Resource::new(
            "file:///tmp/test.txt",
            "test.txt",
            Some("A test file".to_string()),
        );
        assert_eq!(res.uri, "file:///tmp/test.txt");
        assert_eq!(res.name, "test.txt");
        assert_eq!(res.description.as_deref(), Some("A test file"));
        assert!(res.mime_type.is_none());
    }

    #[test]
    fn test_resource_with_mime_type() {
        let res = Resource::new("file:///tmp/data.json", "data.json", None)
            .with_mime_type("application/json");
        assert_eq!(res.mime_type.as_deref(), Some("application/json"));
        assert!(res.description.is_none());
    }

    #[test]
    fn test_resource_serialization() {
        let res = Resource::new("db://users/1", "user_1", Some("User record".to_string()));
        let json_str = serde_json::to_string(&res).unwrap();
        let deserialized: Resource = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.uri, "db://users/1");
        assert_eq!(deserialized.name, "user_1");
    }
}
