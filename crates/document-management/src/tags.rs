//! Document tags and classification.
//!
//! Supports hierarchical tagging, auto-classification by content,
//! and tag-based filtering/search.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A document tag with optional hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Tag {
    pub name: String,
    pub category: String,
    pub parent: Option<String>,
}

impl Tag {
    pub fn new(name: impl Into<String>, category: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            parent: None,
        }
    }

    pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// Get the full path like "category/name".
    pub fn full_path(&self) -> String {
        if let Some(ref parent) = self.parent {
            format!("{}/{}/{}", self.category, parent, self.name)
        } else {
            format!("{}/{}", self.category, self.name)
        }
    }
}

/// Document classification level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Classification {
    Public,
    Internal,
    Confidential,
    Restricted,
    TopSecret,
}

impl Classification {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Public => "Public",
            Self::Internal => "Internal",
            Self::Confidential => "Confidential",
            Self::Restricted => "Restricted",
            Self::TopSecret => "Top Secret",
        }
    }
}

/// Tag manager for documents.
pub struct TagManager {
    /// document_id -> tags.
    doc_tags: HashMap<String, HashSet<Tag>>,
    /// tag_name -> document_ids (inverted index).
    tag_index: HashMap<String, HashSet<String>>,
    /// document_id -> classification.
    classifications: HashMap<String, Classification>,
}

impl TagManager {
    pub fn new() -> Self {
        Self {
            doc_tags: HashMap::new(),
            tag_index: HashMap::new(),
            classifications: HashMap::new(),
        }
    }

    /// Add a tag to a document.
    pub fn add_tag(&mut self, doc_id: &str, tag: Tag) {
        let tag_name = tag.name.clone();
        self.doc_tags
            .entry(doc_id.to_string())
            .or_default()
            .insert(tag);
        self.tag_index
            .entry(tag_name)
            .or_default()
            .insert(doc_id.to_string());
    }

    /// Remove a tag from a document.
    pub fn remove_tag(&mut self, doc_id: &str, tag_name: &str) {
        if let Some(tags) = self.doc_tags.get_mut(doc_id) {
            tags.retain(|t| t.name != tag_name);
        }
        if let Some(docs) = self.tag_index.get_mut(tag_name) {
            docs.remove(doc_id);
        }
    }

    /// Get all tags for a document.
    pub fn get_tags(&self, doc_id: &str) -> Vec<&Tag> {
        self.doc_tags
            .get(doc_id)
            .map(|tags| tags.iter().collect())
            .unwrap_or_default()
    }

    /// Find all documents with a specific tag.
    pub fn find_by_tag(&self, tag_name: &str) -> Vec<&str> {
        self.tag_index
            .get(tag_name)
            .map(|docs| docs.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Find documents with ALL of the given tags (AND query).
    pub fn find_by_tags_all(&self, tag_names: &[&str]) -> Vec<String> {
        if tag_names.is_empty() {
            return Vec::new();
        }

        let sets: Vec<&HashSet<String>> = tag_names
            .iter()
            .filter_map(|name| self.tag_index.get(*name))
            .collect();

        if sets.len() != tag_names.len() {
            return Vec::new();
        }

        let mut result = sets[0].clone();
        for set in &sets[1..] {
            result = result.intersection(set).cloned().collect();
        }
        result.into_iter().collect()
    }

    /// Find documents with ANY of the given tags (OR query).
    pub fn find_by_tags_any(&self, tag_names: &[&str]) -> Vec<String> {
        let mut result = HashSet::new();
        for name in tag_names {
            if let Some(docs) = self.tag_index.get(*name) {
                result.extend(docs.iter().cloned());
            }
        }
        result.into_iter().collect()
    }

    /// Set the classification level for a document.
    pub fn set_classification(&mut self, doc_id: &str, classification: Classification) {
        self.classifications
            .insert(doc_id.to_string(), classification);
    }

    /// Get the classification level for a document.
    pub fn get_classification(&self, doc_id: &str) -> Option<Classification> {
        self.classifications.get(doc_id).copied()
    }

    /// Auto-classify a document based on content keywords.
    pub fn auto_classify(&mut self, doc_id: &str, content: &str) -> Classification {
        let lower = content.to_lowercase();

        let classification = if lower.contains("top secret") || lower.contains("绝密") {
            Classification::TopSecret
        } else if lower.contains("restricted") || lower.contains("机密") || lower.contains("gxp")
        {
            Classification::Restricted
        } else if lower.contains("confidential") || lower.contains("保密") {
            Classification::Confidential
        } else if lower.contains("internal") || lower.contains("内部") {
            Classification::Internal
        } else {
            Classification::Public
        };

        self.classifications
            .insert(doc_id.to_string(), classification);
        classification
    }

    /// Get all unique tags in the system.
    pub fn all_tags(&self) -> Vec<&Tag> {
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for tags in self.doc_tags.values() {
            for tag in tags {
                if seen.insert(&tag.name) {
                    result.push(tag);
                }
            }
        }
        result
    }

    /// Get tag statistics (tag_name -> count).
    pub fn tag_stats(&self) -> HashMap<&str, usize> {
        self.tag_index
            .iter()
            .map(|(name, docs)| (name.as_str(), docs.len()))
            .collect()
    }
}

impl Default for TagManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_tags() {
        let mut mgr = TagManager::new();
        let tag = Tag::new("important", "priority");
        mgr.add_tag("doc1", tag);

        let tags = mgr.get_tags("doc1");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "important");
    }

