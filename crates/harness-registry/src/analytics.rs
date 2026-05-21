//! Evolution Analyzer: Tracks artifact changes and identifies patterns.
//!
//! This module analyzes the evolution of engineering artifacts to identify
//! patterns, optimization opportunities, and potential issues.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

use crate::artifact::ArtifactMetadata;
use crate::error::{HarnessResult};

/// Pattern identified in artifact evolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangePattern {
    /// Artifact changes frequently (potential instability).
    FrequentChanger {
        artifact_id: String,
        change_count: u32,
        period_days: u32,
    },
    /// Artifact is stable (rarely changes).
    StableArtifact {
        artifact_id: String,
        days_since_change: u32,
    },
    /// Artifact has many dependents (high impact).
    HighImpactArtifact {
        artifact_id: String,
        dependent_count: usize,
    },
    /// Artifact has no dependents (orphan).
    OrphanArtifact {
        artifact_id: String,
    },
    /// Artifact version is outdated.
    OutdatedVersion {
        artifact_id: String,
        current_version: String,
        latest_version: String,
    },
    /// Artifact has circular dependencies.
    CircularDependency {
        artifact_ids: Vec<String>,
    },
}

/// Recommendation for optimizing artifact management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    /// Type of recommendation.
    pub recommendation_type: RecommendationType,
    /// Artifact(s) involved.
    pub artifact_ids: Vec<String>,
    /// Description of the recommendation.
    pub description: String,
    /// Expected impact of implementing the recommendation.
    pub expected_impact: String,
    /// Priority of the recommendation.
    pub priority: RecommendationPriority,
}

/// Type of optimization recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecommendationType {
    /// Consolidate multiple artifacts into one.
    Consolidate,
    /// Split artifact into smaller parts.
    Split,
    /// Update artifact version.
    UpdateVersion,
    /// Add missing dependencies.
    AddDependency,
    /// Remove unused artifact.
    RemoveUnused,
    /// Improve documentation.
    ImproveDocumentation,
    /// Add tests.
    AddTests,
}

/// Priority of a recommendation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecommendationPriority {
    /// Low priority.
    Low,
    /// Medium priority.
    Medium,
    /// High priority.
    High,
    /// Critical priority.
    Critical,
}

/// History entry for an artifact change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeHistoryEntry {
    /// When the change occurred.
    pub timestamp: DateTime<Utc>,
    /// Version after the change.
    pub version: String,
    /// Description of the change.
    pub description: String,
    /// Who made the change.
    pub changed_by: String,
    /// Hash of the content after change.
    pub content_hash: String,
}

/// Statistics for artifact evolution analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionStatistics {
    /// Total number of artifacts analyzed.
    pub total_artifacts: usize,
    /// Average changes per artifact.
    pub avg_changes_per_artifact: f64,
    /// Most frequently changed artifact.
    pub most_changed_artifact: Option<String>,
    /// Most stable artifact.
    pub most_stable_artifact: Option<String>,
    /// Number of orphan artifacts.
    pub orphan_count: usize,
    /// Number of high-impact artifacts.
    pub high_impact_count: usize,
}

/// Analyzer for artifact evolution patterns.
pub struct EvolutionAnalyzer {
    /// Change history for artifacts.
    change_history: HashMap<String, Vec<ChangeHistoryEntry>>,
    /// Current artifact metadata.
    artifacts: HashMap<String, ArtifactMetadata>,
}

impl EvolutionAnalyzer {
    /// Create a new EvolutionAnalyzer.
    pub fn new() -> Self {
        Self {
            change_history: HashMap::new(),
            artifacts: HashMap::new(),
        }
    }

    /// Add an artifact to the analyzer.
    pub fn add_artifact(&mut self, metadata: ArtifactMetadata) {
        self.artifacts.insert(metadata.id.clone(), metadata);
    }

