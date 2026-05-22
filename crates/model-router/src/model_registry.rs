
//! Model Registry
//!
//! A central registry that stores, retrieves, and selects AI/ML models.
//! Each model has a unique identifier, a semantic version, a set of
//! capabilities, and a cost model.  The registry supports multiple
//! versions of the same model, default‑version selection, and
//! capability‑based lookup.

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// Crate‑wide error types (assumed to exist in the `error` module)
// ---------------------------------------------------------------------------
// The actual definitions are not required here; we simply import them.
pub use crate::error::{RouterError, RouterResult};

// ---------------------------------------------------------------------------
// Local registry errors that will be translated into `RouterError`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The requested model identifier does not exist in the registry.
    ModelNotFound(String),
    /// A model with the same identifier already exists at the given version.
    DuplicateVersion { model_id: String, version: Version },
    /// No default version has been set for the requested model.
    NoDefaultVersion(String),
    /// The model does not expose the required capability.
    CapabilityMismatch,
    /// The supplied cost values are invalid (e.g. negative).
    InvalidCost,
    /// Tried to remove a version that does not exist.
    VersionNotFound { model_id: String, version: Version },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::ModelNotFound(id) => {
                write!(f, "Model with id '{}' not found in registry", id)
            }
            RegistryError::DuplicateVersion { model_id, version } => {
                write!(
                    f,
                    "Model '{}' already has version {} registered",
                    model_id, version
                )
            }
            RegistryError::NoDefaultVersion(id) => {
                write!(f, "No default version set for model '{}'", id)
            }
            RegistryError::CapabilityMismatch => {
                write!(f, "Model does not satisfy the required capability")
            }
            RegistryError::InvalidCost => {
                write!(f, "Cost values must be non‑negative")
            }
            RegistryError::VersionNotFound { model_id, version } => {
                write!(
                    f,
                    "Model '{}' has no version {} registered",
                    model_id, version
                )
            }
        }
    }
}

// Convert our local error into the public `RouterError`.
impl From<RegistryError> for RouterError {
    fn from(err: RegistryError) -> RouterError {
        RouterError::new(err.to_string())
    }
}

// ---------------------------------------------------------------------------
// Basic types used throughout the registry
// ---------------------------------------------------------------------------

/// A model identifier – in practice this could be a ULID, UUID, or a
/// descriptive name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(pub String);

impl ModelId {
    pub fn new(s: impl Into<String>) -> Self {
        ModelId(s.into())
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Semantic version (major.minor.patch) with total ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parse a version from a string like `"1.2.3"`.
    pub fn parse(s: &str) -> RouterResult<Version> {
        let mut parts = s.splitn(3, '.');
        let major = parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| RouterError::new("Invalid major segment".to_string()))?;
        let minor = parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| RouterError::new("Invalid minor segment".to_string()))?;
        let patch = parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| RouterError::new("Invalid patch segment".to_string()))?;
        Ok(Version { major, minor, patch })
    }

    /// Convenience constructor.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Version { major, minor, patch }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Capabilities that a model may expose.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Pure text generation (e.g. LLM).
    TextGeneration,
    /// Produce dense text embeddings.
    TextEmbedding,
    /// Image classification.
    ImageClassification,
    /// Object detection in images.
    ObjectDetection,
    /// Speech‑to‑text.
    SpeechRecognition,
    /// Machine translation.
    Translation,
    /// Custom capability identified by a tag.
    Custom(String),
}

impl Capability {
    /// Human‑readable name for logging / UI.
    pub fn label(&self) -> &str {
        match self {
            Capability::TextGeneration => "TextGeneration",
            Capability::TextEmbedding => "TextEmbedding",
            Capability::ImageClassification => "ImageClassification",
            Capability::ObjectDetection => "ObjectDetection",
            Capability::SpeechRecognition => "SpeechRecognition",
            Capability::Translation => "Translation",
            Capability::Custom(s) => s,
        }
    }
}

/// Cost model for using a model.  Costs are expressed in arbitrary units
/// but must be non‑negative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    /// Fixed cost per request.
    pub fixed: f64,
    /// Variable cost per token (or per image, etc.).
    pub per_token: f64,
}

impl Cost {
    /// Create a new `Cost`.  Returns `Err` if any component is negative.
    pub fn new(fixed: f64, per_token: f64) -> RouterResult<Cost> {
        if fixed < 0.0 || per_token < 0.0 {
            return Err(RegistryError::InvalidCost.into());
        }
        Ok(Cost { fixed, per_token })
    }

    /// Compute the total cost for a given number of tokens.
    pub fn total(&self, tokens: u64) -> f64 {
        self.fixed + self.per_token * tokens as f64
    }
}

