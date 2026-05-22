//! Unified Namespace (UNS) — hierarchical topic/namespace governance.
//!
//! Provides a structured namespace for agent communication, inspired by:
//! - EMQX UNS (Unified Namespace) for industrial IoT
//! - MQTT topic hierarchy with access control
//! - ISA-95 Purdue model namespace structure
//!
//! Pattern: Hierarchical namespace with RBAC, schema binding, and audit.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Namespace node types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NamespaceNodeType {
    /// Root namespace (enterprise level).
    Root,
    /// Area/site namespace.
    Area,
    /// Line/cell namespace.
    Line,
    /// Device/agent namespace.
    Device,
    /// Topic for data exchange.
    Topic,
}

/// A node in the namespace tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceNode {
    pub path: String,
    pub node_type: NamespaceNodeType,
    pub display_name: String,
    pub description: String,
    /// Allowed readers (agent IDs or role names).
    pub read_permissions: HashSet<String>,
    /// Allowed writers.
    pub write_permissions: HashSet<String>,
    /// Schema ID binding (if any).
    pub schema_id: Option<String>,
    /// Metadata.
    pub metadata: HashMap<String, String>,
    pub created_at_ms: u64,
}

/// UNS manager — manages the unified namespace tree.
pub struct UnifiedNamespace {
    nodes: HashMap<String, NamespaceNode>,
    /// Default permissions for new nodes.
    #[allow(dead_code)]
    default_read: HashSet<String>,
    #[allow(dead_code)]
    default_write: HashSet<String>,
}

impl UnifiedNamespace {
    pub fn new() -> Self {
        let mut ns = Self {
            nodes: HashMap::new(),
            default_read: HashSet::new(),
            default_write: HashSet::new(),
        };
        // Create root
        ns.nodes.insert(
            "/".to_string(),
            NamespaceNode {
                path: "/".to_string(),
                node_type: NamespaceNodeType::Root,
                display_name: "Root".to_string(),
                description: "Enterprise root namespace".to_string(),
                read_permissions: HashSet::new(),
                write_permissions: HashSet::new(),
                schema_id: None,
                metadata: HashMap::new(),
                created_at_ms: now_ms(),
            },
        );
        ns
    }

    /// Add a namespace node.
    pub fn add_node(&mut self, node: NamespaceNode) -> Result<(), String> {
        if self.nodes.contains_key(&node.path) {
            return Err(format!("Namespace '{}' already exists", node.path));
        }
        // Validate parent exists
        let parent = parent_path(&node.path);
        if !self.nodes.contains_key(&parent) && !parent.is_empty() {
            return Err(format!("Parent namespace '{}' does not exist", parent));
        }
        self.nodes.insert(node.path.clone(), node);
        Ok(())
    }

    /// Remove a namespace node (and all children).
    pub fn remove_node(&mut self, path: &str) -> Result<(), String> {
        if path == "/" {
            return Err("Cannot remove root namespace".to_string());
        }
        // Remove children first
        let children: Vec<String> = self
            .nodes
            .keys()
            .filter(|k| k.starts_with(path) && *k != path)
            .cloned()
            .collect();
        for child in children {
            self.nodes.remove(&child);
        }
        self.nodes
            .remove(path)
            .ok_or_else(|| format!("Namespace '{}' not found", path))?;
        Ok(())
    }

    /// Check read permission.
    pub fn can_read(&self, path: &str, identity: &str) -> bool {
        // Walk up the tree to find a permission
        let mut current = path.to_string();
        loop {
            if let Some(node) = self.nodes.get(&current) {
                // If node has explicit permissions, use ONLY those (no inheritance)
                if !node.read_permissions.is_empty() {
                    return node.read_permissions.contains(identity);
                }
                // Empty permissions mean inherit from parent (continue loop)
            }
            if current == "/" || current.is_empty() {
                break;
            }
            current = parent_path(&current);
            if current.is_empty() {
                break;
            }
        }
        false
    }

    /// Check write permission.
    pub fn can_write(&self, path: &str, identity: &str) -> bool {
        let mut current = path.to_string();
        loop {
            if let Some(node) = self.nodes.get(&current) {
                // If node has explicit permissions, use ONLY those (no inheritance)
                if !node.write_permissions.is_empty() {
                    return node.write_permissions.contains(identity);
                }
                // Empty permissions mean inherit from parent (continue loop)
            }
            if current == "/" || current.is_empty() {
                break;
            }
            current = parent_path(&current);
            if current.is_empty() {
                break;
            }
        }
        false
    }

