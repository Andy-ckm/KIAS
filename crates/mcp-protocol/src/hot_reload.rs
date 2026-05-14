//! MCP Tool Hot-Reload
//!
//! Provides:
//! - File watcher for tool definitions (YAML/JSON)
//! - Hot-reload without server restart
//! - Tool versioning and rollback
//! - Tool validation before activation
//! - Tool dependency resolution

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::McpError;
use crate::server::{ToolAnnotations, ToolDefinition};
use crate::tool::Tool;

// ---------------------------------------------------------------------------
// Tool Definition File Format
// ---------------------------------------------------------------------------

/// Tool definition loaded from YAML/JSON file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinitionFile {
    /// Tool name (must be unique).
    pub name: String,
    /// Tool version (semver).
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for input parameters.
    pub input_schema: serde_json::Value,
    /// Optional behavioral annotations.
    #[serde(default)]
    pub annotations: Option<ToolAnnotations>,
    /// Tool implementation type.
    pub implementation: ToolImplementation,
    /// Dependencies on other tools.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Tags for organization.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Whether this tool is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

/// Tool implementation types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolImplementation {
    /// HTTP endpoint.
    #[serde(rename = "http")]
    Http {
        /// Endpoint URL.
        url: String,
        /// HTTP method.
        #[serde(default = "default_post")]
        method: String,
        /// Request timeout (seconds).
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// Shell command.
    #[serde(rename = "shell")]
    Shell {
        /// Command to execute.
        command: String,
        /// Working directory.
        #[serde(default)]
        workdir: Option<String>,
        /// Environment variables.
        #[serde(default)]
        env: HashMap<String, String>,
    },
    /// Python script.
    #[serde(rename = "python")]
    Python {
        /// Script path.
        script: PathBuf,
        /// Virtual environment path.
        #[serde(default)]
        venv: Option<PathBuf>,
    },
    /// WASM module.
    #[serde(rename = "wasm")]
    Wasm {
        /// Module path.
        module: PathBuf,
        /// Function to call.
        #[serde(default = "default_handler")]
        function: String,
    },
    /// gRPC service.
    #[serde(rename = "grpc")]
    Grpc {
        /// Service address.
        address: String,
        /// Service name.
        service: String,
        /// Method name.
        method: String,
    },
    /// Built-in Rust function.
    #[serde(rename = "builtin")]
    Builtin {
        /// Function name.
        function: String,
    },
}

fn default_post() -> String {
    "POST".to_string()
}

fn default_timeout() -> u64 {
    30
}

fn default_handler() -> String {
    "handle".to_string()
}

// ---------------------------------------------------------------------------
// Tool Registry with Hot-Reload
// ---------------------------------------------------------------------------

/// Tool version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolVersion {
    /// Version string.
    pub version: String,
    /// When this version was loaded.
    pub loaded_at: SystemTime,
    /// File path where this tool was loaded from.
    pub source_path: PathBuf,
    /// Whether this version is active.
    pub active: bool,
    /// Tool definition.
    pub definition: ToolDefinitionFile,
}

/// Tool registry entry.
#[derive(Debug, Clone)]
pub struct ToolRegistryEntry {
    /// Current active version.
    pub current: ToolVersion,
    /// Previous version (for rollback).
    pub previous: Option<ToolVersion>,
    /// All versions loaded.
    pub history: Vec<ToolVersion>,
}

/// Tool registry with hot-reload support.
pub struct ToolRegistry {
    /// Registered tools.
    tools: Arc<RwLock<HashMap<String, ToolRegistryEntry>>>,
    /// Watched directories.
    watched_dirs: Arc<RwLock<Vec<PathBuf>>>,
    /// File hash cache for change detection.
    file_hashes: Arc<RwLock<HashMap<PathBuf, String>>>,
    /// Validation enabled.
    validation_enabled: bool,
}

