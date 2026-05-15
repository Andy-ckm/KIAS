//! # Agent Workspace
//!
//! VFS-backed workspace for agent context, skills, sessions, and knowledge.
//!
//! ## Standard Layout
//!
//! ```text
//! AGENTS.md
//! MEMORY.md
//! knowledge/
//! skills/<name>.json
//! subagents/
//! agents/<id>/context/
//! agents/<id>/sessions/<session_id>.json
//! ```

use kias_common::vfs::{VfsError, VirtualFs};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Skill Definition ──────────────────────────────────────────────────

/// Skill definition stored as JSON in the skills/ directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillDef {
    pub name: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub parameters: Option<serde_json::Value>,
}

// ── Workspace Config ──────────────────────────────────────────────────

/// Configuration for an agent workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    /// Root path within the VFS (e.g. "" or "workspaces/agent-1")
    pub root_path: String,
    /// Agent identifier
    pub agent_id: String,
}

impl WorkspaceConfig {
    pub fn new(root_path: impl Into<String>, agent_id: impl Into<String>) -> Self {
        Self {
            root_path: root_path.into(),
            agent_id: agent_id.into(),
        }
    }
}

// ── Workspace ─────────────────────────────────────────────────────────

/// VFS-backed workspace providing structured access to agent files.
pub struct Workspace {
    config: WorkspaceConfig,
    fs: Arc<dyn VirtualFs>,
}

impl Workspace {
    pub fn new(config: WorkspaceConfig, fs: Arc<dyn VirtualFs>) -> Self {
        Self { config, fs }
    }

    /// Join the root path with a relative path.
    fn path(&self, rel: &str) -> String {
        if self.config.root_path.is_empty() {
            rel.to_string()
        } else if rel.is_empty() {
            self.config.root_path.clone()
        } else {
            format!("{}/{}", self.config.root_path.trim_end_matches('/'), rel)
        }
    }

    // ── Markdown files ────────────────────────────────────────────────

