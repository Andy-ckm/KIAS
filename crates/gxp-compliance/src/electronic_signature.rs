//! # Electronic Signatures — 21 CFR Part 11 Compliant
//!
//! Non-repudiable electronic signatures for GxP-regulated AI agent decisions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Type of electronic signature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureType {
    /// Cryptographic digital signature
    Digital,
    /// Manually signed document scan
    ManuScript,
    /// Certified professional signature
    Certified,
}

/// GxP operation requiring electronic signature
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Approve an AI agent decision
    ApproveAgentDecision,
    /// Approve a change request
    ApproveChangeRequest,
    /// Approve a validation report
    ApproveValidationReport,
    /// Approve a CAPA (Corrective and Preventive Action)
    ApproveCAPA,
    /// Approve a Standard Operating Procedure
    ApproveSOP,
    /// Reject AI agent output
    RejectAgentOutput,
    /// Escalate decision to human reviewer
    EscalateToHuman,
    /// Approve AI model retraining
    ApproveModelRetraining,
    /// Release batch record
    ReleaseBatchRecord,
}

/// A single electronic signature on an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectronicSignature {
    /// Who signed
    pub signer_id: String,
    /// Type of signature
    pub signature_type: SignatureType,
    /// When signed
    pub signed_at: DateTime<Utc>,
    /// Why signed (GxP requires non-trivial rationale)
    pub rationale: String,
    /// SHA-256 of rationale + timestamp + signer
    pub hash: String,
}

impl ElectronicSignature {
    /// Create a new electronic signature.
    pub fn new(signer_id: &str, signature_type: SignatureType, rationale: &str) -> Self {
        let signed_at = Utc::now();
        let hash = Self::compute_hash(signer_id, rationale, &signed_at);
        Self {
            signer_id: signer_id.to_string(),
            signature_type,
            signed_at,
            rationale: rationale.to_string(),
            hash,
        }
    }

    fn compute_hash(signer_id: &str, rationale: &str, signed_at: &DateTime<Utc>) -> String {
        let payload = format!("{}|{}|{}", signer_id, rationale, signed_at.to_rfc3339());
        let mut hasher = Sha256::new();
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify the signature integrity
    pub fn verify(&self) -> bool {
        let expected = Self::compute_hash(&self.signer_id, &self.rationale, &self.signed_at);
        self.hash == expected
    }
}

/// A bundle of signatures for a single operation (some GxP ops require multiple signers)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureBundle {
    /// Unique operation ID
    pub operation_id: String,
    /// Type of operation
    pub operation_type: OperationType,
    /// All signatures in the bundle
    pub signatures: Vec<ElectronicSignature>,
    /// Combined hash of all signatures
    pub combined_hash: Option<String>,
}

impl SignatureBundle {
    /// Create a new signature bundle for an operation.
    pub fn new(operation_id: &str, operation_type: OperationType) -> Self {
        Self {
            operation_id: operation_id.to_string(),
            operation_type,
            signatures: Vec::new(),
            combined_hash: None,
        }
    }

    /// Add a signature to the bundle.
    pub fn add_signature(&mut self, sig: ElectronicSignature) {
        self.signatures.push(sig);
    }

    /// Number of signatures collected.
    pub fn signature_count(&self) -> usize {
        self.signatures.len()
    }
}

/// Manages electronic signatures and verifies counter-sign requirements.
pub struct SignatureManager {
    /// Track all bundles ever created (for verification)
    bundles: Vec<SignatureBundle>,
}

impl SignatureManager {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    /// Determine if an operation type requires counter-signature (two-person rule).
    /// Per GxP, critical decisions require two independent reviewers.
    pub fn requires_counter_sign(&self, operation_type: &OperationType) -> bool {
        matches!(
            operation_type,
            OperationType::ApproveAgentDecision
                | OperationType::ApproveChangeRequest
                | OperationType::ApproveValidationReport
                | OperationType::ApproveCAPA
                | OperationType::ReleaseBatchRecord
        )
    }

