//! # Skill Version Control
//!
//! Provides version tracking, history, and rollback for skills.
//! Inspired by skill-mcp's SemVer + content hash + rollback.
//!
//! ## Features
//!
//! - Semantic versioning (SemVer)
//! - Content hash for change detection
//! - Version history with snapshots
//! - One-click rollback
//! - Diff between versions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A versioned snapshot of a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnapshot {
    /// Version string (SemVer: "1.2.0")
    pub version: String,
    /// Content hash (SHA-256 or similar)
    pub content_hash: String,
    /// When this version was created
    pub created_at: String,
    /// Who/what created this version
    pub created_by: String,
    /// Changelog for this version
    pub changelog: String,
    /// Skill content at this version
    pub content: String,
    /// Metadata at this version
    pub metadata: serde_json::Value,
}

/// Version history for a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersionHistory {
    /// Skill name
    pub skill_name: String,
    /// Current version
    pub current_version: String,
    /// All versions (ordered by creation)
    pub versions: Vec<SkillSnapshot>,
}

impl SkillVersionHistory {
    /// Create a new version history
    pub fn new(skill_name: impl Into<String>) -> Self {
        Self {
            skill_name: skill_name.into(),
            current_version: "0.0.0".to_string(),
            versions: Vec::new(),
        }
    }

    /// Add a new version
    pub fn add_version(
        &mut self,
        version: impl Into<String>,
        content: impl Into<String>,
        changelog: impl Into<String>,
        created_by: impl Into<String>,
    ) -> &SkillSnapshot {
        let content = content.into();
        let content_hash = compute_hash(&content);
        let version_str = version.into();

        let snapshot = SkillSnapshot {
            version: version_str.clone(),
            content_hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: created_by.into(),
            changelog: changelog.into(),
            content,
            metadata: serde_json::json!({}),
        };

        self.current_version = version_str;
        self.versions.push(snapshot);
        self.versions.last().unwrap()
    }

    /// Get the current version
    pub fn current(&self) -> Option<&SkillSnapshot> {
        self.versions.last()
    }

    /// Get a specific version
    pub fn get_version(&self, version: &str) -> Option<&SkillSnapshot> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// Get all version strings
    pub fn list_versions(&self) -> Vec<&str> {
        self.versions.iter().map(|v| v.version.as_str()).collect()
    }

    /// Rollback to a specific version
    pub fn rollback(&mut self, target_version: &str) -> Result<&SkillSnapshot, String> {
        let idx = self
            .versions
            .iter()
            .position(|v| v.version == target_version)
            .ok_or_else(|| format!("Version {} not found", target_version))?;

        // Create a new snapshot that copies the target version's content
        let target = &self.versions[idx];
        let rollback_snapshot = SkillSnapshot {
            version: increment_patch(&self.current_version),
            content_hash: target.content_hash.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: "rollback".to_string(),
            changelog: format!("Rollback to version {}", target_version),
            content: target.content.clone(),
            metadata: target.metadata.clone(),
        };

        let new_version = rollback_snapshot.version.clone();
        self.versions.push(rollback_snapshot);
        self.current_version = new_version;
        Ok(self.versions.last().unwrap())
    }

    /// Check if content has changed since a version
    pub fn has_changed_since(&self, version: &str, current_content: &str) -> bool {
        let current_hash = compute_hash(current_content);
        self.versions
            .iter()
            .find(|v| v.version == version)
            .map(|v| v.content_hash != current_hash)
            .unwrap_or(true)
    }

    /// Get diff summary between two versions
    pub fn diff_summary(&self, v1: &str, v2: &str) -> Option<DiffSummary> {
        let snap1 = self.get_version(v1)?;
        let snap2 = self.get_version(v2)?;

        let lines1: Vec<&str> = snap1.content.lines().collect();
        let lines2: Vec<&str> = snap2.content.lines().collect();

        let added = lines2.len().saturating_sub(lines1.len());
        let removed = lines1.len().saturating_sub(lines2.len());
        let changed = lines1
            .iter()
            .zip(lines2.iter())
            .filter(|(a, b)| a != b)
            .count();

        Some(DiffSummary {
            from_version: v1.to_string(),
            to_version: v2.to_string(),
            lines_added: added,
            lines_removed: removed,
            lines_changed: changed,
            content_hash_changed: snap1.content_hash != snap2.content_hash,
        })
    }

    /// Get total number of versions
    pub fn version_count(&self) -> usize {
        self.versions.len()
    }
}

