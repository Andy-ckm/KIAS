//! Supply Chain Security Module
//!
//! Implements SBOM generation, dependency signatures, and vulnerability baselines

use crate::error::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// SBOM entry for a single dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomEntry {
    pub name: String,
    pub version: String,
    pub license: String,
    pub source: String,
    pub transitive: bool,
    pub hash: String,
}

/// Software Bill of Materials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sbom {
    pub entries: Vec<SbomEntry>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub tool_version: String,
    pub format_version: String,
}

impl Sbom {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            generated_at: chrono::Utc::now(),
            tool_version: "1.0.0".to_string(),
            format_version: "1.0".to_string(),
        }
    }

    pub fn add_entry(&mut self, entry: SbomEntry) {
        self.entries.push(entry);
    }

    pub fn total_dependencies(&self) -> usize {
        self.entries.len()
    }

    pub fn direct_dependencies(&self) -> usize {
        self.entries.iter().filter(|e| !e.transitive).count()
    }

    pub fn transitive_dependencies(&self) -> usize {
        self.entries.iter().filter(|e| e.transitive).count()
    }

    pub fn unique_licenses(&self) -> Vec<String> {
        let mut licenses: Vec<_> = self.entries.iter().map(|e| e.license.clone()).collect();
        licenses.sort();
        licenses.dedup();
        licenses
    }
}

impl Default for Sbom {
    fn default() -> Self {
        Self::new()
    }
}

/// Dependency signature for verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencySignature {
    pub dependency_name: String,
    pub version: String,
    pub content_hash: String,
    pub signature: String,
    pub signed_at: chrono::DateTime<chrono::Utc>,
    pub signer: String,
}

impl DependencySignature {
    pub fn new(name: &str, version: &str, content: &[u8], signer: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let content_hash = format!("{:x}", hasher.finalize());
        
        Self {
            dependency_name: name.to_string(),
            version: version.to_string(),
            content_hash: content_hash.clone(),
            signature: format!("sig_{}", &content_hash[..16]),
            signed_at: chrono::Utc::now(),
            signer: signer.to_string(),
        }
    }

    pub fn verify(&self, content: &[u8]) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(content);
        let computed = format!("{:x}", hasher.finalize());
        computed == self.content_hash
    }
}

/// Vulnerability severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl Severity {
    pub fn from_score(score: f64) -> Self {
        if score >= 9.0 {
            Severity::Critical
        } else if score >= 7.0 {
            Severity::High
        } else if score >= 4.0 {
            Severity::Medium
        } else if score >= 0.1 {
            Severity::Low
        } else {
            Severity::Info
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "CRITICAL",
            Severity::High => "HIGH",
            Severity::Medium => "MEDIUM",
            Severity::Low => "LOW",
            Severity::Info => "INFO",
        }
    }
}

/// Known vulnerability entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub cve_id: String,
    pub package: String,
    pub affected_versions: String,
    pub severity: Severity,
    pub cvss_score: f64,
    pub description: String,
    pub recommendation: String,
    pub published_at: chrono::DateTime<chrono::Utc>,
}

/// Vulnerability scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityScanResult {
    pub package_name: String,
    pub version: String,
    pub vulnerabilities: Vec<Vulnerability>,
    pub scan_time: chrono::DateTime<chrono::Utc>,
    pub total_vulnerabilities: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
}

impl VulnerabilityScanResult {
    pub fn new(package_name: &str, version: &str) -> Self {
        Self {
            package_name: package_name.to_string(),
            version: version.to_string(),
            vulnerabilities: Vec::new(),
            scan_time: chrono::Utc::now(),
            total_vulnerabilities: 0,
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
        }
    }

    pub fn add_vulnerability(&mut self, vuln: Vulnerability) {
        match vuln.severity {
            Severity::Critical => self.critical_count += 1,
            Severity::High => self.high_count += 1,
            Severity::Medium => self.medium_count += 1,
            Severity::Low => self.low_count += 1,
            Severity::Info => {}
        }
        self.total_vulnerabilities += 1;
        self.vulnerabilities.push(vuln);
    }

    pub fn has_critical(&self) -> bool {
        self.critical_count > 0
    }
}

/// Vulnerability baseline for comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityBaseline {
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub vulnerabilities: Vec<Vulnerability>,
    pub max_acceptable_severity: Severity,
}

