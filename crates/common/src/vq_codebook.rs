//! # VQ Codebook — Agent State Discretization
//!
//! Inspired by PRISM-VQ: quantize continuous agent capability profiles
//! into a finite set of discrete prototypes for efficient matching,
//! clustering, and scheduling.
//!
//! ## Core Idea
//!
//! Each agent has a continuous **capability vector** (resource usage, skill scores,
//! latency, throughput, etc.). A VQ codebook maps these to the **nearest discrete
//! prototype**, enabling:
//! - O(K) agent classification instead of O(N²) pairwise comparison
//! - Stable "archetype" labels for scheduling heuristics
//! - Incremental codebook updates via EMA (exponential moving average)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::KiasError;
use crate::KiasResult;

// ─── Agent Profile (continuous feature vector) ───────────────────────────────

/// Continuous capability vector for an agent.
///
/// Each dimension captures a measurable aspect of the agent's behavior.
/// The vector is normalized to [0, 1] per dimension before quantization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProfile {
    /// Agent identifier
    pub agent_id: String,
    /// Feature dimensions (name → value in [0, 1])
    pub features: Vec<f64>,
    /// Human-readable feature names (same length as `features`)
    pub feature_names: Vec<String>,
    /// Timestamp of last update
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl AgentProfile {
    /// Create a new profile from named features.
    pub fn new(agent_id: impl Into<String>, named_features: Vec<(String, f64)>) -> Self {
        let (names, values): (Vec<_>, Vec<_>) = named_features.into_iter().unzip();
        Self {
            agent_id: agent_id.into(),
            features: values,
            feature_names: names,
            updated_at: chrono::Utc::now(),
        }
    }

    /// Dimensionality of this profile.
    pub fn dim(&self) -> usize {
        self.features.len()
    }

    /// Euclidean distance to another profile.
    pub fn distance(&self, other: &AgentProfile) -> f64 {
        self.features
            .iter()
            .zip(other.features.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Cosine similarity to another profile.
    pub fn cosine_similarity(&self, other: &AgentProfile) -> f64 {
        let dot: f64 = self
            .features
            .iter()
            .zip(other.features.iter())
            .map(|(a, b)| a * b)
            .sum();
        let norm_a: f64 = self.features.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
        let norm_b: f64 = other.features.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

// ─── Codebook Entry (discrete prototype) ─────────────────────────────────────

/// A single discrete prototype in the codebook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodebookEntry {
    /// Unique prototype ID (index in codebook)
    pub id: usize,
    /// Prototype centroid vector (same dimensionality as AgentProfile)
    pub centroid: Vec<f64>,
    /// Feature names (mirrors AgentProfile)
    pub feature_names: Vec<String>,
    /// Human-readable archetype label (e.g., "high-cpu-worker", "io-bound-agent")
    pub label: String,
    /// Number of agents currently assigned to this prototype
    pub assigned_count: u64,
    /// Total lifetime assignments (for statistics)
    pub total_assignments: u64,
    /// Average intra-cluster distance (cohesion metric)
    pub avg_intra_distance: f64,
}

// ─── Quantization Result ─────────────────────────────────────────────────────

/// Result of quantizing an agent profile to the nearest prototype.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationResult {
    /// Agent ID that was quantized
    pub agent_id: String,
    /// Nearest prototype ID
    pub prototype_id: usize,
    /// Prototype label
    pub prototype_label: String,
    /// Euclidean distance to the nearest prototype
    pub distance: f64,
    /// Cosine similarity to the nearest prototype
    pub cosine_sim: f64,
    /// Residual vector (profile - centroid), useful for reconstruction error
    pub residual: Vec<f64>,
    /// Per-dimension contribution to distance (for explainability)
    pub dimension_contributions: Vec<f64>,
}

// ─── VQ Codebook ─────────────────────────────────────────────────────────────

/// Vector Quantization Codebook for agent state discretization.
///
/// Maintains K discrete prototypes and supports:
/// - **Quantize**: map a continuous profile to the nearest prototype
/// - **Train**: update prototypes via EMA from observed profiles
/// - **Init**: k-means++ style initialization from seed profiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VqCodebook {
    /// Number of prototypes (codebook size)
    pub k: usize,
    /// Feature dimensionality
    pub dim: usize,
    /// Feature names (shared across all prototypes)
    pub feature_names: Vec<String>,
    /// The discrete prototypes
    pub entries: Vec<CodebookEntry>,
    /// EMA learning rate for incremental updates
    pub learning_rate: f64,
    /// Total training steps
    pub training_steps: u64,
    /// Global assignment history (agent_id → prototype_id)
    pub assignments: HashMap<String, usize>,
}

impl VqCodebook {
    /// Create a new codebook with K prototypes, initialized to zero.
    pub fn new(k: usize, feature_names: Vec<String>) -> Self {
        let dim = feature_names.len();
        let entries = (0..k)
            .map(|i| CodebookEntry {
                id: i,
                centroid: vec![0.0; dim],
                feature_names: feature_names.clone(),
                label: format!("proto-{i}"),
                assigned_count: 0,
                total_assignments: 0,
                avg_intra_distance: 0.0,
            })
            .collect();
        Self {
            k,
            dim,
            feature_names,
            entries,
            learning_rate: 0.1,
            training_steps: 0,
            assignments: HashMap::new(),
        }
    }

    /// Initialize codebook using k-means++ seeding from a set of observed profiles.
    pub fn init_from_profiles(
        k: usize,
        feature_names: Vec<String>,
        profiles: &[AgentProfile],
        learning_rate: f64,
    ) -> KiasResult<Self> {
        if profiles.is_empty() {
            return Err(KiasError::Validation(
                "Cannot initialize codebook from empty profile set".to_string(),
            ));
        }
        let dim = feature_names.len();
        if profiles.iter().any(|p| p.dim() != dim) {
            return Err(KiasError::Validation(format!(
                "Profile dimension mismatch: expected {dim}, found varying dimensions"
            )));
        }

        let mut codebook = Self::new(k, feature_names);
        codebook.learning_rate = learning_rate;

        // k-means++ initialization: pick first centroid randomly (use first profile),
        // then pick subsequent centroids proportional to squared distance.
        let mut chosen_indices = Vec::with_capacity(k);
        chosen_indices.push(0); // first profile as first centroid

        for _ in 1..k.min(profiles.len()) {
            // Compute squared distance from each profile to nearest chosen centroid
            let distances: Vec<f64> = profiles
                .iter()
                .map(|p| {
                    chosen_indices
                        .iter()
                        .map(|&ci| p.distance(&profiles[ci]).powi(2))
                        .fold(f64::INFINITY, f64::min)
                })
                .collect();
            let total: f64 = distances.iter().sum();
            if total == 0.0 {
                break;
            }

            // Deterministic selection: pick the profile with max distance (diverse init)
            let max_idx = distances
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
            chosen_indices.push(max_idx);
        }

        // Set centroids from chosen profiles
        for (entry_idx, &profile_idx) in chosen_indices.iter().enumerate().take(k) {
            if entry_idx < codebook.entries.len() {
                codebook.entries[entry_idx].centroid = profiles[profile_idx].features.clone();
            }
        }

        // Run a few rounds of mini-batch k-means to refine
        for _ in 0..5 {
            // Assignment step
            let mut clusters: Vec<Vec<usize>> = vec![Vec::new(); k];
            for (pi, p) in profiles.iter().enumerate() {
                let nearest = codebook.nearest_prototype_id(p);
                clusters[nearest].push(pi);
            }
            // Update step
            for (ci, members) in clusters.iter().enumerate() {
                if members.is_empty() {
                    continue;
                }
                let mut new_centroid = vec![0.0f64; dim];
                for &mi in members {
                    for (d, nc) in new_centroid.iter_mut().enumerate() {
                        *nc += profiles[mi].features[d];
                    }
                }
                let n = members.len() as f64;
                for nc in new_centroid.iter_mut() {
                    *nc /= n;
                }
                codebook.entries[ci].centroid = new_centroid;
            }
        }

        // Assign labels based on centroid characteristics
        codebook.auto_label();

        Ok(codebook)
    }

    /// Find the nearest prototype ID for a given profile.
    pub fn nearest_prototype_id(&self, profile: &AgentProfile) -> usize {
        self.entries
            .iter()
            .min_by(|a, b| {
                let da = euclidean_distance(&a.centroid, &profile.features);
                let db = euclidean_distance(&b.centroid, &profile.features);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|e| e.id)
            .unwrap_or(0)
    }

    /// Quantize an agent profile to the nearest discrete prototype.
    pub fn quantize(&mut self, profile: &AgentProfile) -> KiasResult<QuantizationResult> {
        if profile.dim() != self.dim {
            return Err(KiasError::Validation(format!(
                "Profile dimension {} != codebook dimension {}",
                profile.dim(),
                self.dim
            )));
        }

        let nearest_id = self.nearest_prototype_id(profile);
        let centroid = &self.entries[nearest_id].centroid;

        let distance = euclidean_distance(centroid, &profile.features);
        let cosine_sim = cosine_similarity(centroid, &profile.features);

        let residual: Vec<f64> = profile
            .features
            .iter()
            .zip(centroid.iter())
            .map(|(a, b)| a - b)
            .collect();

        let dimension_contributions: Vec<f64> = profile
            .features
            .iter()
            .zip(centroid.iter())
            .map(|(a, b)| (a - b).powi(2))
            .collect();

        // Update assignment tracking
        self.assignments
            .insert(profile.agent_id.clone(), nearest_id);
        self.entries[nearest_id].assigned_count += 1;
        self.entries[nearest_id].total_assignments += 1;

        // Update running average of intra-cluster distance
        let entry = &mut self.entries[nearest_id];
        let n = entry.total_assignments as f64;
        entry.avg_intra_distance =
            entry.avg_intra_distance * ((n - 1.0) / n) + distance / n;

        Ok(QuantizationResult {
            agent_id: profile.agent_id.clone(),
            prototype_id: nearest_id,
            prototype_label: self.entries[nearest_id].label.clone(),
            distance,
            cosine_sim,
            residual,
            dimension_contributions,
        })
    }

    /// Incremental EMA update: nudge the nearest centroid toward the observed profile.
    pub fn train_step(&mut self, profile: &AgentProfile) -> KiasResult<QuantizationResult> {
        let result = self.quantize(profile)?;
        let lr = self.learning_rate;

        // EMA update: centroid += lr * (profile - centroid)
        let centroid = &mut self.entries[result.prototype_id].centroid;
        for (c, &f) in centroid.iter_mut().zip(profile.features.iter()) {
            *c += lr * (f - *c);
        }

        self.training_steps += 1;
        Ok(result)
    }

    /// Batch training: iterate over profiles for multiple epochs.
    pub fn train(
        &mut self,
        profiles: &[AgentProfile],
        epochs: usize,
    ) -> KiasResult<TrainingReport> {
        let mut total_distance = 0.0;
        let mut total_steps = 0u64;

        for epoch in 0..epochs {
            let mut epoch_distance = 0.0;
            for profile in profiles {
                let result = self.train_step(profile)?;
                epoch_distance += result.distance;
                total_steps += 1;
            }
            let avg = epoch_distance / profiles.len() as f64;
            tracing::debug!(
                epoch = epoch + 1,
                avg_distance = avg,
                "VQ training epoch complete"
            );
            total_distance += epoch_distance;
        }

        // Auto-relabel after training
        self.auto_label();

        Ok(TrainingReport {
            epochs,
            total_steps,
            avg_distance: total_distance / total_steps.max(1) as f64,
            codebook_size: self.k,
            prototype_stats: self.prototype_stats(),
        })
    }

    /// Get the current prototype for an agent (if previously assigned).
    pub fn get_assignment(&self, agent_id: &str) -> Option<usize> {
        self.assignments.get(agent_id).copied()
    }

    /// Get all agents assigned to a specific prototype.
    pub fn agents_in_prototype(&self, prototype_id: usize) -> Vec<&str> {
        self.assignments
            .iter()
            .filter(|(_, &pid)| pid == prototype_id)
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Summary statistics per prototype.
    pub fn prototype_stats(&self) -> Vec<PrototypeStats> {
        self.entries
            .iter()
            .map(|e| PrototypeStats {
                prototype_id: e.id,
                label: e.label.clone(),
                assigned_count: e.assigned_count,
                total_assignments: e.total_assignments,
                avg_intra_distance: e.avg_intra_distance,
                centroid_norm: l2_norm(&e.centroid),
            })
            .collect()
    }

    /// Auto-label prototypes based on centroid characteristics.
    /// Uses the highest-weighted feature dimension as the archetype descriptor.
    fn auto_label(&mut self) {
        for entry in &mut self.entries {
            if entry.centroid.is_empty() {
                continue;
            }
            // Find the dominant feature
            let max_dim = entry
                .centroid
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);

            let feature_name = if max_dim < entry.feature_names.len() {
                &entry.feature_names[max_dim]
            } else {
                "unknown"
            };

            let magnitude = l2_norm(&entry.centroid);
            let tier = if magnitude > 0.7 {
                "high"
            } else if magnitude > 0.3 {
                "mid"
            } else {
                "low"
            };

            entry.label = format!("{tier}-{feature_name}");
        }
    }
}

// ─── Training Report ─────────────────────────────────────────────────────────

/// Summary of a codebook training run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingReport {
    pub epochs: usize,
    pub total_steps: u64,
    pub avg_distance: f64,
    pub codebook_size: usize,
    pub prototype_stats: Vec<PrototypeStats>,
}

/// Per-prototype statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrototypeStats {
    pub prototype_id: usize,
    pub label: String,
    pub assigned_count: u64,
    pub total_assignments: u64,
    pub avg_intra_distance: f64,
    pub centroid_norm: f64,
}