    /// Record a change to an artifact.
    pub fn record_change(
        &mut self,
        artifact_id: &str,
        entry: ChangeHistoryEntry,
    ) -> HarnessResult<()> {
        let history = self.change_history
            .entry(artifact_id.to_string())
            .or_insert_with(Vec::new);
        history.push(entry);
        Ok(())
    }

    /// Analyze artifacts and identify patterns.
    pub fn analyze(&self) -> Vec<ChangePattern> {
        let mut patterns = Vec::new();

        // Identify frequent changers
        for (artifact_id, history) in &self.change_history {
            if history.len() > 5 {
                patterns.push(ChangePattern::FrequentChanger {
                    artifact_id: artifact_id.clone(),
                    change_count: history.len() as u32,
                    period_days: 30, // Assume 30-day period for now
                });
            }
        }

        // Identify stable artifacts (no changes in last 90 days)
        let now = Utc::now();
        for (artifact_id, _metadata) in &self.artifacts {
            if let Some(history) = self.change_history.get(artifact_id) {
                if let Some(last_change) = history.last() {
                    let days_since = (now - last_change.timestamp).num_days();
                    if days_since > 90 {
                        patterns.push(ChangePattern::StableArtifact {
                            artifact_id: artifact_id.clone(),
                            days_since_change: days_since as u32,
                        });
                    }
                }
            } else {
                // No change history at all
                patterns.push(ChangePattern::StableArtifact {
                    artifact_id: artifact_id.clone(),
                    days_since_change: 365, // Assume 1 year if no history
                });
            }
        }

        // Identify orphan artifacts (no dependents)
        let mut dependent_counts: HashMap<String, usize> = HashMap::new();
        for metadata in self.artifacts.values() {
            for dep in &metadata.dependencies {
                *dependent_counts.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        for artifact_id in self.artifacts.keys() {
            if !dependent_counts.contains_key(artifact_id) {
                patterns.push(ChangePattern::OrphanArtifact {
                    artifact_id: artifact_id.clone(),
                });
            }
        }

        // Identify high-impact artifacts (many dependents)
        for (artifact_id, count) in &dependent_counts {
            if *count >= 3 {
                patterns.push(ChangePattern::HighImpactArtifact {
                    artifact_id: artifact_id.clone(),
                    dependent_count: *count,
                });
            }
        }

        patterns
    }

    /// Generate optimization recommendations based on analysis.
    pub fn recommend(&self) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();
        let patterns = self.analyze();

        for pattern in &patterns {
            match pattern {
                ChangePattern::FrequentChanger { artifact_id, change_count, .. } => {
                    if *change_count > 10 {
                        recommendations.push(OptimizationRecommendation {
                            recommendation_type: RecommendationType::Split,
                            artifact_ids: vec![artifact_id.clone()],
                            description: format!(
                                "Artifact '{}' has changed {} times. Consider splitting into smaller, more stable parts.",
                                artifact_id, change_count
                            ),
                            expected_impact: "Reduce change frequency and improve maintainability".to_string(),
                            priority: RecommendationPriority::Medium,
                        });
                    }
                }
                ChangePattern::OrphanArtifact { artifact_id } => {
                    recommendations.push(OptimizationRecommendation {
                        recommendation_type: RecommendationType::RemoveUnused,
                        artifact_ids: vec![artifact_id.clone()],
                        description: format!(
                            "Artifact '{}' has no dependents. Consider removing if unused.",
                            artifact_id
                        ),
                        expected_impact: "Reduce maintenance burden and clutter".to_string(),
                        priority: RecommendationPriority::Low,
                    });
                }
                ChangePattern::HighImpactArtifact { artifact_id, dependent_count } => {
                    recommendations.push(OptimizationRecommendation {
                        recommendation_type: RecommendationType::AddTests,
                        artifact_ids: vec![artifact_id.clone()],
                        description: format!(
                            "Artifact '{}' has {} dependents. Ensure comprehensive test coverage.",
                            artifact_id, dependent_count
                        ),
                        expected_impact: "Prevent regressions affecting multiple artifacts".to_string(),
                        priority: RecommendationPriority::High,
                    });
                }
                _ => {}
            }
        }

        recommendations
    }

    /// Get statistics about artifact evolution.
    pub fn statistics(&self) -> EvolutionStatistics {
        let total_artifacts = self.artifacts.len();
        let total_changes: usize = self.change_history.values().map(|h| h.len()).sum();
        let avg_changes = if total_artifacts > 0 {
            total_changes as f64 / total_artifacts as f64
        } else {
            0.0
        };

        let most_changed = self.change_history
            .iter()
            .max_by_key(|(_, h)| h.len())
            .map(|(id, _)| id.clone());

        let most_stable = self.change_history
            .iter()
            .min_by_key(|(_, h)| h.len())
            .map(|(id, _)| id.clone());

        let orphan_count = self.analyze()
            .iter()
            .filter(|p| matches!(p, ChangePattern::OrphanArtifact { .. }))
            .count();

        let high_impact_count = self.analyze()
            .iter()
            .filter(|p| matches!(p, ChangePattern::HighImpactArtifact { .. }))
            .count();

        EvolutionStatistics {
            total_artifacts,
            avg_changes_per_artifact: avg_changes,
            most_changed_artifact: most_changed,
            most_stable_artifact: most_stable,
            orphan_count,
            high_impact_count,
        }
    }
}

impl Default for EvolutionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_metadata(id: &str, deps: Vec<String>) -> ArtifactMetadata {
        ArtifactMetadata {
            id: id.to_string(),
            name: format!("{}.md", id),
            artifact_type: crate::artifact::ArtifactType::AgentsMd,
            path: PathBuf::from(format!("{}.md", id)),
            version: "1.0.0".to_string(),
            owner: "test-owner".to_string(),
            dependencies: deps,
            created_at: Utc::now(),
            last_modified: Utc::now(),
            content_hash: "abc123".to_string(),
            custom_metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_identify_orphan_artifacts() {
        let mut analyzer = EvolutionAnalyzer::new();

        // Add artifacts with dependencies
        analyzer.add_artifact(create_test_metadata("a", vec!["b".to_string()]));
        analyzer.add_artifact(create_test_metadata("b", vec![]));
        analyzer.add_artifact(create_test_metadata("c", vec![])); // Orphan

        let patterns = analyzer.analyze();
        let orphans: Vec<_> = patterns.iter()
            .filter(|p| matches!(p, ChangePattern::OrphanArtifact { .. }))
            .collect();

        // 'b' and 'c' should be orphans (no one depends on them)
        assert_eq!(orphans.len(), 2);
    }

    #[test]
    fn test_identify_frequent_changers() {
        let mut analyzer = EvolutionAnalyzer::new();

        let metadata = create_test_metadata("frequent", vec![]);
        analyzer.add_artifact(metadata);

        // Add many changes
        for i in 0..15 {
            analyzer.record_change("frequent", ChangeHistoryEntry {
                timestamp: Utc::now(),
                version: format!("1.0.{}", i),
                description: format!("Change {}", i),
                changed_by: "test".to_string(),
                content_hash: format!("hash{}", i),
            }).unwrap();
        }

        let patterns = analyzer.analyze();
        let frequent: Vec<_> = patterns.iter()
            .filter(|p| matches!(p, ChangePattern::FrequentChanger { .. }))
            .collect();

        assert_eq!(frequent.len(), 1);
    }

    #[test]
    fn test_recommendations() {
        let mut analyzer = EvolutionAnalyzer::new();

        // Add an orphan artifact
        analyzer.add_artifact(create_test_metadata("orphan", vec![]));

        let recommendations = analyzer.recommend();
        let remove_recs: Vec<_> = recommendations.iter()
            .filter(|r| r.recommendation_type == RecommendationType::RemoveUnused)
            .collect();

        assert_eq!(remove_recs.len(), 1);
        assert_eq!(remove_recs[0].artifact_ids[0], "orphan");
    }
}