impl VulnerabilityBaseline {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: String::new(),
            created_at: chrono::Utc::now(),
            vulnerabilities: Vec::new(),
            max_acceptable_severity: Severity::Medium,
        }
    }

    pub fn passes_baseline(&self, scan_result: &VulnerabilityScanResult) -> bool {
        for vuln in &scan_result.vulnerabilities {
            if vuln.severity == Severity::Critical {
                return false;
            }
            if vuln.severity >= self.max_acceptable_severity && vuln.severity != Severity::Critical {
                if vuln.severity == self.max_acceptable_severity {
                    return false;
                }
            }
        }
        true
    }
}

/// Supply chain security manager
pub struct SupplyChainSecurity {
    sbom: Sbom,
    signatures: HashMap<String, DependencySignature>,
    vulnerability_baselines: HashMap<String, VulnerabilityBaseline>,
    known_vulnerabilities: Vec<Vulnerability>,
}

impl Default for SupplyChainSecurity {
    fn default() -> Self {
        Self::new()
    }
}

impl SupplyChainSecurity {
    pub fn new() -> Self {
        let mut security = Self {
            sbom: Sbom::new(),
            signatures: HashMap::new(),
            vulnerability_baselines: HashMap::new(),
            known_vulnerabilities: Vec::new(),
        };
        security.load_known_vulnerabilities();
        security
    }

    fn load_known_vulnerabilities(&mut self) {
        self.known_vulnerabilities.push(Vulnerability {
            cve_id: "CVE-2024-1234".to_string(),
            package: "log4j".to_string(),
            affected_versions: "< 2.17.0".to_string(),
            severity: Severity::Critical,
            cvss_score: 10.0,
            description: "Remote code execution vulnerability".to_string(),
            recommendation: "Upgrade to 2.17.0 or later".to_string(),
            published_at: chrono::Utc::now(),
        });
        self.known_vulnerabilities.push(Vulnerability {
            cve_id: "CVE-2024-5678".to_string(),
            package: "openssl".to_string(),
            affected_versions: "< 1.1.1t".to_string(),
            severity: Severity::High,
            cvss_score: 7.5,
            description: "Memory corruption vulnerability".to_string(),
            recommendation: "Upgrade to 1.1.1t or later".to_string(),
            published_at: chrono::Utc::now(),
        });
    }

    pub fn generate_sbom(&mut self) -> &Sbom {
        &self.sbom
    }

    pub fn add_sbom_entry(&mut self, entry: SbomEntry) {
        self.sbom.add_entry(entry);
    }

    pub fn sign_dependency(&mut self, name: &str, version: &str, content: &[u8], signer: &str) -> DependencySignature {
        let sig = DependencySignature::new(name, version, content, signer);
        let key = format!("{}@{}", name, version);
        self.signatures.insert(key, sig.clone());
        sig
    }

    pub fn verify_signature(&self, name: &str, version: &str, content: &[u8]) -> bool {
        let key = format!("{}@{}", name, version);
        self.signatures.get(&key).map(|s| s.verify(content)).unwrap_or(false)
    }

    pub fn scan_vulnerability(&self, package_name: &str, version: &str) -> VulnerabilityScanResult {
        let mut result = VulnerabilityScanResult::new(package_name, version);
        
        for vuln in &self.known_vulnerabilities {
            if vuln.package.to_lowercase() == package_name.to_lowercase() {
                result.add_vulnerability(vuln.clone());
            }
        }
        
        result
    }

    pub fn add_baseline(&mut self, baseline: VulnerabilityBaseline) {
        self.vulnerability_baselines.insert(baseline.name.clone(), baseline);
    }

    pub fn check_against_baseline(&self, baseline_name: &str, scan_result: &VulnerabilityScanResult) -> bool {
        self.vulnerability_baselines.get(baseline_name)
            .map(|b| b.passes_baseline(scan_result))
            .unwrap_or(true)
    }