    /// Bind a schema to a namespace path.
    pub fn bind_schema(&mut self, path: &str, schema_id: &str) -> Result<(), String> {
        let node = self
            .nodes
            .get_mut(path)
            .ok_or_else(|| format!("Namespace '{}' not found", path))?;
        node.schema_id = Some(schema_id.to_string());
        Ok(())
    }

    /// Get a namespace node.
    pub fn get(&self, path: &str) -> Option<&NamespaceNode> {
        self.nodes.get(path)
    }

    /// List all children of a path.
    pub fn children(&self, path: &str) -> Vec<&NamespaceNode> {
        let prefix = if path == "/" {
            "/".to_string()
        } else {
            format!("{}/", path)
        };
        self.nodes
            .values()
            .filter(|n| n.path.starts_with(&prefix) && n.path != path)
            .collect()
    }

    /// List all paths matching a glob pattern (simple * wildcard).
    pub fn glob(&self, pattern: &str) -> Vec<&NamespaceNode> {
        let _regex_pattern = pattern.replace("*", "([^/]+)");
        self.nodes
            .values()
            .filter(|n| simple_glob_match(&n.path, pattern))
            .collect()
    }

    pub fn size(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for UnifiedNamespace {
    fn default() -> Self {
        Self::new()
    }
}

fn parent_path(path: &str) -> String {
    if path == "/" {
        return String::new();
    }
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(pos) => path[..pos].to_string(),
        None => String::new(),
    }
}

fn simple_glob_match(text: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return text == pattern;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 2 {
        text.starts_with(parts[0]) && text.ends_with(parts[1])
    } else {
        text.contains(parts[0])
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_root() {
        let ns = UnifiedNamespace::new();
        assert_eq!(ns.size(), 1);
        assert!(ns.get("/").is_some());
    }

    #[test]
    fn test_add_and_remove() {
        let mut ns = UnifiedNamespace::new();
        ns.add_node(NamespaceNode {
            path: "/enterprise".to_string(),
            node_type: NamespaceNodeType::Area,
            display_name: "Enterprise".to_string(),
            description: "".to_string(),
            read_permissions: HashSet::new(),
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();
        assert_eq!(ns.size(), 2);
        ns.remove_node("/enterprise").unwrap();
        assert_eq!(ns.size(), 1);
    }

    #[test]
    fn test_hierarchy() {
        let mut ns = UnifiedNamespace::new();
        ns.add_node(NamespaceNode {
            path: "/factory".to_string(),
            node_type: NamespaceNodeType::Area,
            display_name: "Factory".into(),
            description: "".into(),
            read_permissions: HashSet::new(),
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();
        ns.add_node(NamespaceNode {
            path: "/factory/line1".to_string(),
            node_type: NamespaceNodeType::Line,
            display_name: "Line 1".into(),
            description: "".into(),
            read_permissions: HashSet::new(),
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();
        ns.add_node(NamespaceNode {
            path: "/factory/line1/agent1".to_string(),
            node_type: NamespaceNodeType::Device,
            display_name: "Agent 1".into(),
            description: "".into(),
            read_permissions: HashSet::new(),
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();

        let children = ns.children("/factory");
        assert_eq!(children.len(), 2); // line1 + agent1
    }

    #[test]
    fn test_permissions() {
        let mut ns = UnifiedNamespace::new();
        let mut read_perms = HashSet::new();
        read_perms.insert("agent-1".to_string());
        ns.add_node(NamespaceNode {
            path: "/secure".to_string(),
            node_type: NamespaceNodeType::Area,
            display_name: "Secure".into(),
            description: "".into(),
            read_permissions: read_perms,
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();

        assert!(ns.can_read("/secure", "agent-1"));
        assert!(!ns.can_read("/secure", "agent-2"));
    }

    #[test]
    fn test_schema_binding() {
        let mut ns = UnifiedNamespace::new();
        ns.add_node(NamespaceNode {
            path: "/data".to_string(),
            node_type: NamespaceNodeType::Topic,
            display_name: "Data".into(),
            description: "".into(),
            read_permissions: HashSet::new(),
            write_permissions: HashSet::new(),
            schema_id: None,
            metadata: HashMap::new(),
            created_at_ms: now_ms(),
        })
        .unwrap();
        ns.bind_schema("/data", "schema-001").unwrap();
        assert_eq!(
            ns.get("/data").unwrap().schema_id.as_deref(),
            Some("schema-001")
        );
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("/a/b/c"), "/a/b");
        assert_eq!(parent_path("/a"), "/");
        assert_eq!(parent_path("/"), "");
    }
}
