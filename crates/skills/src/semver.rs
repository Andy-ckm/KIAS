//! Semantic versioning for the skill registry with backward compatibility guarantees.
//!
//! Implements SemVer 2.0.0 parsing, comparison, compatibility checking,
//! and version requirement matching (like Cargo's semver).
//!
//! ## Backward Compatibility Rules
//!
//! - **Patch** (x.y.Z): Bug fixes, fully compatible. No API changes.
//! - **Minor** (x.Y.0): New features, backward compatible. Old consumers still work.
//! - **Major** (X.0.0): Breaking changes. Old consumers may fail.
//!
//! The registry enforces these rules when resolving skill dependencies.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A parsed semantic version (SemVer 2.0.0).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// Optional pre-release label (e.g., "alpha.1", "beta.3").
    pub pre_release: Option<String>,
    /// Optional build metadata (e.g., "build.123"). Ignored in comparisons.
    pub build: Option<String>,
}

/// Error type for SemVer parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemVerError {
    InvalidFormat(String),
    InvalidNumber(String),
}

impl fmt::Display for SemVerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemVerError::InvalidFormat(s) => write!(f, "invalid semver format: '{}'", s),
            SemVerError::InvalidNumber(s) => write!(f, "invalid number in semver: '{}'", s),
        }
    }
}

impl std::error::Error for SemVerError {}

impl FromStr for SemVer {
    type Err = SemVerError;

    /// Parse a SemVer string like "1.2.3", "1.0.0-alpha.1", "2.0.0+build.123".
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(SemVerError::InvalidFormat(s.to_string()));
        }

        // Split off build metadata
        let (s, build) = match s.find('+') {
            Some(pos) => (&s[..pos], Some(s[pos + 1..].to_string())),
            None => (s, None),
        };

        // Split off pre-release
        let (s, pre_release) = match s.find('-') {
            Some(pos) => (&s[..pos], Some(s[pos + 1..].to_string())),
            None => (s, None),
        };

        // Parse major.minor.patch
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(SemVerError::InvalidFormat(s.to_string()));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| SemVerError::InvalidNumber(parts[0].to_string()))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| SemVerError::InvalidNumber(parts[1].to_string()))?;
        let patch = if parts.len() == 3 {
            parts[2]
                .parse::<u32>()
                .map_err(|_| SemVerError::InvalidNumber(parts[2].to_string()))?
        } else {
            0
        };

        Ok(SemVer {
            major,
            minor,
            patch,
            pre_release,
            build,
        })
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(ref pre) = self.pre_release {
            write!(f, "-{}", pre)?;
        }
        // Build metadata is NOT included in canonical display
        Ok(())
    }
}

impl SemVer {
    /// Create a new SemVer directly.
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build: None,
        }
    }

    /// Parse from string. Returns `None` on failure (convenience).
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }

    /// Bump major version (X+1.0.0). Signals breaking changes.
    pub fn bump_major(&self) -> Self {
        SemVer::new(self.major + 1, 0, 0)
    }

    /// Bump minor version (x.Y+1.0). Signals new backward-compatible features.
    pub fn bump_minor(&self) -> Self {
        SemVer::new(self.major, self.minor + 1, 0)
    }

    /// Bump patch version (x.y.Z+1). Signals backward-compatible bug fixes.
    pub fn bump_patch(&self) -> Self {
        SemVer::new(self.major, self.minor, self.patch + 1)
    }

    /// Check if this version is a pre-release.
    pub fn is_pre_release(&self) -> bool {
        self.pre_release.is_some()
    }

    /// Check if `self` is compatible with a consumer expecting `required`.
    ///
    /// Compatible means:
    /// - Same major version
    /// - `self.minor >= required.minor` (or same minor with self.patch >= required.patch)
    /// - Pre-releases only match exact versions
    pub fn is_compatible_with(&self, required: &SemVer) -> bool {
        // Pre-releases require exact match
        if self.is_pre_release() || required.is_pre_release() {
            return self == required;
        }

        // Same major, and self >= required within that major
        self.major == required.major
            && (self.minor > required.minor
                || (self.minor == required.minor && self.patch >= required.patch))
    }

    /// Determine the compatibility level between two versions.
    pub fn compatibility_level(&self, other: &SemVer) -> CompatibilityLevel {
        if self.major != other.major {
            CompatibilityLevel::Breaking
        } else if self.minor != other.minor {
            CompatibilityLevel::Compatible
        } else if self.patch != other.patch {
            CompatibilityLevel::PatchOnly
        } else {
            CompatibilityLevel::Identical
        }
    }

    /// Increment to the next version based on change type.
    pub fn next(&self, change: ChangeType) -> Self {
        match change {
            ChangeType::Patch => self.bump_patch(),
            ChangeType::Minor => self.bump_minor(),
            ChangeType::Major => self.bump_major(),
        }
    }
}