    /// Load the workspace AGENTS.md file.
    pub async fn load_agents_md(&self) -> Result<String, VfsError> {
        let bytes = self.fs.read(&self.path("AGENTS.md")).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// Load the workspace MEMORY.md file.
    pub async fn load_memory_md(&self) -> Result<String, VfsError> {
        let bytes = self.fs.read(&self.path("MEMORY.md")).await?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// Save content to the MEMORY.md file.
    pub async fn save_memory(&self, content: &str) -> Result<(), VfsError> {
        self.fs
            .write(&self.path("MEMORY.md"), content.as_bytes())
            .await
    }

    // ── Skills ────────────────────────────────────────────────────────

    /// List all skill definitions in the skills/ directory.
    pub async fn list_skills(&self) -> Result<Vec<SkillDef>, VfsError> {
        let skills_dir = self.path("skills");
        let entries = self.fs.list_dir(&skills_dir).await.unwrap_or_default();
        let mut skills = Vec::new();
        for entry in entries {
            if entry.is_dir || !entry.name.ends_with(".json") {
                continue;
            }
            let path = format!("{}/{}", skills_dir, entry.name);
            match self.fs.read(&path).await {
                Ok(bytes) => {
                    if let Ok(skill) = serde_json::from_slice::<SkillDef>(&bytes) {
                        skills.push(skill);
                    }
                }
                Err(_) => continue,
            }
        }
        Ok(skills)
    }

    /// Load a single skill definition by name.
    pub async fn load_skill(&self, name: &str) -> Result<SkillDef, VfsError> {
        let path = self.path(&format!("skills/{}.json", name));
        let bytes = self.fs.read(&path).await?;
        serde_json::from_slice(&bytes).map_err(|e| VfsError::Other(e.to_string()))
    }

    // ── Session Snapshots ─────────────────────────────────────────────

    /// Save a session snapshot for the current agent.
    pub async fn save_session_snapshot(
        &self,
        session_id: &str,
        data: &serde_json::Value,
    ) -> Result<(), VfsError> {
        let path = self.path(&format!(
            "agents/{}/sessions/{}.json",
            self.config.agent_id, session_id
        ));
        let bytes = serde_json::to_vec_pretty(data)
            .map_err(|e| VfsError::Other(e.to_string()))?;
        self.fs.write(&path, &bytes).await
    }

    /// Load a session snapshot for the current agent.
    pub async fn load_session_snapshot(
        &self,
        session_id: &str,
    ) -> Result<serde_json::Value, VfsError> {
        let path = self.path(&format!(
            "agents/{}/sessions/{}.json",
            self.config.agent_id, session_id
        ));
        let bytes = self.fs.read(&path).await?;
        serde_json::from_slice(&bytes).map_err(|e| VfsError::Other(e.to_string()))
    }

    // ── Convenience ───────────────────────────────────────────────────

    /// Ensure standard workspace directories exist.
    pub async fn init_standard_dirs(&self) -> Result<(), VfsError> {
        let dirs = [
            "knowledge",
            "skills",
            "subagents",
            &format!("agents/{}/context", self.config.agent_id),
            &format!("agents/{}/sessions", self.config.agent_id),
        ];
        for dir in &dirs {
            self.fs.mkdir(&self.path(dir)).await?;
        }
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::vfs::MemoryFs;

    fn test_workspace() -> Workspace {
        let fs = Arc::new(MemoryFs::new());
        let config = WorkspaceConfig::new("", "agent-1");
        Workspace::new(config, fs)
    }

    #[tokio::test]
    async fn test_save_and_load_memory_md() {
        let ws = test_workspace();
        ws.save_memory("# Memory\n\nKey facts stored here.")
            .await
            .unwrap();
        let content = ws.load_memory_md().await.unwrap();
        assert!(content.contains("Key facts stored here."));
    }

    #[tokio::test]
    async fn test_load_agents_md_not_found() {
        let ws = test_workspace();
        let err = ws.load_agents_md().await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_list_skills_empty() {
        let ws = test_workspace();
        ws.fs.mkdir("skills").await.unwrap();
        let skills = ws.list_skills().await.unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_list_skills_and_load() {
        let ws = test_workspace();
        let skill = SkillDef {
            name: "summarization".into(),
            description: "Text summarization".into(),
            version: "1.0.0".into(),
            tags: vec!["nlp".into()],
            parameters: None,
        };
        let json = serde_json::to_string(&skill).unwrap();
        ws.fs.write("skills/summarization.json", json.as_bytes()).await.unwrap();

        let skills = ws.list_skills().await.unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "summarization");

        let loaded = ws.load_skill("summarization").await.unwrap();
        assert_eq!(loaded, skill);
    }

    #[tokio::test]
    async fn test_load_skill_not_found() {
        let ws = test_workspace();
        let err = ws.load_skill("nonexistent").await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_save_and_load_session_snapshot() {
        let ws = test_workspace();
        let data = serde_json::json!({
            "messages": ["hello", "world"],
            "step": 3
        });
        ws.save_session_snapshot("sess-001", &data).await.unwrap();
        let loaded = ws.load_session_snapshot("sess-001").await.unwrap();
        assert_eq!(loaded["step"], 3);
        assert_eq!(loaded["messages"][0], "hello");
    }

    #[tokio::test]
    async fn test_session_snapshot_not_found() {
        let ws = test_workspace();
        let err = ws.load_session_snapshot("missing").await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_init_standard_dirs() {
        let ws = test_workspace();
        ws.init_standard_dirs().await.unwrap();
        // Verify directories were created (MemoryFs mkdir is no-op but shouldn't error)
        // Write a file to an agent context dir to verify the path works
        ws.fs
            .write("agents/agent-1/context/notes.md", b"hello")
            .await
            .unwrap();
        let data = ws.fs.read("agents/agent-1/context/notes.md").await.unwrap();
        assert_eq!(data, b"hello");
    }

    #[tokio::test]
    async fn test_root_path_prefix() {
        let fs = Arc::new(MemoryFs::new());
        let config = WorkspaceConfig::new("workspaces/w1", "agent-2");
        let ws = Workspace::new(config, fs);

        ws.save_memory("prefixed memory").await.unwrap();
        let content = ws.load_memory_md().await.unwrap();
        assert!(content.contains("prefixed memory"));
    }

    #[tokio::test]
    async fn test_list_skills_skips_non_json() {
        let ws = test_workspace();
        ws.fs.write("skills/README.md", b"# Skills").await.unwrap();
        let skill = SkillDef {
            name: "codegen".into(),
            description: "Code gen".into(),
            version: "0.1.0".into(),
            tags: vec![],
            parameters: None,
        };
        let json = serde_json::to_string(&skill).unwrap();
        ws.fs.write("skills/codegen.json", json.as_bytes()).await.unwrap();

        let skills = ws.list_skills().await.unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "codegen");
    }
}