/// Summary of differences between two versions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub from_version: String,
    pub to_version: String,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_changed: usize,
    pub content_hash_changed: bool,
}

/// Compute a simple hash for content
pub fn compute_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Increment patch version (e.g., "1.2.3" -> "1.2.4")
fn increment_patch(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 3 {
        let patch: u32 = parts[2].parse().unwrap_or(0);
        format!("{}.{}.{}", parts[0], parts[1], patch + 1)
    } else {
        format!("{}.1", version)
    }
}

/// Version control store: manages version histories for multiple skills
pub struct VersionStore {
    histories: HashMap<String, SkillVersionHistory>,
}

impl VersionStore {
    pub fn new() -> Self {
        Self {
            histories: HashMap::new(),
        }
    }

    /// Get or create version history for a skill
    pub fn get_or_create(&mut self, skill_name: &str) -> &mut SkillVersionHistory {
        self.histories
            .entry(skill_name.to_string())
            .or_insert_with(|| SkillVersionHistory::new(skill_name))
    }

    /// Get version history for a skill
    pub fn get(&self, skill_name: &str) -> Option<&SkillVersionHistory> {
        self.histories.get(skill_name)
    }

    /// List all skills with version history
    pub fn list_skills(&self) -> Vec<&str> {
        self.histories.keys().map(|s| s.as_str()).collect()
    }

    /// Total version count across all skills
    pub fn total_versions(&self) -> usize {
        self.histories.values().map(|h| h.version_count()).sum()
    }
}

impl Default for VersionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_history_add_and_get() {
        let mut history = SkillVersionHistory::new("test-skill");
        history.add_version("1.0.0", "content v1", "Initial release", "system");
        history.add_version("1.1.0", "content v2", "Added feature X", "system");

        assert_eq!(history.current_version, "1.1.0");
        assert_eq!(history.version_count(), 2);
        assert_eq!(history.list_versions(), vec!["1.0.0", "1.1.0"]);
    }

    #[test]
    fn test_version_history_rollback() {
        let mut history = SkillVersionHistory::new("test-skill");
        history.add_version("1.0.0", "content v1", "Initial", "system");
        history.add_version("1.1.0", "content v2", "Feature", "system");
        history.add_version("1.2.0", "content v3", "Bugfix", "system");

        let rolled_back = history.rollback("1.0.0").unwrap();
        assert_eq!(rolled_back.content, "content v1");
        assert_eq!(history.current_version, "1.2.1"); // Patch incremented from current (1.2.0)
        assert_eq!(history.version_count(), 4); // Original 3 + rollback
    }

    #[test]
    fn test_version_history_rollback_not_found() {
        let mut history = SkillVersionHistory::new("test-skill");
        history.add_version("1.0.0", "content", "Initial", "system");

        let result = history.rollback("2.0.0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_version_history_has_changed() {
        let mut history = SkillVersionHistory::new("test-skill");
        history.add_version("1.0.0", "original content", "Initial", "system");

        assert!(!history.has_changed_since("1.0.0", "original content"));
        assert!(history.has_changed_since("1.0.0", "modified content"));
    }

    #[test]
    fn test_version_history_diff_summary() {
        let mut history = SkillVersionHistory::new("test-skill");
        history.add_version("1.0.0", "line1\nline2\nline3", "Initial", "system");
        history.add_version(
            "1.1.0",
            "line1\nline2\nline3\nline4",
            "Added line",
            "system",
        );

        let diff = history.diff_summary("1.0.0", "1.1.0").unwrap();
        assert!(diff.content_hash_changed);
        assert_eq!(diff.from_version, "1.0.0");
        assert_eq!(diff.to_version, "1.1.0");
    }

    #[test]
    fn test_increment_patch() {
        assert_eq!(increment_patch("1.2.3"), "1.2.4");
        assert_eq!(increment_patch("1.0.0"), "1.0.1");
        assert_eq!(increment_patch("0.1"), "0.1.1");
    }

    #[test]
    fn test_version_store() {
        let mut store = VersionStore::new();
        store
            .get_or_create("skill-a")
            .add_version("1.0.0", "a", "init", "sys");
        store
            .get_or_create("skill-b")
            .add_version("1.0.0", "b", "init", "sys");

        assert_eq!(store.list_skills().len(), 2);
        assert_eq!(store.total_versions(), 2);
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = compute_hash("hello");
        let hash2 = compute_hash("hello");
        let hash3 = compute_hash("world");
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