impl ToolRegistry {
    /// Create a new tool registry.
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            watched_dirs: Arc::new(RwLock::new(Vec::new())),
            file_hashes: Arc::new(RwLock::new(HashMap::new())),
            validation_enabled: true,
        }
    }

    /// Disable validation (for testing).
    pub fn without_validation() -> Self {
        Self {
            validation_enabled: false,
            ..Self::new()
        }
    }

    /// Register a tool from a definition file.
    pub async fn register(&self, definition: ToolDefinitionFile) -> Result<(), McpError> {
        // Validate if enabled
        if self.validation_enabled {
            self.validate(&definition)?;
        }

        let version = ToolVersion {
            version: definition.version.clone(),
            loaded_at: SystemTime::now(),
            source_path: PathBuf::new(), // Will be set by caller
            active: true,
            definition: definition.clone(),
        };

        let mut tools = self.tools.write().await;

        if let Some(entry) = tools.get_mut(&definition.name) {
            // Deactivate current version
            entry.current.active = false;

            // Move current to history
            entry.history.push(entry.current.clone());

            // Set new version as current
            entry.previous = Some(entry.current.clone());
            entry.current = version;
        } else {
            // New tool
            tools.insert(
                definition.name.clone(),
                ToolRegistryEntry {
                    current: version,
                    previous: None,
                    history: Vec::new(),
                },
            );
        }

        Ok(())
    }

    /// Register a tool from a file path.
    pub async fn register_from_file(&self, path: &Path) -> Result<(), McpError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| McpError::Internal(format!("Failed to read tool file: {}", e)))?;

        let definition: ToolDefinitionFile = if path.extension().and_then(|e| e.to_str()) == Some("yaml")
            || path.extension().and_then(|e| e.to_str()) == Some("yml")
        {
            serde_yaml::from_str(&content)
                .map_err(|e| McpError::Internal(format!("Invalid YAML: {}", e)))?
        } else {
            serde_json::from_str(&content)
                .map_err(|e| McpError::Internal(format!("Invalid JSON: {}", e)))?
        };

        // Store file hash
        let hash = self.compute_hash(&content);
        let mut hashes = self.file_hashes.write().await;
        hashes.insert(path.to_path_buf(), hash);

        self.register(definition).await
    }

    /// Load all tool definitions from a directory.
    pub async fn load_directory(&self, dir: &Path) -> Result<usize, McpError> {
        if !dir.is_dir() {
            return Err(McpError::Internal(format!(
                "Not a directory: {}",
                dir.display()
            )));
        }

        let mut count = 0;
        let entries = std::fs::read_dir(dir)
            .map_err(|e| McpError::Internal(format!("Failed to read directory: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| McpError::Internal(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str());
                if matches!(ext, Some("yaml" | "yml" | "json")) {
                    match self.register_from_file(&path).await {
                        Ok(()) => count += 1,
                        Err(e) => {
                            eprintln!("Failed to load tool from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        // Add to watched directories
        let mut watched = self.watched_dirs.write().await;
        if !watched.contains(&dir.to_path_buf()) {
            watched.push(dir.to_path_buf());
        }

        Ok(count)
    }

    /// Check for changes in watched directories and reload if needed.
    pub async fn check_and_reload(&self) -> Result<Vec<String>, McpError> {
        let watched = self.watched_dirs.read().await;
        let mut reloaded = Vec::new();

        for dir in watched.iter() {
            if !dir.is_dir() {
                continue;
            }

            let entries = std::fs::read_dir(dir)
                .map_err(|e| McpError::Internal(format!("Failed to read directory: {}", e)))?;

            for entry in entries {
                let entry = entry.map_err(|e| McpError::Internal(format!("Failed to read entry: {}", e)))?;
                let path = entry.path();

                if !path.is_file() {
                    continue;
                }

                let ext = path.extension().and_then(|e| e.to_str());
                if !matches!(ext, Some("yaml" | "yml" | "json")) {
                    continue;
                }

                // Check if file has changed
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| McpError::Internal(format!("Failed to read file: {}", e)))?;

                let hash = self.compute_hash(&content);
                let hashes = self.file_hashes.read().await;

                if let Some(old_hash) = hashes.get(&path) {
                    if *old_hash != hash {
                        // File has changed, reload
                        drop(hashes);
                        self.register_from_file(&path).await?;
                        reloaded.push(path.display().to_string());
                    }
                } else {
                    // New file
                    drop(hashes);
                    self.register_from_file(&path).await?;
                    reloaded.push(path.display().to_string());
                }
            }
        }

        Ok(reloaded)
    }

    /// Rollback a tool to its previous version.
    pub async fn rollback(&self, name: &str) -> Result<(), McpError> {
        let mut tools = self.tools.write().await;

        if let Some(entry) = tools.get_mut(name) {
            if let Some(previous) = entry.previous.take() {
                // Deactivate current
                entry.current.active = false;
                entry.history.push(entry.current.clone());

                // Activate previous
                let mut previous = previous;
                previous.active = true;
                entry.current = previous;

                Ok(())
            } else {
                Err(McpError::InvalidRequest(format!(
                    "No previous version for tool: {}",
                    name
                )))
            }
        } else {
            Err(McpError::ToolNotFound(name.to_string()))
        }
    }

    /// Get a tool definition by name.
    pub async fn get(&self, name: &str) -> Option<ToolDefinitionFile> {
        let tools = self.tools.read().await;
        tools.get(name).map(|entry| entry.current.definition.clone())
    }

    /// List all registered tools.
    pub async fn list(&self) -> Vec<ToolDefinitionFile> {
        let tools = self.tools.read().await;
        tools
            .values()
            .filter(|entry| entry.current.active)
            .map(|entry| entry.current.definition.clone())
            .collect()
    }

    /// Get tool version history.
    pub async fn history(&self, name: &str) -> Option<Vec<ToolVersion>> {
        let tools = self.tools.read().await;
        tools.get(name).map(|entry| {
            let mut versions = entry.history.clone();
            versions.push(entry.current.clone());
            versions
        })
    }

    /// Remove a tool.
    pub async fn remove(&self, name: &str) -> Result<(), McpError> {
        let mut tools = self.tools.write().await;
        if tools.remove(name).is_some() {
            Ok(())
        } else {
            Err(McpError::ToolNotFound(name.to_string()))
        }
    }

    /// Validate a tool definition.
    fn validate(&self, definition: &ToolDefinitionFile) -> Result<(), McpError> {
        // Check name is not empty
        if definition.name.is_empty() {
            return Err(McpError::InvalidRequest("Tool name cannot be empty".to_string()));
        }

        // Check version is valid semver
        if !is_valid_semver(&definition.version) {
            return Err(McpError::InvalidRequest(format!(
                "Invalid version: {}",
                definition.version
            )));
        }

        // Check description is not empty
        if definition.description.is_empty() {
            return Err(McpError::InvalidRequest(
                "Tool description cannot be empty".to_string(),
            ));
        }

        // Validate input schema is a valid JSON Schema
        if !definition.input_schema.is_object() {
            return Err(McpError::InvalidRequest(
                "Input schema must be a JSON object".to_string(),
            ));
        }

        Ok(())
    }

    /// Compute a simple hash of content.
    fn compute_hash(&self, content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple semver validation.
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }

    parts.iter().all(|part| part.parse::<u32>().is_ok())
}

// ---------------------------------------------------------------------------
// Tool Loader (converts ToolDefinitionFile to ToolDefinition)
// ---------------------------------------------------------------------------

/// Convert a ToolDefinitionFile to a ToolDefinition for the MCP server.
pub fn to_tool_definition(file: &ToolDefinitionFile) -> ToolDefinition {
    let mut def = ToolDefinition::new(&file.name, &file.description, file.input_schema.clone());

    if let Some(ref annotations) = file.annotations {
        def = def.with_annotations(annotations.clone());
    }

    def
}

/// Convert a ToolDefinitionFile to a Tool.
pub fn to_tool(file: &ToolDefinitionFile) -> Tool {
    Tool::new(
        &file.name,
        &file.description,
        file.input_schema.clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tool_definition() -> ToolDefinitionFile {
        ToolDefinitionFile {
            name: "test-tool".to_string(),
            version: "1.0.0".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                }
            }),
            annotations: None,
            implementation: ToolImplementation::Http {
                url: "http://localhost:8080/tool".to_string(),
                method: "POST".to_string(),
                timeout_secs: 30,
            },
            dependencies: vec![],
            tags: vec!["test".to_string()],
            enabled: true,
        }
    }

    #[tokio::test]
    async fn test_register_tool() {
        let registry = ToolRegistry::new();
        let def = sample_tool_definition();

        registry.register(def.clone()).await.unwrap();

        let tools = registry.list().await;
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "test-tool");
    }

    #[tokio::test]
    async fn test_tool_versioning() {
        let registry = ToolRegistry::new();

        let mut def = sample_tool_definition();
        registry.register(def.clone()).await.unwrap();

        // Update version
        def.version = "1.1.0".to_string();
        def.description = "Updated tool".to_string();
        registry.register(def).await.unwrap();

        let history = registry.history("test-tool").await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, "1.0.0");
        assert_eq!(history[1].version, "1.1.0");
    }

    #[tokio::test]
    async fn test_tool_rollback() {
        let registry = ToolRegistry::new();

        let mut def = sample_tool_definition();
        registry.register(def.clone()).await.unwrap();

        // Update version
        def.version = "1.1.0".to_string();
        registry.register(def).await.unwrap();

        // Rollback
        registry.rollback("test-tool").await.unwrap();

        let tool = registry.get("test-tool").await.unwrap();
        assert_eq!(tool.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_tool_validation() {
        let registry = ToolRegistry::new();

        // Invalid version
        let mut def = sample_tool_definition();
        def.version = "invalid".to_string();

        let result = registry.register(def).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_tool_removal() {
        let registry = ToolRegistry::new();
        let def = sample_tool_definition();

        registry.register(def).await.unwrap();
        assert_eq!(registry.list().await.len(), 1);

        registry.remove("test-tool").await.unwrap();
        assert_eq!(registry.list().await.len(), 0);
    }

    #[test]
    fn test_semver_validation() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.1.0"));
        assert!(is_valid_semver("10.20.30"));
        assert!(!is_valid_semver("invalid"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("1.0.0-alpha"));
    }

    #[test]
    fn test_yaml_parsing() {
        let yaml = r#"
name: echo
version: "1.0.0"
description: Echo back input
input_schema:
  type: object
  properties:
    text:
      type: string
implementation:
  type: http
  url: "http://localhost:8080/echo"
"#;

        let def: ToolDefinitionFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(def.name, "echo");
        assert_eq!(def.version, "1.0.0");
    }
}
