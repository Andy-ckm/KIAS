//! Zero-LLM entity extraction using regex patterns.
//!
//! Phase 1 of the GBrain pattern absorption plan — extracts entities and
//! relationships from plain text without any LLM calls.

use std::collections::HashMap;

use regex::Regex;

use crate::graph::{Edge, KnowledgeGraph, KnowledgeNode, NodeType};

/// Types of relationships that can be extracted from text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationType {
    WorksAt,
    InvestedIn,
    Founded,
    Advises,
    Attended,
    CollaboratedWith,
    Mentions,
    RelatedTo,
}

/// A compiled regex pattern paired with the relation it represents.
pub struct RelationPattern {
    /// The relation type this pattern detects.
    pub relation: RelationType,
    /// Compiled regex with named/positional capture groups.
    pub pattern: Regex,
    /// Index of the capture group that yields the *subject* entity.
    pub subject_group: usize,
    /// Index of the capture group that yields the *object* entity.
    pub object_group: usize,
}

/// A single extracted (subject, relation, object) triple.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedRelation {
    pub subject: String,
    pub relation: RelationType,
    pub object: String,
    pub source_text: String,
    pub confidence: f64,
}

/// Regex-based entity extractor (zero LLM cost).
pub struct EntityExtractor {
    patterns: Vec<RelationPattern>,
    #[allow(dead_code)]
    known_entities: HashMap<String, String>,
}

impl Default for EntityExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityExtractor {
    /// Create an extractor pre-loaded with the default relation patterns.
    pub fn new() -> Self {
        let patterns = vec![
            // "Alice works at Google" / "Alice is employed by Google"
            RelationPattern {
                relation: RelationType::WorksAt,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:works?\s+at|employed\s+by|is\s+at)\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice invested in Acme" / "Alice backed Acme"
            RelationPattern {
                relation: RelationType::InvestedIn,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:invested?\s+in|backed|funded)\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice founded Acme" / "Alice co-founded Acme"
            RelationPattern {
                relation: RelationType::Founded,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:founded|co-founded|started|created)\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice advises Acme" / "Alice is advisor to Acme"
            RelationPattern {
                relation: RelationType::Advises,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:advises?|advisor\s+(?:to|of)|board\s+member\s+(?:of|at))\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice attended the meeting" / "Alice participated in QBR"
            RelationPattern {
                relation: RelationType::Attended,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:attended|participated\s+in|joined)\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // "Alice and Bob collaborated"
            RelationPattern {
                relation: RelationType::CollaboratedWith,
                pattern: Regex::new(
                    r"(?i)([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+and\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s+[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+(?:collaborated|worked\s+together|partnered)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // Chinese patterns: "张三 在 Google 工作"
            RelationPattern {
                relation: RelationType::WorksAt,
                pattern: Regex::new(
                    r"([\u{4e00}-\u{9fff}]+)\s+在\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s*[A-Za-z\u{4e00}-\u{9fff}]+)*)\s+工作",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // Chinese patterns: "张三 创办了 Acme"
            RelationPattern {
                relation: RelationType::Founded,
                pattern: Regex::new(
                    r"([\u{4e00}-\u{9fff}]+)\s+创办了?\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s*[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
            // Chinese patterns: "张三 投资了 Acme"
            RelationPattern {
                relation: RelationType::InvestedIn,
                pattern: Regex::new(
                    r"([\u{4e00}-\u{9fff}]+)\s+投资了?\s+([A-Za-z\u{4e00}-\u{9fff}]+(?:\s*[A-Za-z\u{4e00}-\u{9fff}]+)*)",
                )
                .unwrap(),
                subject_group: 1,
                object_group: 2,
            },
        ];

        Self {
            patterns,
            known_entities: HashMap::new(),
        }
    }

    /// Add a known entity to the internal dictionary.
    pub fn add_known_entity(&mut self, name: String, entity_type: String) {
        self.known_entities.insert(name, entity_type);
    }

    /// Extract all relation triples from `text`.
    pub fn extract(&self, text: &str) -> Vec<ExtractedRelation> {
        let mut results = Vec::new();
        for pattern in &self.patterns {
            for cap in pattern.pattern.captures_iter(text) {
                if let (Some(subject), Some(object)) = (
                    cap.get(pattern.subject_group),
                    cap.get(pattern.object_group),
                ) {
                    let subject_str = subject.as_str().trim().to_string();
                    let object_str = object.as_str().trim().to_string();
                    if subject_str.is_empty() || object_str.is_empty() {
                        continue;
                    }
                    results.push(ExtractedRelation {
                        subject: subject_str,
                        relation: pattern.relation.clone(),
                        object: object_str,
                        source_text: text.to_string(),
                        confidence: 0.8,
                    });
                }
            }
        }
        results
    }

    /// Extract relations from `text` and add the corresponding nodes and edges
    /// to the given `KnowledgeGraph`.
    pub fn extract_and_update(
        &self,
        text: &str,
        graph: &mut KnowledgeGraph,
    ) -> Vec<ExtractedRelation> {
        let relations = self.extract(text);
        for rel in &relations {
            if graph.get_node(&rel.subject).is_none() {
                graph.add_node(KnowledgeNode {
                    id: rel.subject.clone(),
                    content: rel.subject.clone(),
                    node_type: NodeType::Entity,
                    metadata: HashMap::new(),
                });
            }
            if graph.get_node(&rel.object).is_none() {
                graph.add_node(KnowledgeNode {
                    id: rel.object.clone(),
                    content: rel.object.clone(),
                    node_type: NodeType::Entity,
                    metadata: HashMap::new(),
                });
            }
            graph.add_edge(Edge {
                from: rel.subject.clone(),
                to: rel.object.clone(),
                relationship: format!("{:?}", rel.relation),
                weight: rel.confidence,
            });
        }
        relations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_works_at_extraction() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("Alice works at Google");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "Alice");
        assert_eq!(results[0].relation, RelationType::WorksAt);
        assert_eq!(results[0].object, "Google");
    }

    #[test]
    fn test_multiple_relations_in_text() {
        let extractor = EntityExtractor::new();
        let text = "Alice works at Google. Bob founded Acme. Carol invested in Startup.";
        let results = extractor.extract(text);
        assert_eq!(results.len(), 3);

        let works_at = results
            .iter()
            .find(|r| r.relation == RelationType::WorksAt)
            .unwrap();
        assert_eq!(works_at.subject, "Alice");
        assert_eq!(works_at.object, "Google");

        let founded = results
            .iter()
            .find(|r| r.relation == RelationType::Founded)
            .unwrap();
        assert_eq!(founded.subject, "Bob");
        assert_eq!(founded.object, "Acme");

        let invested = results
            .iter()
            .find(|r| r.relation == RelationType::InvestedIn)
            .unwrap();
        assert_eq!(invested.subject, "Carol");
        assert_eq!(invested.object, "Startup");
    }

    #[test]
    fn test_no_match_returns_empty() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("The quick brown fox jumps over the lazy dog.");
        assert!(results.is_empty());
    }

    #[test]
    fn test_case_insensitive_extraction() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("alice works at google");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "alice");
        assert_eq!(results[0].object, "google");
    }

    #[test]
    fn test_founded_pattern() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("Elon co-founded SpaceX");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relation, RelationType::Founded);
        assert_eq!(results[0].subject, "Elon");
        assert_eq!(results[0].object, "SpaceX");
    }

