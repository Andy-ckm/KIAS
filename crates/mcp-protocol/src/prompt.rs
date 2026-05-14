use serde::{Deserialize, Serialize};

/// Represents an MCP prompt definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique prompt name.
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Arguments this prompt accepts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<PromptArgument>,
}

/// A single argument accepted by an MCP prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,
}

impl Prompt {
    /// Create a new prompt.
    pub fn new(name: impl Into<String>, description: Option<String>) -> Self {
        Self {
            name: name.into(),
            description,
            arguments: Vec::new(),
        }
    }

    /// Add an argument to this prompt.
    pub fn with_argument(mut self, arg: PromptArgument) -> Self {
        self.arguments.push(arg);
        self
    }
}

impl PromptArgument {
    /// Create a new prompt argument.
    pub fn new(name: impl Into<String>, description: Option<String>, required: bool) -> Self {
        Self {
            name: name.into(),
            description,
            required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_creation() {
        let prompt = Prompt::new("code_review", Some("Review code for issues".to_string()));
        assert_eq!(prompt.name, "code_review");
        assert_eq!(
            prompt.description.as_deref(),
            Some("Review code for issues")
        );
        assert!(prompt.arguments.is_empty());
    }

    #[test]
    fn test_prompt_with_arguments() {
        let prompt = Prompt::new("summarize", None).with_argument(PromptArgument::new(
            "text",
            Some("Text to summarize".to_string()),
            true,
        ));
        assert_eq!(prompt.arguments.len(), 1);
        assert_eq!(prompt.arguments[0].name, "text");
        assert!(prompt.arguments[0].required);
    }

    #[test]
    fn test_prompt_serialization() {
        let prompt = Prompt::new("translate", Some("Translate text".to_string()))
            .with_argument(PromptArgument::new(
                "source_lang",
                Some("Source language".to_string()),
                true,
            ))
            .with_argument(PromptArgument::new(
                "target_lang",
                Some("Target language".to_string()),
                true,
            ));
        let json_str = serde_json::to_string(&prompt).unwrap();
        let deserialized: Prompt = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "translate");
        assert_eq!(deserialized.arguments.len(), 2);
    }
}