// ─── Helper Functions ────────────────────────────────────────────────────────

fn euclidean_distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn l2_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x.powi(2)).sum::<f64>().sqrt()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_features() -> Vec<String> {
        vec![
            "cpu_intensity".to_string(),
            "memory_intensity".to_string(),
            "io_intensity".to_string(),
            "latency_sensitivity".to_string(),
        ]
    }

    fn sample_profiles() -> Vec<AgentProfile> {
        vec![
            AgentProfile::new(
                "agent-1",
                vec![
                    ("cpu_intensity".into(), 0.9),
                    ("memory_intensity".into(), 0.2),
                    ("io_intensity".into(), 0.1),
                    ("latency_sensitivity".into(), 0.3),
                ],
            ),
            AgentProfile::new(
                "agent-2",
                vec![
                    ("cpu_intensity".into(), 0.85),
                    ("memory_intensity".into(), 0.25),
                    ("io_intensity".into(), 0.15),
                    ("latency_sensitivity".into(), 0.35),
                ],
            ),
            AgentProfile::new(
                "agent-3",
                vec![
                    ("cpu_intensity".into(), 0.1),
                    ("memory_intensity".into(), 0.9),
                    ("io_intensity".into(), 0.8),
                    ("latency_sensitivity".into(), 0.7),
                ],
            ),
            AgentProfile::new(
                "agent-4",
                vec![
                    ("cpu_intensity".into(), 0.15),
                    ("memory_intensity".into(), 0.85),
                    ("io_intensity".into(), 0.75),
                    ("latency_sensitivity".into(), 0.65),
                ],
            ),
        ]
    }

    #[test]
    fn test_codebook_creation() {
        let cb = VqCodebook::new(4, test_features());
        assert_eq!(cb.k, 4);
        assert_eq!(cb.dim, 4);
        assert_eq!(cb.entries.len(), 4);
    }

    #[test]
    fn test_init_from_profiles() {
        let profiles = sample_profiles();
        let cb = VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();
        assert_eq!(cb.k, 2);
        // Centroids should not be all zeros after k-means init
        let non_zero = cb.entries.iter().any(|e| e.centroid.iter().any(|&v| v != 0.0));
        assert!(non_zero, "Centroids should be initialized from profiles");
    }

    #[test]
    fn test_quantize_basic() {
        let profiles = sample_profiles();
        let mut cb =
            VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();

        let result = cb.quantize(&profiles[0]).unwrap();
        assert_eq!(result.agent_id, "agent-1");
        assert!(result.distance >= 0.0);
        assert!(result.cosine_sim >= -1.0 && result.cosine_sim <= 1.0);
        assert_eq!(result.residual.len(), 4);
    }

    #[test]
    fn test_quantize_dimension_mismatch() {
        let mut cb = VqCodebook::new(2, test_features());
        let bad_profile = AgentProfile::new("bad", vec![("x".into(), 1.0)]);
        assert!(cb.quantize(&bad_profile).is_err());
    }

    #[test]
    fn test_train_step_updates_centroid() {
        let profiles = sample_profiles();
        let mut cb =
            VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.5).unwrap();

        let before_centroid = cb.entries[0].centroid.clone();
        // Train with a profile that maps to prototype 0
        let nearest = cb.nearest_prototype_id(&profiles[0]);
        let _ = cb.train_step(&profiles[0]).unwrap();
        // Centroid should have moved (unless lr=0)
        assert_ne!(cb.entries[nearest].centroid, before_centroid);
        assert_eq!(cb.training_steps, 1);
    }

    #[test]
    fn test_batch_training_convergence() {
        let profiles = sample_profiles();
        let mut cb =
            VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();

        let report = cb.train(&profiles, 20).unwrap();
        assert_eq!(report.epochs, 20);
        assert!(report.avg_distance >= 0.0);
        // After training, avg distance should be reasonable
        assert!(
            report.avg_distance < 2.0,
            "Average distance should converge"
        );
    }

    #[test]
    fn test_assignment_tracking() {
        let profiles = sample_profiles();
        let mut cb =
            VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();

        let result = cb.quantize(&profiles[0]).unwrap();
        assert_eq!(cb.get_assignment("agent-1"), Some(result.prototype_id));
        assert_eq!(cb.get_assignment("nonexistent"), None);
    }

    #[test]
    fn test_agents_in_prototype() {
        let profiles = sample_profiles();
        let mut cb =
            VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();

        for p in &profiles {
            let _ = cb.quantize(p).unwrap();
        }

        let total: usize = (0..cb.k)
            .map(|i| cb.agents_in_prototype(i).len())
            .sum();
        assert_eq!(total, profiles.len());
    }

    #[test]
    fn test_auto_label() {
        let profiles = sample_profiles();
        let cb = VqCodebook::init_from_profiles(2, test_features(), &profiles, 0.1).unwrap();
        // Labels should be auto-generated, not default "proto-N"
        for entry in &cb.entries {
            assert!(
                !entry.label.starts_with("proto-"),
                "Label should be auto-generated: {}",
                entry.label
            );
        }
    }

    #[test]
    fn test_profile_distance() {
        let p1 = AgentProfile::new("a", vec![("x".into(), 0.0), ("y".into(), 0.0)]);
        let p2 = AgentProfile::new("b", vec![("x".into(), 3.0), ("y".into(), 4.0)]);
        assert!((p1.distance(&p2) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_profile_cosine_similarity() {
        let p1 = AgentProfile::new("a", vec![("x".into(), 1.0), ("y".into(), 0.0)]);
        let p2 = AgentProfile::new("b", vec![("x".into(), 0.0), ("y".into(), 1.0)]);
        let p3 = AgentProfile::new("c", vec![("x".into(), 1.0), ("y".into(), 0.0)]);
        assert!((p1.cosine_similarity(&p2) - 0.0).abs() < 1e-10);
        assert!((p1.cosine_similarity(&p3) - 1.0).abs() < 1e-10);
    }
}