/// The type of change in a new version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// Backward-compatible bug fix (x.y.Z+1).
    Patch,
    /// Backward-compatible new feature (x.Y.0).
    Minor,
    /// Breaking change (X.0.0).
    Major,
}

/// Compatibility level between two versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompatibilityLevel {
    /// Same version.
    Identical,
    /// Same major, different patch only. Fully compatible.
    PatchOnly,
    /// Same major, different minor. Backward compatible (new features).
    Compatible,
    /// Different major. Breaking changes possible.
    Breaking,
}

// ── PartialOrd / Ord ──────────────────────────────────────────────

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
            .then(match (&self.pre_release, &other.pre_release) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater, // 1.0.0 > 1.0.0-alpha
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            })
    }
}

// ── Version Requirement ───────────────────────────────────────────

/// A version requirement expression for dependency resolution.
///
/// Supports:
/// - Exact: `"1.2.3"`
/// - Caret (compatible): `"^1.2.3"` or `"~1.2.3"` (same major, >= specified)
/// - Greater-than-or-equal: `">=1.2.0"`
/// - Range: `">=1.0, <2.0"`
/// - Wildcard: `"1.*"` or `"1.2.*"`
/// - Any: `"*"` or `""`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionReq {
    /// Parsed predicates. All must match (AND logic).
    predicates: Vec<Predicate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Predicate {
    op: Op,
    version: SemVer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Op {
    Exact,      // =1.2.3
    Gte,        // >=1.2.0
    Lt,         // <2.0.0
    Caret,      // ^1.2.3 (same major, >= specified)
    Compatible, // ~1.2.3 (same major.minor, >= specified)
}

impl VersionReq {
    /// Parse a version requirement string.
    pub fn parse(s: &str) -> Result<Self, SemVerError> {
        let s = s.trim();
        if s.is_empty() || s == "*" {
            return Ok(VersionReq { predicates: vec![] });
        }

        let mut predicates = Vec::new();
        for part in s.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            let (op, ver_str) = if let Some(rest) = part.strip_prefix(">=") {
                (Op::Gte, rest.trim())
            } else if let Some(rest) = part.strip_prefix("<") {
                (Op::Lt, rest.trim())
            } else if let Some(rest) = part.strip_prefix('^') {
                (Op::Caret, rest.trim())
            } else if let Some(rest) = part.strip_prefix('~') {
                (Op::Compatible, rest.trim())
            } else if let Some(rest) = part.strip_prefix('=') {
                (Op::Exact, rest.trim())
            } else {
                // Check for wildcard pattern like "1.*" or "1.2.*"
                if part.contains('*') {
                    // Handle as caret range
                    let normalized = part.replace('*', "0");
                    let version = SemVer::from_str(&normalized)?;
                    // Determine op based on wildcard position
                    let dots = part.split('.').count();
                    match dots {
                        2 => predicates.push(Predicate {
                            op: Op::Caret,
                            version: SemVer::new(version.major, 0, 0),
                        }),
                        3 => predicates.push(Predicate {
                            op: Op::Compatible,
                            version: SemVer::new(version.major, version.minor, 0),
                        }),
                        _ => predicates.push(Predicate {
                            op: Op::Caret,
                            version,
                        }),
                    }
                    continue;
                }
                (Op::Caret, part) // Default to caret (compatible) like Cargo
            };

            predicates.push(Predicate {
                op,
                version: SemVer::from_str(ver_str)?,
            });
        }

        Ok(VersionReq { predicates })
    }

    /// Check if a version satisfies this requirement.
    pub fn matches(&self, version: &SemVer) -> bool {
        if self.predicates.is_empty() {
            return true; // "*" or "" matches everything
        }

        self.predicates.iter().all(|p| match p.op {
            Op::Exact => version == &p.version,
            Op::Gte => version >= &p.version,
            Op::Lt => version < &p.version,
            Op::Caret => {
                // Same major, >= specified (pre-releases only match exact)
                if version.is_pre_release() || p.version.is_pre_release() {
                    version == &p.version
                } else {
                    version.major == p.version.major
                        && (version.minor > p.version.minor
                            || (version.minor == p.version.minor
                                && version.patch >= p.version.patch))
                }
            }
            Op::Compatible => {
                // Same major.minor, >= specified patch
                if version.is_pre_release() || p.version.is_pre_release() {
                    version == &p.version
                } else {
                    version.major == p.version.major
                        && version.minor == p.version.minor
                        && version.patch >= p.version.patch
                }
            }
        })
    }

    /// Check if this requirement is "any" (wildcard).
    pub fn is_any(&self) -> bool {
        self.predicates.is_empty()
    }
}