/// Metadata associated with a model (e.g. author, license, endpoint).
pub type ModelMetadata = std::collections::HashMap<String, String>;

/// The core model description stored in the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Unique identifier for the model family.
    pub id: ModelId,
    /// Human‑readable name (may be the same as `id.0`).
    pub name: String,
    /// Semantic version of this model.
    pub version: Version,
    /// Set of capabilities offered by this model.
    pub capabilities: Vec<Capability>,
    /// Cost model for this model.
    pub cost: Cost,
    /// Arbitrary key/value metadata.
    pub metadata: ModelMetadata,
}

impl Model {
    /// Convenience constructor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: Version,
        capabilities: Vec<Capability>,
        cost: Cost,
        metadata: ModelMetadata,
    ) -> Self {
        Model {
            id: ModelId(id.into()),
            name: name.into(),
            version,
            capabilities,
            cost,
            metadata,
        }
    }

    /// Check whether the model provides a specific capability.
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    /// Compute total cost for a given token count.
    pub fn total_cost(&self, tokens: u64) -> f64 {
        self.cost.total(tokens)
    }
}

// ---------------------------------------------------------------------------
// Model Registry
// ---------------------------------------------------------------------------

/// The central registry that keeps track of all known models.
pub struct ModelRegistry {
    /// Map from model id → all registered versions (sorted by version descending).
    models: std::collections::HashMap<ModelId, Vec<Model>>,
    /// Default version for each model id (if any).
    default_versions: std::collections::HashMap<ModelId, Version>,
    /// Internal marker for generic associated types (not used at runtime).
    _marker: PhantomData<DefaultHasher>,
}