    #[test]
    fn test_remove_tag() {
        let mut mgr = TagManager::new();
        mgr.add_tag("doc1", Tag::new("tag1", "cat"));
        mgr.add_tag("doc1", Tag::new("tag2", "cat"));
        mgr.remove_tag("doc1", "tag1");

        let tags = mgr.get_tags("doc1");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "tag2");
    }

    #[test]
    fn test_find_by_tag() {
        let mut mgr = TagManager::new();
        mgr.add_tag("doc1", Tag::new("rust", "lang"));
        mgr.add_tag("doc2", Tag::new("rust", "lang"));
        mgr.add_tag("doc3", Tag::new("python", "lang"));

        let results = mgr.find_by_tag("rust");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_find_by_tags_all() {
        let mut mgr = TagManager::new();
        mgr.add_tag("doc1", Tag::new("rust", "lang"));
        mgr.add_tag("doc1", Tag::new("important", "priority"));
        mgr.add_tag("doc2", Tag::new("rust", "lang"));

        let results = mgr.find_by_tags_all(&["rust", "important"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "doc1");
    }

    #[test]
    fn test_find_by_tags_any() {
        let mut mgr = TagManager::new();
        mgr.add_tag("doc1", Tag::new("rust", "lang"));
        mgr.add_tag("doc2", Tag::new("python", "lang"));

        let results = mgr.find_by_tags_any(&["rust", "python"]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_classification() {
        let mut mgr = TagManager::new();
        mgr.set_classification("doc1", Classification::Confidential);
        assert_eq!(
            mgr.get_classification("doc1"),
            Some(Classification::Confidential)
        );
    }

    #[test]
    fn test_auto_classify() {
        let mut mgr = TagManager::new();
        let c = mgr.auto_classify("doc1", "This is a GxP regulated document");
        assert_eq!(c, Classification::Restricted);

        let c2 = mgr.auto_classify("doc2", "Public announcement");
        assert_eq!(c2, Classification::Public);
    }

    #[test]
    fn test_tag_full_path() {
        let tag = Tag::new("rust", "lang").with_parent("programming");
        assert_eq!(tag.full_path(), "lang/programming/rust");
    }

    #[test]
    fn test_tag_stats() {
        let mut mgr = TagManager::new();
        mgr.add_tag("doc1", Tag::new("rust", "lang"));
        mgr.add_tag("doc2", Tag::new("rust", "lang"));
        mgr.add_tag("doc3", Tag::new("python", "lang"));

        let stats = mgr.tag_stats();
        assert_eq!(stats.get("rust"), Some(&2));
        assert_eq!(stats.get("python"), Some(&1));
    }
}