impl FromStr for VersionReq {
    type Err = SemVerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        VersionReq::parse(s)
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.predicates.is_empty() {
            return write!(f, "*");
        }
        for (i, p) in self.predicates.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            match p.op {
                Op::Exact => write!(f, "={}", p.version)?,
                Op::Gte => write!(f, ">={}", p.version)?,
                Op::Lt => write!(f, "<{}", p.version)?,
                Op::Caret => write!(f, "^{}", p.version)?,
                Op::Compatible => write!(f, "~{}", p.version)?,
            }
        }
        Ok(())
    }
}

// ── Versioned Skill Entry ─────────────────────────────────────────

/// A skill entry in the registry with full version metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedSkillEntry {
    /// Skill name (unique identifier).
    pub name: String,
    /// Current version.
    pub version: SemVer,
    /// All published versions (sorted, newest last).
    pub published_versions: Vec<SemVer>,
    /// Deprecation info for old versions.
    pub deprecated_versions: Vec<DeprecatedVersion>,
    /// Minimum version that is still supported (not deprecated).
    pub min_supported_version: Option<SemVer>,
    /// Migration paths between major versions.
    pub migration_paths: Vec<MigrationPath>,
}

/// A deprecated version with reason and sunset date.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprecatedVersion {
    pub version: SemVer,
    pub reason: String,
    /// When this version will stop working entirely.
    pub sunset_at: Option<String>,
}

/// A migration path from one major version to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPath {
    pub from_major: u32,
    pub to_major: u32,
    /// Human-readable migration guide.
    pub guide: String,
    /// Whether automatic migration is possible.
    pub auto_migratable: bool,
}

impl VersionedSkillEntry {
    /// Create a new versioned skill entry.
    pub fn new(name: impl Into<String>, version: SemVer) -> Self {
        Self {
            name: name.into(),
            version: version.clone(),
            published_versions: vec![version],
            deprecated_versions: vec![],
            min_supported_version: None,
            migration_paths: vec![],
        }
    }

    /// Publish a new version.
    pub fn publish(&mut self, version: SemVer) -> Result<(), String> {
        if version <= self.version {
            return Err(format!(
                "New version {} must be greater than current {}",
                version, self.version
            ));
        }
        self.version = version.clone();
        self.published_versions.push(version);
        Ok(())
    }

    /// Mark a version as deprecated.
    pub fn deprecate(
        &mut self,
        version: SemVer,
        reason: impl Into<String>,
        sunset_at: Option<String>,
    ) {
        self.deprecated_versions.push(DeprecatedVersion {
            version,
            reason: reason.into(),
            sunset_at,
        });
    }

    /// Check if a specific version is deprecated.
    pub fn is_deprecated(&self, version: &SemVer) -> bool {
        self.deprecated_versions
            .iter()
            .any(|d| &d.version == version)
    }