impl ModelRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        ModelRegistry {
            models: std::collections::HashMap::new(),
            default_versions: std::collections::HashMap::new(),
            _marker: PhantomData,
        }
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a new model version.
    ///
    /// * If the model identifier does not yet exist, a new entry is created.
    /// * If the exact version already exists for that identifier, an error
    ///   `DuplicateVersion` is returned.
    /// * If this is the first version for the identifier, it becomes the default.
    pub fn register(&mut self, model: Model) -> RouterResult<()> {
        let id = model.id.clone();
        let version = model.version;

        // Ensure version is not already present.
        let versions = self.models.entry(id.clone()).or_insert_with(Vec::new);
        if versions.iter().any(|m| m.version == version) {
            return Err(RegistryError::DuplicateVersion {
                model_id: id.0,
                version,
            }
            .into());
        }

        // Insert and keep the vector sorted descending.
        versions.push(model);
        versions.sort_by(|a, b| b.version.cmp(&a.version));

        // First version for this id becomes the default.
        if versions.len() == 1 {
            self.default_versions.insert(id, version);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Retrieval
    // -----------------------------------------------------------------------

    /// Return the latest (i.e. highest semver) version of a model.
    pub fn get_latest(&self, id: &ModelId) -> RouterResult<&Model> {
        self.models
            .get(id)
            .and_then(|v| v.first()) // already sorted descending
            .ok_or_else(|| RegistryError::ModelNotFound(id.0.clone()).into())
    }

    /// Return a specific version of a model.
    pub fn get_by_version(&self, id: &ModelId, version: &Version) -> RouterResult<&Model> {
        self.models
            .get(id)
            .and_then(|v| v.iter().find(|m| &m.version == version))
            .ok_or_else(|| {
                if self.models.contains_key(id) {
                    RegistryError::VersionNotFound {
                        model_id: id.0.clone(),
                        version: *version,
                    }
                    .into()
                } else {
                    RegistryError::ModelNotFound(id.0.clone()).into()
                }
            })
    }

    /// Return the default version for a model.
    pub fn get_default(&self, id: &ModelId) -> RouterResult<&Model> {
        let version = self
            .default_versions
            .get(id)
            .ok_or_else(|| RegistryError::NoDefaultVersion(id.0.clone()).into())?;
        self.get_by_version(id, version)
    }

    // -----------------------------------------------------------------------
    // Version management
    // -----------------------------------------------------------------------

    /// Set the default version for an already‑registered model.
    pub fn set_default_version(&mut self, id: &ModelId, version: Version) -> RouterResult<()> {
        // Verify the version exists.
        let versions = self
            .models
            .get(id)
            .ok_or_else(|| RegistryError::ModelNotFound(id.0.clone()))?;
        if !versions.iter().any(|m| m.version == version) {
            return Err(RegistryError::VersionNotFound {
                model_id: id.0.clone(),
                version,
            }
            .into());
        }
        self.default_versions.insert(id.clone(), version);
        Ok(())
    }

    /// Return all known versions for a model (sorted descending).
    pub fn list_versions(&self, id: &ModelId) -> RouterResult<Vec<Version>> {
        let versions = self
            .models
            .get(id)
            .ok_or_else(|| RegistryError::ModelNotFound(id.0.clone()))?;
        Ok(versions.iter().map(|m| m.version).collect())
    }

    // -----------------------------------------------------------------------
    // Deletion
    // -----------------------------------------------------------------------

    /// Remove a specific version of a model.
    ///
    /// *If* `version` is `None`, **all** versions of that model are removed.
    ///
    /// Returns the removed model(s) (wrapped in `Option`) on success.
    pub fn remove(
        &mut self,
        id: &ModelId,
        version: Option<&Version>,
    ) -> RouterResult<Option<Vec<Model>>> {
        let removed = match version {
            Some(v) => {
                // Try to remove a single version.
                let versions = self
                    .models
                    .get_mut(id)
                    .ok_or_else(|| RegistryError::ModelNotFound(id.0.clone()))?;

                let pos = versions
                    .iter()
                    .position(|m| &m.version == v)
                    .ok_or_else(|| RegistryError::VersionNotFound {
                        model_id: id.0.clone(),
                        version: *v,
                    })?;

                let removed_model = versions.remove(pos);
                // If that was the last version, drop the whole entry.
                if versions.is_empty() {
                    self.models.remove(id);
                    self.default_versions.remove(id);
                } else if self.default_versions.get(id) == Some(v) {
                    // Default version changed – pick the newest remaining.
                    let newest = versions.first().map(|m| m.version);
                    if let Some(nv) = newest {
                        self.default_versions.insert(id.clone(), nv);
                    }
                }
                Some(vec![removed_model])
            }
            None => {
                // Remove everything for this id.
                self.models.remove(id);
                self.default_versions.remove(id);
                None // caller can check if they want the old data
            }
        };
        Ok(removed)
    }

    // -----------------------------------------------------------------------
    // Query helpers
    // -----------------------------------------------------------------------

    /// Return a list of all model identifiers currently registered.
    pub fn list_models(&self) -> Vec<ModelId> {
        self.models.keys().cloned().collect()
    }

    /// Find all models that expose a given capability.
    pub fn find_by_capability(&self, cap: Capability) -> Vec<&Model> {
        self.models
            .values()
            .flatten()
            .filter(|m| m.has_capability(&cap))
            .collect()
    }

    /// Choose the cheapest model that satisfies the given capability.
    ///
    /// If `token_count` is supplied, the total cost (fixed + per_token*tokens)
    /// is used for comparison; otherwise only the fixed cost is considered.
    ///
    /// Returns `Ok(None)` if no model provides the requested capability.
    pub fn select_model(
        &self,
        cap: Capability,
        token_count: Option<u64>,
    ) -> RouterResult<Option<&Model>> {
        let candidates = self.find_by_capability(cap);
        if candidates.is_empty() {
            return Ok(None);
        }

        let best = candidates
            .into_iter()
            .min_by(|a, b| {
                let cost_a = a.total_cost(token_count.unwrap_or(0));
                let cost_b = b.total_cost(token_count.unwrap_or(0));
                cost_a
                    .partial_cmp(&cost_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap(); // safe because we checked non‑empty

        Ok(Some(best))
    }

    /// Return the number of registered model entries (versions counted individually).
    pub fn len(&self) -> usize {
        self.models.values().map(Vec::len).sum()
    }

    /// Returns `true` if the registry contains no models.
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Default implementation for `ModelRegistry`
// ---------------------------------------------------------------------------

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a simple version.
    fn v(major: u32, minor: u32, patch: u32) -> Version {
        Version::new(major, minor, patch)
    }

    // Helper: create a model with a given id, version, capability and cost.
    fn make_model(
        id: &str,
        major: u32,
        minor: u32,
        patch: u32,
        caps: Vec<Capability>,
        fixed: f64,
        per_token: f64,
    ) -> Model {
        Model::new(
            id,
            id,
            v(major, minor, patch),
            caps,
            Cost::new(fixed, per_token).expect("Cost should be valid"),
            std::collections::HashMap::new(),
        )
    }

    // -----------------------------------------------------------------------
    // Test 1 – successful registration and retrieval
    // -----------------------------------------------------------------------
    #[test]
    fn test_register_model_success() {
        let mut registry = ModelRegistry::new();
        let model = make_model("gpt-4", 1, 0, 0, vec![Capability::TextGeneration], 0.0, 0.001);

        registry.register(model.clone()).expect("registration should succeed");

        let found = registry.get_latest(&ModelId::new("gpt-4")).expect("model should be found");
        assert_eq!(found.id.0, "gpt-4");
        assert_eq!(found.version, v(1, 0, 0));
        assert!(found.has_capability(&Capability::TextGeneration));
    }

    // -----------------------------------------------------------------------
    // Test 2 – duplicate version returns error
    // -----------------------------------------------------------------------
    #[test]
    fn test_register_duplicate_version_error() {
        let mut registry = ModelRegistry::new();
        let model = make_model("gpt-4", 1, 0, 0, vec![Capability::TextGeneration], 0.0, 0.001);
        registry.register(model.clone()).expect("first registration should succeed");

        let result = registry.register(model);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // We expect a `RouterError` that wraps `RegistryError::DuplicateVersion`.
        assert!(
            format!("{}", err).contains("already has version"),
            "error message should mention duplicate version"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3 – get_latest returns highest version
    // -----------------------------------------------------------------------
    #[test]
    fn test_get_latest_version() {
        let mut registry = ModelRegistry::new();

        // Register multiple versions.
        registry
            .register(make_model(
                "llama2",
                0,
                9,
                0,
                vec![Capability::TextGeneration],
                0.0,
                0.0005,
            ))
            .expect("valid registration");
        registry
            .register(make_model(
                "llama2",
                1,
                0,
                0,
                vec![Capability::TextGeneration],
                0.0,
                0.001,
            ))
            .expect("valid registration");
        registry
            .register(make_model(
                "llama2",
                1,
                0,
                1,
                vec![Capability::TextGeneration],
                0.0,
                0.0012,
            ))
            .expect("valid registration");

        let latest = registry.get_latest(&ModelId::new("llama2")).expect("should exist");
        assert_eq!(latest.version, v(1, 0, 1));
    }

    // -----------------------------------------------------------------------
    // Test 4 – get_by_version
    // -----------------------------------------------------------------------
    #[test]
    fn test_get_by_version() {
        let mut registry = ModelRegistry::new();

        registry
            .register(make_model(
                "clip",
                0,
                1,
                0,
                vec![Capability::ImageClassification],
                0.1,
                0.0,
            ))
            .expect("valid registration");
        registry
            .register(make_model(
                "clip",
                0,
                2,
                0,
                vec![Capability::ImageClassification, Capability::TextEmbedding],
                0.15,
                0.0,
            ))
            .expect("valid registration");

        let v1 = registry.get_by_version(&ModelId::new("clip"), &v(0, 1, 0));
        assert!(v1.is_ok());
        assert_eq!(v1.unwrap().version, v(0, 1, 0));

        let missing = registry.get_by_version(&ModelId::new("clip"), &v(2, 0, 0));
        assert!(missing.is_err());
    }

    // -----------------------------------------------------------------------
    // Test 5 – remove model version(s)
    // -----------------------------------------------------------------------
    #[test]
    fn test_remove_model() {
        let mut registry = ModelRegistry::new();

        registry
            .register(make_model(
                "t5",
                1,
                0,
                0,
                vec![Capability::Translation],
                0.05,
                0.0,
            ))
            .expect("valid registration");
        registry
            .register(make_model(
                "t5",
                1,
                1,
                0,
                vec![Capability::Translation, Capability::TextGeneration],
                0.06,
                0.0,
            ))
            .expect("valid registration");

        // Remove specific version.
        let removed = registry
            .remove(&ModelId::new("t5"), Some(&v(1, 0, 0)))
            .expect("remove should succeed");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().len(), 1);

        // The remaining version should still be accessible.
        let latest = registry.get_latest(&ModelId::new("t5")).expect("should still exist");
        assert_eq!(latest.version, v(1, 1, 0));

        // Remove all remaining versions.
        let removed_all = registry.remove(&ModelId::new("t5"), None).expect("should succeed");
        assert!(removed_all.is_none());
        assert!(registry.get_latest(&ModelId::new("t5")).is_err());
    }

    // -----------------------------------------------------------------------
    // Test 6 – find by capability and select cheapest
    // -----------------------------------------------------------------------
    #[test]
    fn test_find_by_capability_and_select_cheapest() {
        let mut registry = ModelRegistry::new();

        // Register three models with varying costs.
        registry
            .register(make_model(
                "fast-gpt",
                1,
                0,
                0,
                vec![Capability::TextGeneration],
                1.0,
                0.002,
            ))
            .expect("valid");
        registry
            .register(make_model(
                "cheap-gpt",
                1,
                0,
                0,
                vec![Capability::TextGeneration],
                0.5,
                0.001,
            ))
            .expect("valid");
        registry
            .register(make_model(
                "premium-gpt",
                2,
                0,
                0,
                vec![Capability::TextGeneration],
                0.0,
                0.005,
            ))
            .expect("valid");

        let candidates = registry.find_by_capability(Capability::TextGeneration);
        assert_eq!(candidates.len(), 3, "all three models should match");

        //