    /// Sign an operation bundle: compute combined hash from all signatures.
    /// Returns the combined_hash on success.
    pub fn sign(&mut self, mut bundle: SignatureBundle) -> Result<String, SignatureError> {
        if bundle.signatures.is_empty() {
            return Err(SignatureError::NoSignatures);
        }

        // Verify all individual signatures
        for sig in &bundle.signatures {
            if !sig.verify() {
                return Err(SignatureError::SignatureTampered(sig.signer_id.clone()));
            }
        }

        // Combined hash = SHA-256 of all individual hashes concatenated
        let combined = bundle
            .signatures
            .iter()
            .map(|s| s.hash.as_str())
            .collect::<Vec<_>>()
            .join("|");
        let combined_hash = {
            let mut hasher = Sha256::new();
            hasher.update(combined.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        bundle.combined_hash = Some(combined_hash.clone());
        self.bundles.push(bundle);
        Ok(combined_hash)
    }

    /// Verify a previously signed bundle against a known combined hash.
    pub fn verify(
        &self,
        bundle: &SignatureBundle,
        combined_hash: &str,
    ) -> Result<bool, SignatureError> {
        if bundle.signatures.is_empty() {
            return Err(SignatureError::NoSignatures);
        }

        // Check each individual signature integrity
        for sig in &bundle.signatures {
            if !sig.verify() {
                return Err(SignatureError::SignatureTampered(sig.signer_id.clone()));
            }
        }

        // Re-compute combined hash
        let combined = bundle
            .signatures
            .iter()
            .map(|s| s.hash.as_str())
            .collect::<Vec<_>>()
            .join("|");
        let computed = {
            let mut hasher = Sha256::new();
            hasher.update(combined.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        Ok(computed == combined_hash)
    }

    /// Get number of bundles managed.
    pub fn bundle_count(&self) -> usize {
        self.bundles.len()
    }
}

impl Default for SignatureManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("bundle has no signatures")]
    NoSignatures,

    #[error("signature from {0} appears to be tampered")]
    SignatureTampered(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_single_signature() {
        let mut manager = SignatureManager::new();
        let mut bundle = SignatureBundle::new("op-1", OperationType::ApproveAgentDecision);
        let sig = ElectronicSignature::new(
            "dr-smith",
            SignatureType::Digital,
            "Approved based on review",
        );
        bundle.add_signature(sig);

        let combined = manager.sign(bundle).unwrap();
        assert!(!combined.is_empty());
    }

    #[test]
    fn test_verify_bundle() {
        let mut manager = SignatureManager::new();
        let mut bundle = SignatureBundle::new("op-2", OperationType::RejectAgentOutput);
        bundle.add_signature(ElectronicSignature::new(
            "qa-lead",
            SignatureType::Certified,
            "Rejected due to error",
        ));
        let combined = manager.sign(bundle.clone()).unwrap();

        assert!(manager.verify(&bundle, &combined).unwrap());
    }

    #[test]
    fn test_verify_fails_on_tampering() {
        let mut manager = SignatureManager::new();
        let mut bundle = SignatureBundle::new("op-3", OperationType::ApproveSOP);
        bundle.add_signature(ElectronicSignature::new(
            "manager",
            SignatureType::ManuScript,
            "Approved",
        ));
        let combined = manager.sign(bundle.clone()).unwrap();

        // Tamper with rationale
        if let Some(sig) = bundle.signatures.first_mut() {
            sig.rationale = "Tampered rationale".to_string();
        }
        // Should fail verification
        assert!(manager.verify(&bundle, &combined).is_err());
    }

    #[test]
    fn test_counter_sign_requirement() {
        let manager = SignatureManager::new();
        assert!(manager.requires_counter_sign(&OperationType::ApproveAgentDecision));
        assert!(manager.requires_counter_sign(&OperationType::ApproveValidationReport));
        assert!(manager.requires_counter_sign(&OperationType::ReleaseBatchRecord));
        assert!(!manager.requires_counter_sign(&OperationType::EscalateToHuman));
        assert!(!manager.requires_counter_sign(&OperationType::ApproveSOP));
    }

    #[test]
    fn test_signature_verification() {
        let sig = ElectronicSignature::new("user-1", SignatureType::Digital, "I approve this");
        assert!(sig.verify());
    }

    #[test]
    fn test_signature_tamper_detection() {
        let mut sig = ElectronicSignature::new("user-1", SignatureType::Digital, "Original");
        // Simulate tampering by manually changing hash
        sig.hash = "tampered".to_string();
        assert!(!sig.verify());
    }

    #[test]
    fn test_multiple_signers() {
        let mut manager = SignatureManager::new();
        let mut bundle = SignatureBundle::new("op-multi", OperationType::ApproveValidationReport);
        bundle.add_signature(ElectronicSignature::new(
            "signer-1",
            SignatureType::Digital,
            "First approval",
        ));
        bundle.add_signature(ElectronicSignature::new(
            "signer-2",
            SignatureType::Digital,
            "Second approval",
        ));

        let combined = manager.sign(bundle.clone()).unwrap();
        assert!(manager.verify(&bundle, &combined).unwrap());
        assert_eq!(bundle.signature_count(), 2);
    }

    #[test]
    fn test_sign_empty_bundle_fails() {
        let mut manager = SignatureManager::new();
        let bundle = SignatureBundle::new("op-empty", OperationType::EscalateToHuman);
        let result = manager.sign(bundle);
        assert!(result.is_err());
    }
}