    /// Add a migration path.
    pub fn add_migration(
        &mut self,
        from_major: u32,
        to_major: u32,
        guide: impl Into<String>,
        auto_migratable: bool,
    ) {
        self.migration_paths.push(MigrationPath {
            from_major,
            to_major,
            guide: guide.into(),
            auto_migratable,
        });
    }

    /// Find the best matching version for a requirement.
    pub fn resolve(&self, req: &VersionReq) -> Option<&SemVer> {
        // Prefer newest non-deprecated version that matches
        self.published_versions
            .iter()
            .rev()
            .find(|v| req.matches(v) && !self.is_deprecated(v))
            .or_else(|| {
                // Fall back to deprecated if no non-deprecated matches
                self.published_versions
                    .iter()
                    .rev()
                    .find(|v| req.matches(v))
            })
    }

    /// Find migration path between two versions.
    pub fn find_migration(&self, from: &SemVer, to: &SemVer) -> Option<&MigrationPath> {
        self.migration_paths
            .iter()
            .find(|m| m.from_major == from.major && m.to_major == to.major)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SemVer parsing ─────────────────────────────────────────

    #[test]
    fn test_parse_basic() {
        let v: SemVer = "1.2.3".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.pre_release.is_none());
        assert!(v.build.is_none());
    }

