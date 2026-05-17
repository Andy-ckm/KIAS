//! # Agent Soul (SOUL.md) Identity Layer
//!
//! Inspired by Hermes Agent's SOUL.md mechanism.
//! Each agent can have a SOUL.md file that defines its identity:
//! personality, knowledge domains, capabilities, constraints.
//!
//! ## File Format
//!
//! ```yaml
//! ---
//! name: researcher
//! role: intelligence
//! personality:
//!   style: analytical
//!   thoroughness: high
//! knowledge_domains: [ai, ml, nlp]
//! capabilities: [pattern_recognition, literature_review]
//! constraints: [always_cite_sources]
//! ---
//! # Research Agent
//! Detailed description here...
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Personality traits
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Personality {
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub thoroughness: String,
    #[serde(default)]
    pub communication: String,
}

/// Soul configuration parsed from SOUL.md YAML frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    /// Agent name
    #[serde(default)]
    pub name: String,
    /// Agent role/class
    #[serde(default)]
    pub role: String,
    /// Schema version
    #[serde(default = "default_version")]
    pub version: String,
    /// Personality traits
    #[serde(default)]
    pub personality: Personality,
    /// Knowledge domains (e.g., ai, finance, logistics)
    #[serde(default)]
    pub knowledge_domains: Vec<String>,
    /// Capabilities (e.g., pattern_recognition, code_review)
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Behavioral constraints (e.g., always_cite_sources)
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Allowed tools
    #[serde(default)]
    pub tools: Vec<String>,
    /// Markdown body (description)
    #[serde(default)]
    pub description: String,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl Default for SoulConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            role: String::new(),
            version: default_version(),
            personality: Personality::default(),
            knowledge_domains: Vec::new(),
            capabilities: Vec::new(),
            constraints: Vec::new(),
            tools: Vec::new(),
            description: String::new(),
        }
    }
}

impl SoulConfig {
    /// Parse SOUL.md content into SoulConfig
    pub fn from_markdown(content: &str) -> Result<Self, String> {
        let content = content.trim();

        // Find YAML frontmatter between --- delimiters
        if !content.starts_with("---") {
            return Err("Missing YAML frontmatter delimiter (---)".to_string());
        }

        let after_first = &content[3..];
        let end_idx = after_first
            .find("---")
            .ok_or("Missing closing YAML frontmatter delimiter (---)")?;

        let yaml_str = &after_first[..end_idx];
        let markdown_body = after_first[end_idx + 3..].trim();

        let mut config: SoulConfig =
            serde_yaml::from_str(yaml_str).map_err(|e| format!("YAML parse error: {}", e))?;
        config.description = markdown_body.to_string();

        Ok(config)
    }

    /// Build system prompt string from soul config
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = String::new();

        // Identity header
        if !self.name.is_empty() {
            prompt.push_str(&format!("# Agent: {}\n\n", self.name));
        }
        if !self.role.is_empty() {
            prompt.push_str(&format!("Role: {}\n", self.role));
        }

        // Personality
        if !self.personality.style.is_empty() {
            prompt.push_str(&format!("Personality style: {}\n", self.personality.style));
        }
        if !self.personality.thoroughness.is_empty() {
            prompt.push_str(&format!(
                "Thoroughness: {}\n",
                self.personality.thoroughness
            ));
        }
        if !self.personality.communication.is_empty() {
            prompt.push_str(&format!(
                "Communication style: {}\n",
                self.personality.communication
            ));
        }

        // Knowledge domains
        if !self.knowledge_domains.is_empty() {
            prompt.push_str(&format!(
                "\n## Knowledge Domains\n{}\n",
                self.knowledge_domains.join(", ")
            ));
        }