    #[test]
    fn test_advises_pattern() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("Sam advises YC");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relation, RelationType::Advises);
    }

    #[test]
    fn test_collaborated_with_pattern() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("Alice and Bob collaborated on the project");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relation, RelationType::CollaboratedWith);
        assert_eq!(results[0].subject, "Alice");
        assert_eq!(results[0].object, "Bob");
    }

    #[test]
    fn test_chinese_works_at() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("张三 在 Google 工作");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "张三");
        assert_eq!(results[0].relation, RelationType::WorksAt);
        assert_eq!(results[0].object, "Google");
    }

    #[test]
    fn test_chinese_founded() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("李四 创办了 ByteDance");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "李四");
        assert_eq!(results[0].relation, RelationType::Founded);
        assert_eq!(results[0].object, "ByteDance");
    }

    #[test]
    fn test_chinese_invested() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("王五 投资了 Alibaba");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].subject, "王五");
        assert_eq!(results[0].relation, RelationType::InvestedIn);
        assert_eq!(results[0].object, "Alibaba");
    }

    #[test]
    fn test_extract_and_update_graph() {
        let extractor = EntityExtractor::new();
        let mut graph = KnowledgeGraph::new();

        let relations = extractor.extract_and_update("Alice works at Google", &mut graph);
        assert_eq!(relations.len(), 1);

        // Nodes should exist
        assert!(graph.get_node("Alice").is_some());
        assert!(graph.get_node("Google").is_some());
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);

        // Edge should point from Alice to Google
        let outgoing = graph.get_outgoing_edges("Alice");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].to, "Google");
        assert!(outgoing[0].relationship.contains("WorksAt"));
    }

    #[test]
    fn test_extract_and_update_no_duplicates() {
        let extractor = EntityExtractor::new();
        let mut graph = KnowledgeGraph::new();

        // Run twice — should not duplicate nodes
        extractor.extract_and_update("Alice works at Google", &mut graph);
        extractor.extract_and_update("Alice works at Google", &mut graph);

        assert_eq!(graph.node_count(), 2); // Alice, Google
        assert_eq!(graph.edge_count(), 2); // but edges are appended
    }

    #[test]
    fn test_attended_pattern() {
        let extractor = EntityExtractor::new();
        let results = extractor.extract("Alice attended the quarterly review");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relation, RelationType::Attended);
        assert_eq!(results[0].subject, "Alice");
        assert_eq!(results[0].object, "the quarterly review");
    }
}