    pub fn get_baseline(&self, name: &str) -> Option<&VulnerabilityBaseline> {
        self.vulnerability_baselines.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbom_creation() {
        let mut sbom = Sbom::new();
        assert_eq!(sbom.total_dependencies(), 0);
        
        sbom.add_entry(SbomEntry {
            name: "tokio".to_string(),
            version: "1.0.0".to_string(),
            license: "MIT".to_string(),
            source: "crates.io".to_string(),
            transitive: false,
            hash: "abc123".to_string(),
        });
        
        assert_eq!(sbom.total_dependencies(), 1);
        assert_eq!(sbom.direct_dependencies(), 1);
    }

    #[test]
    fn test_sbom_transitive_counting() {
        let mut sbom = Sbom::new();
        sbom.add_entry(SbomEntry { name: "a".to_string(), version: "1.0".to_string(), license: "MIT".to_string(), source: "".to_string(), transitive: false, hash: "".to_string() });
        sbom.add_entry(SbomEntry { name: "b".to_string(), version: "1.0".to_string(), license: "MIT".to_string(), source: "".to_string(), transitive: true, hash: "".to_string() });
        
        assert_eq!(sbom.direct_dependencies(), 1);
        assert_eq!(sbom.transitive_dependencies(), 1);
    }

    #[test]
    fn test_unique_licenses() {
        let mut sbom = Sbom::new();
        sbom.add_entry(SbomEntry { name: "a".to_string(), version: "1.0".to_string(), license: "MIT".to_string(), source: "".to_string(), transitive: false, hash: "".to_string() });
        sbom.add_entry(SbomEntry { name: "b".to_string(), version: "1.0".to_string(), license: "Apache-2.0".to_string(), source: "".to_string(), transitive: false, hash: "".to_string() });
        sbom.add_entry(SbomEntry { name: "c".to_string(), version: "1.0".to_string(), license: "MIT".to_string(), source: "".to_string(), transitive: false, hash: "".to_string() });
        
        let licenses = sbom.unique_licenses();
        assert_eq!(licenses.len(), 2);
    }

    #[test]
    fn test_dependency_signature() {
        let content = b"package data";
        let sig = DependencySignature::new("test-pkg", "1.0.0", content, "test-signer");
        
        assert_eq!(sig.dependency_name, "test-pkg");
        assert!(sig.verify(content));
        assert!(!sig.verify(b"different content"));
    }

    #[test]
    fn test_severity_from_score() {
        assert_eq!(Severity::from_score(9.5), Severity::Critical);
        assert_eq!(Severity::from_score(7.5), Severity::High);
        assert_eq!(Severity::from_score(4.5), Severity::Medium);
        assert_eq!(Severity::from_score(1.0), Severity::Low);
        assert_eq!(Severity::from_score(0.0), Severity::Info);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
        assert!(Severity::Low > Severity::Info);
    }

    #[test]
    fn test_vulnerability_scan_result() {
        let mut result = VulnerabilityScanResult::new("test-package", "1.0.0");
        assert_eq!(result.total_vulnerabilities, 0);
        
        result.add_vulnerability(Vulnerability {
            cve_id: "CVE-2024-0001".to_string(),
            package: "test-package".to_string(),
            affected_versions: "< 2.0".to_string(),
            severity: Severity::Critical,
            cvss_score: 9.8,
            description: "Test".to_string(),
            recommendation: "Upgrade".to_string(),
            published_at: chrono::Utc::now(),
        });
        
        assert_eq!(result.total_vulnerabilities, 1);
        assert_eq!(result.critical_count, 1);
        assert!(result.has_critical());
    }

    #[test]
    fn test_baseline_passes() {
        let baseline = VulnerabilityBaseline::new("production");
        let mut scan = VulnerabilityScanResult::new("pkg", "1.0");
        scan.add_vulnerability(Vulnerability {
            cve_id: "CVE-2024-0001".to_string(),
            package: "pkg".to_string(),
            affected_versions: "*".to_string(),
            severity: Severity::Low,
            cvss_score: 3.0,
            description: "".to_string(),
            recommendation: "".to_string(),
            published_at: chrono::Utc::now(),
        });
        
        assert!(baseline.passes_baseline(&scan));
    }

    #[test]
    fn test_baseline_fails_critical() {
        let baseline = VulnerabilityBaseline::new("production");
        let mut scan = VulnerabilityScanResult::new("pkg", "1.0");
        scan.add_vulnerability(Vulnerability {
            cve_id: "CVE-2024-0001".to_string(),
            package: "pkg".to_string(),
            affected_versions: "*".to_string(),
            severity: Severity::Critical,
            cvss_score: 10.0,
            description: "".to_string(),
            recommendation: "".to_string(),
            published_at: chrono::Utc::now(),
        });
        
        assert!(!baseline.passes_baseline(&scan));
    }

    #[test]
    fn test_supply_chain_security() {
        let mut security = SupplyChainSecurity::new();
        
        security.add_sbom_entry(SbomEntry {
            name: "serde".to_string(),
            version: "1.0".to_string(),
            license: "MIT".to_string(),
            source: "crates.io".to_string(),
            transitive: false,
            hash: "hash123".to_string(),
        });
        
        let sig = security.sign_dependency("serde", "1.0", b"content", "test-signer");
        assert_eq!(sig.dependency_name, "serde");
        
        assert!(security.verify_signature("serde", "1.0", b"content"));
        
        let scan_result = security.scan_vulnerability("log4j", "2.16.0");
        assert!(scan_result.has_critical());
    }
}