        // Capabilities
        if !self.capabilities.is_empty() {
            prompt.push_str(&format!(
                "\n## Capabilities\n{}\n",
                self.capabilities
                    .iter()
                    .map(|c| format!("- {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        // Constraints
        if !self.constraints.is_empty() {
            prompt.push_str(&format!(
                "\n## Constraints\n{}\n",
                self.constraints
                    .iter()
                    .map(|c| format!("- {}", c))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        // Tools
        if !self.tools.is_empty() {
            prompt.push_str(&format!("\n## Allowed Tools\n{}\n", self.tools.join(", ")));
        }

        // Description body
        if !self.description.is_empty() {
            prompt.push_str(&format!("\n## Description\n{}\n", self.description));
        }

        prompt
    }
}

/// Soul loader with caching and file change detection
pub struct SoulLoader {
    souls_dir: PathBuf,
    cache: HashMap<String, (SoulConfig, std::time::SystemTime)>,
}

impl SoulLoader {
    /// Create a new SoulLoader pointing to a directory of SOUL.md files
    pub fn new(souls_dir: PathBuf) -> Self {
        Self {
            souls_dir,
            cache: HashMap::new(),
        }
    }

    /// Get the path to an agent's SOUL.md file
    fn soul_path(&self, agent_id: &str) -> PathBuf {
        self.souls_dir.join(format!("{}.soul.md", agent_id))
    }

    /// Load soul from file (with caching)
    pub fn load(&mut self, agent_id: &str) -> Result<&SoulConfig, String> {
        let path = self.soul_path(agent_id);
        let mtime = Self::get_mtime(&path)?;

        // Check cache — if hit and fresh, return early
        let cache_hit = self
            .cache
            .get(agent_id)
            .map(|(_, cached_mtime)| *cached_mtime == mtime)
            .unwrap_or(false);

        if !cache_hit {
            // Load and parse
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let config = SoulConfig::from_markdown(&content)?;
            self.cache.insert(agent_id.to_string(), (config, mtime));
        }

        Ok(&self.cache.get(agent_id).unwrap().0)
    }

    /// Check if file changed since last load
    pub fn has_changed(&self, agent_id: &str) -> bool {
        let path = self.soul_path(agent_id);
        let Ok(mtime) = Self::get_mtime(&path) else {
            return false;
        };
        match self.cache.get(agent_id) {
            Some((_, cached_mtime)) => *cached_mtime != mtime,
            None => true,
        }
    }

    /// Reload if changed, return true if reloaded
    pub fn reload_if_changed(&mut self, agent_id: &str) -> Result<bool, String> {
        if self.has_changed(agent_id) {
            self.load(agent_id)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove from cache
    pub fn invalidate(&mut self, agent_id: &str) {
        self.cache.remove(agent_id);
    }

    /// Get cached soul without file I/O
    pub fn get_cached(&self, agent_id: &str) -> Option<&SoulConfig> {
        self.cache.get(agent_id).map(|(config, _)| config)
    }

    /// List all agent IDs that have SOUL.md files
    pub fn list_agents(&self) -> Vec<String> {
        let mut agents = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.souls_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".soul.md") {
                    let agent_id = name.trim_end_matches(".soul.md").to_string();
                    agents.push(agent_id);
                }
            }
        }
        agents
    }

    fn get_mtime(path: &Path) -> Result<std::time::SystemTime, String> {
        std::fs::metadata(path)
            .map(|m| m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH))
            .map_err(|e| format!("Failed to get mtime for {}: {}", path.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample_soul_md() -> &'static str {
        r#"---
name: researcher
role: intelligence
version: "1.0"
personality:
  style: analytical
  thoroughness: high
  communication: formal
knowledge_domains:
  - ai
  - ml
  - nlp
capabilities:
  - pattern_recognition
  - literature_review
constraints:
  - always_cite_sources
  - never_speculate_without_evidence
tools:
  - web_search
  - arxiv
---

# Research Agent

This agent specializes in AI research and literature review.
It always cites sources and prefers peer-reviewed papers.
"#
    }

    #[test]
    fn test_soul_config_parse() {
        let config = SoulConfig::from_markdown(sample_soul_md()).unwrap();
        assert_eq!(config.name, "researcher");
        assert_eq!(config.role, "intelligence");
        assert_eq!(config.version, "1.0");
        assert_eq!(config.personality.style, "analytical");
        assert_eq!(config.personality.thoroughness, "high");
        assert_eq!(config.knowledge_domains, vec!["ai", "ml", "nlp"]);
        assert_eq!(config.capabilities.len(), 2);
        assert_eq!(config.constraints.len(), 2);
        assert_eq!(config.tools, vec!["web_search", "arxiv"]);
        assert!(config.description.contains("Research Agent"));
    }

    #[test]
    fn test_soul_config_minimal() {
        let md = "---\nname: simple\n---\n# Simple Agent\n";
        let config = SoulConfig::from_markdown(md).unwrap();
        assert_eq!(config.name, "simple");
        assert_eq!(config.version, "1.0"); // default
        assert!(config.knowledge_domains.is_empty());
    }

    #[test]
    fn test_soul_config_missing_frontmatter() {
        let md = "# No frontmatter\nJust markdown";
        let result = SoulConfig::from_markdown(md);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing YAML frontmatter"));
    }

    #[test]
    fn test_soul_config_missing_closing_delimiter() {
        let md = "---\nname: test\n# no closing delimiter";
        let result = SoulConfig::from_markdown(md);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing closing"));
    }

    #[test]
    fn test_build_system_prompt() {
        let config = SoulConfig::from_markdown(sample_soul_md()).unwrap();
        let prompt = config.build_system_prompt();

        assert!(prompt.contains("# Agent: researcher"));
        assert!(prompt.contains("Role: intelligence"));
        assert!(prompt.contains("Personality style: analytical"));
        assert!(prompt.contains("Knowledge Domains"));
        assert!(prompt.contains("ai, ml, nlp"));
        assert!(prompt.contains("Capabilities"));
        assert!(prompt.contains("- pattern_recognition"));
        assert!(prompt.contains("Constraints"));
        assert!(prompt.contains("- always_cite_sources"));
        assert!(prompt.contains("Allowed Tools"));
        assert!(prompt.contains("web_search, arxiv"));
        assert!(prompt.contains("Research Agent"));
    }

    #[test]
    fn test_soul_loader_load_and_cache() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("test-agent.soul.md");
        let mut f = std::fs::File::create(&soul_path).unwrap();
        f.write_all(sample_soul_md().as_bytes()).unwrap();

        let mut loader = SoulLoader::new(dir.path().to_path_buf());
        let config = loader.load("test-agent").unwrap();
        assert_eq!(config.name, "researcher");

        // Second load should come from cache
        let config2 = loader.load("test-agent").unwrap();
        assert_eq!(config2.name, "researcher");
    }

    #[test]
    fn test_soul_loader_list_agents() {
        let dir = tempfile::tempdir().unwrap();
        for name in &["agent-a", "agent-b"] {
            let path = dir.path().join(format!("{}.soul.md", name));
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(sample_soul_md().as_bytes()).unwrap();
        }
        // Non-soul file should be ignored
        let other = dir.path().join("readme.md");
        std::fs::File::create(&other).unwrap();

        let loader = SoulLoader::new(dir.path().to_path_buf());
        let mut agents = loader.list_agents();
        agents.sort();
        assert_eq!(agents, vec!["agent-a", "agent-b"]);
    }

    #[test]
    fn test_soul_loader_invalidate() {
        let dir = tempfile::tempdir().unwrap();
        let soul_path = dir.path().join("test.soul.md");
        let mut f = std::fs::File::create(&soul_path).unwrap();
        f.write_all(sample_soul_md().as_bytes()).unwrap();

        let mut loader = SoulLoader::new(dir.path().to_path_buf());
        loader.load("test").unwrap();
        assert!(loader.get_cached("test").is_some());

        loader.invalidate("test");
        assert!(loader.get_cached("test").is_none());
    }
}