    #[test]
    fn test_parse_two_part() {
        let v: SemVer = "1.2".parse().unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn test_parse_pre_release() {
        let v: SemVer = "1.0.0-alpha.1".parse().unwrap();
        assert_eq!(v.pre_release, Some("alpha.1".to_string()));
    }

    #[test]
    fn test_parse_build_metadata() {
        let v: SemVer = "1.0.0+build.123".parse().unwrap();
        assert_eq!(v.build, Some("build.123".to_string()));
    }

    #[test]
    fn test_parse_pre_release_and_build() {
        let v: SemVer = "1.0.0-beta.2+build.456".parse().unwrap();
        assert_eq!(v.pre_release, Some("beta.2".to_string()));
        assert_eq!(v.build, Some("build.456".to_string()));
    }

    #[test]
    fn test_parse_invalid() {
        assert!("".parse::<SemVer>().is_err());
        assert!("abc".parse::<SemVer>().is_err());
        assert!("1".parse::<SemVer>().is_err());
        assert!("1.2.3.4".parse::<SemVer>().is_err());
        assert!("a.b.c".parse::<SemVer>().is_err());
    }

    // ── SemVer display ─────────────────────────────────────────

    #[test]
    fn test_display_basic() {
        let v = SemVer::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_display_pre_release() {
        let v = SemVer {
            major: 1,
            minor: 0,
            patch: 0,
            pre_release: Some("alpha.1".to_string()),
            build: None,
        };
        assert_eq!(v.to_string(), "1.0.0-alpha.1");
    }

    #[test]
    fn test_display_omits_build() {
        let v = SemVer {
            major: 1,
            minor: 0,
            patch: 0,
            pre_release: None,
            build: Some("build.123".to_string()),
        };
        assert_eq!(v.to_string(), "1.0.0");
    }

    // ── SemVer comparison ──────────────────────────────────────

    #[test]
    fn test_ordering_major() {
        assert!(SemVer::new(2, 0, 0) > SemVer::new(1, 9, 9));
    }

    #[test]
    fn test_ordering_minor() {
        assert!(SemVer::new(1, 2, 0) > SemVer::new(1, 1, 9));
    }

    #[test]
    fn test_ordering_patch() {
        assert!(SemVer::new(1, 2, 3) > SemVer::new(1, 2, 2));
    }

    #[test]
    fn test_ordering_pre_release_less_than_release() {
        assert!(SemVer::new(1, 0, 0) > SemVer::parse("1.0.0-alpha").unwrap());
    }

    #[test]
    fn test_ordering_equal() {
        assert_eq!(SemVer::new(1, 2, 3), SemVer::new(1, 2, 3));
    }

    // ── Bump ───────────────────────────────────────────────────

    #[test]
    fn test_bump_major() {
        assert_eq!(SemVer::new(1, 2, 3).bump_major(), SemVer::new(2, 0, 0));
    }

    #[test]
    fn test_bump_minor() {
        assert_eq!(SemVer::new(1, 2, 3).bump_minor(), SemVer::new(1, 3, 0));
    }

    #[test]
    fn test_bump_patch() {
        assert_eq!(SemVer::new(1, 2, 3).bump_patch(), SemVer::new(1, 2, 4));
    }

    // ── Compatibility ──────────────────────────────────────────

    #[test]
    fn test_compatible_same_version() {
        assert!(SemVer::new(1, 2, 3).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_compatible_higher_patch() {
        assert!(SemVer::new(1, 2, 5).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_compatible_higher_minor() {
        assert!(SemVer::new(1, 3, 0).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_not_compatible_different_major() {
        assert!(!SemVer::new(2, 0, 0).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_not_compatible_lower_minor() {
        assert!(!SemVer::new(1, 1, 0).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_not_compatible_lower_patch() {
        assert!(!SemVer::new(1, 2, 2).is_compatible_with(&SemVer::new(1, 2, 3)));
    }

    #[test]
    fn test_pre_release_exact_match() {
        let alpha = SemVer::parse("1.0.0-alpha").unwrap();
        let alpha2 = SemVer::parse("1.0.0-alpha").unwrap();
        assert!(alpha.is_compatible_with(&alpha2));
    }

    #[test]
    fn test_pre_release_no_match_different() {
        let alpha = SemVer::parse("1.0.0-alpha").unwrap();
        let beta = SemVer::parse("1.0.0-beta").unwrap();
        assert!(!alpha.is_compatible_with(&beta));
    }

    // ── Compatibility level ────────────────────────────────────

    #[test]
    fn test_level_identical() {
        assert_eq!(
            SemVer::new(1, 2, 3).compatibility_level(&SemVer::new(1, 2, 3)),
            CompatibilityLevel::Identical
        );
    }

    #[test]
    fn test_level_patch_only() {
        assert_eq!(
            SemVer::new(1, 2, 4).compatibility_level(&SemVer::new(1, 2, 3)),
            CompatibilityLevel::PatchOnly
        );
    }

    #[test]
    fn test_level_compatible() {
        assert_eq!(
            SemVer::new(1, 3, 0).compatibility_level(&SemVer::new(1, 2, 3)),
            CompatibilityLevel::Compatible
        );
    }

    #[test]
    fn test_level_breaking() {
        assert_eq!(
            SemVer::new(2, 0, 0).compatibility_level(&SemVer::new(1, 2, 3)),
            CompatibilityLevel::Breaking
        );
    }

    // ── Version Requirement ────────────────────────────────────

    #[test]
    fn test_req_any() {
        let req = VersionReq::parse("*").unwrap();
        assert!(req.matches(&SemVer::new(0, 0, 1)));
        assert!(req.matches(&SemVer::new(99, 99, 99)));
    }

    #[test]
    fn test_req_empty() {
        let req = VersionReq::parse("").unwrap();
        assert!(req.matches(&SemVer::new(1, 0, 0)));
    }

    #[test]
    fn test_req_caret() {
        let req = VersionReq::parse("^1.2.3").unwrap();
        assert!(req.matches(&SemVer::new(1, 2, 3)));
        assert!(req.matches(&SemVer::new(1, 2, 5)));
        assert!(req.matches(&SemVer::new(1, 5, 0)));
        assert!(!req.matches(&SemVer::new(2, 0, 0)));
        assert!(!req.matches(&SemVer::new(1, 2, 2)));
    }

    #[test]
    fn test_req_tilde() {
        let req = VersionReq::parse("~1.2.3").unwrap();
        assert!(req.matches(&SemVer::new(1, 2, 3)));
        assert!(req.matches(&SemVer::new(1, 2, 5)));
        assert!(!req.matches(&SemVer::new(1, 3, 0)));
        assert!(!req.matches(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn test_req_gte() {
        let req = VersionReq::parse(">=1.2.0").unwrap();
        assert!(req.matches(&SemVer::new(1, 2, 0)));
        assert!(req.matches(&SemVer::new(1, 5, 0)));
        assert!(req.matches(&SemVer::new(2, 0, 0)));
        assert!(!req.matches(&SemVer::new(1, 1, 9)));
    }

    #[test]
    fn test_req_range() {
        let req = VersionReq::parse(">=1.0, <2.0").unwrap();
        assert!(req.matches(&SemVer::new(1, 0, 0)));
        assert!(req.matches(&SemVer::new(1, 5, 0)));
        assert!(!req.matches(&SemVer::new(2, 0, 0)));
        assert!(!req.matches(&SemVer::new(0, 9, 0)));
    }

    #[test]
    fn test_req_exact() {
        let req = VersionReq::parse("=1.2.3").unwrap();
        assert!(req.matches(&SemVer::new(1, 2, 3)));
        assert!(!req.matches(&SemVer::new(1, 2, 4)));
    }

    #[test]
    fn test_req_default_caret() {
        // Without operator, defaults to caret
        let req = VersionReq::parse("1.2.3").unwrap();
        assert!(req.matches(&SemVer::new(1, 2, 3)));
        assert!(req.matches(&SemVer::new(1, 5, 0)));
        assert!(!req.matches(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn test_req_display() {
        let req = VersionReq::parse("^1.2.3").unwrap();
        assert_eq!(req.to_string(), "^1.2.3");
    }

    // ── Versioned Skill Entry ──────────────────────────────────

    #[test]
    fn test_entry_publish() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(1, 0, 0));
        entry.publish(SemVer::new(1, 1, 0)).unwrap();
        entry.publish(SemVer::new(1, 2, 0)).unwrap();
        assert_eq!(entry.version, SemVer::new(1, 2, 0));
        assert_eq!(entry.published_versions.len(), 3);
    }

    #[test]
    fn test_entry_publish_rejects_old() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(1, 0, 0));
        assert!(entry.publish(SemVer::new(0, 9, 0)).is_err());
    }

    #[test]
    fn test_entry_deprecate() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(2, 0, 0));
        entry.publish(SemVer::new(1, 0, 0)).ok(); // won't work, lower version
        entry.deprecate(
            SemVer::new(1, 0, 0),
            "reached EOL",
            Some("2026-12-31".to_string()),
        );
        assert!(entry.is_deprecated(&SemVer::new(1, 0, 0)));
        assert!(!entry.is_deprecated(&SemVer::new(2, 0, 0)));
    }

    #[test]
    fn test_entry_resolve_prefers_non_deprecated() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(1, 0, 0));
        entry.publish(SemVer::new(1, 1, 0)).unwrap();
        entry.publish(SemVer::new(2, 0, 0)).unwrap();
        entry.deprecate(SemVer::new(1, 0, 0), "old", None);

        let req = VersionReq::parse("^1.0").unwrap();
        let resolved = entry.resolve(&req).unwrap();
        // Should resolve to 1.1.0 (not deprecated 1.0.0)
        assert_eq!(*resolved, SemVer::new(1, 1, 0));
    }

    #[test]
    fn test_entry_resolve_falls_back_to_deprecated() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(1, 0, 0));
        entry.deprecate(SemVer::new(1, 0, 0), "old", None);

        let req = VersionReq::parse("=1.0.0").unwrap();
        let resolved = entry.resolve(&req).unwrap();
        // Falls back to deprecated since it's the only match
        assert_eq!(*resolved, SemVer::new(1, 0, 0));
    }

    #[test]
    fn test_entry_migration() {
        let mut entry = VersionedSkillEntry::new("my-skill", SemVer::new(2, 0, 0));
        entry.add_migration(1, 2, "See migration guide", true);

        let path = entry.find_migration(&SemVer::new(1, 0, 0), &SemVer::new(2, 0, 0));
        assert!(path.is_some());
        assert!(path.unwrap().auto_migratable);
    }

    // ── next() ─────────────────────────────────────────────────

    #[test]
    fn test_next_change_types() {
        let v = SemVer::new(1, 2, 3);
        assert_eq!(v.next(ChangeType::Patch), SemVer::new(1, 2, 4));
        assert_eq!(v.next(ChangeType::Minor), SemVer::new(1, 3, 0));
        assert_eq!(v.next(ChangeType::Major), SemVer::new(2, 0, 0));
    }
}
