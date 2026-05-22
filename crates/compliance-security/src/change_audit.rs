//! # Change Audit - Change Record & Impact Analysis
//!
//! Implements change auditing with risk levels, rollback plans,
//! verification evidence, and impact scope tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Risk level for a change
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Low risk - routine changes with minimal impact
    Low = 0,
    /// Medium risk - changes that affect multiple components
    Medium = 1,
    /// High risk - changes affecting critical systems or data
    High = 2,
    /// Critical risk - changes requiring special approval and monitoring
    Critical = 3,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// A single change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    /// Unique identifier for the change
    pub id: String,
    /// Title/description of the change
    pub title: String,
    /// Detailed description
    pub description: String,
    /// Risk assessment
    pub risk_level: RiskLevel,
    /// Who made the change
    pub author: String,
    /// When the change was made
    pub timestamp: DateTime<Utc>,
    /// Components/modules affected
    pub affected_components: Vec<String>,
    /// Type of change
    pub change_type: ChangeType,
    /// Rollback procedure
    pub rollback_plan: String,
    /// Verification steps
    pub verification_evidence: Vec<String>,
    /// Whether the change has been approved
    pub approved: bool,
    /// Approval details
    pub approver: Option<String>,
    pub approval_timestamp: Option<DateTime<Utc>>,
}

impl ChangeRecord {
    pub fn new(
        id: &str,
        title: &str,
        description: &str,
        risk_level: RiskLevel,
        author: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            description: description.to_string(),
            risk_level,
            author: author.to_string(),
            timestamp: Utc::now(),
            affected_components: Vec::new(),
            change_type: ChangeType::Other,
            rollback_plan: String::new(),
            verification_evidence: Vec::new(),
            approved: false,
            approver: None,
            approval_timestamp: None,
        }
    }

    pub fn with_components(mut self, components: Vec<&str>) -> Self {
        self.affected_components = components.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_change_type(mut self, change_type: ChangeType) -> Self {
        self.change_type = change_type;
        self
    }

    pub fn with_rollback_plan(mut self, plan: &str) -> Self {
        self.rollback_plan = plan.to_string();
        self
    }

    pub fn with_verification_evidence(mut self, evidence: Vec<&str>) -> Self {
        self.verification_evidence = evidence.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn approve(&mut self, approver: &str) {
        self.approved = true;
        self.approver = Some(approver.to_string());
        self.approval_timestamp = Some(Utc::now());
    }

    pub fn impact_scope(&self) -> ImpactScope {
        let scope = self.affected_components.len();
        let risk = self.risk_level as usize;
        if scope == 0 && risk == 0 {
            ImpactScope::Minimal
        } else if scope <= 2 && risk <= 1 {
            ImpactScope::Low
        } else if scope <= 5 && risk <= 2 {
            ImpactScope::Medium
        } else if scope <= 10 && risk <= 3 {
            ImpactScope::High
        } else {
            ImpactScope::Critical
        }
    }
}

/// Type of change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    /// New feature implementation
    Feature,
    /// Bug fix
    BugFix,
    /// Performance improvement
    Performance,
    /// Security fix
    SecurityFix,
    /// Configuration change
    ConfigChange,
    /// Dependency update
    DependencyUpdate,
    /// Infrastructure change
    Infrastructure,
    /// Documentation update
    Documentation,
    /// Refactoring
    Refactor,
    /// Other
    Other,
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeType::Feature => write!(f, "Feature"),
            ChangeType::BugFix => write!(f, "Bug Fix"),
            ChangeType::Performance => write!(f, "Performance"),
            ChangeType::SecurityFix => write!(f, "Security Fix"),
            ChangeType::ConfigChange => write!(f, "Config Change"),
            ChangeType::DependencyUpdate => write!(f, "Dependency Update"),
            ChangeType::Infrastructure => write!(f, "Infrastructure"),
            ChangeType::Documentation => write!(f, "Documentation"),
            ChangeType::Refactor => write!(f, "Refactor"),
            ChangeType::Other => write!(f, "Other"),
        }
    }
}

/// Impact scope classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactScope {
    Minimal,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for ImpactScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImpactScope::Minimal => write!(f, "Minimal"),
            ImpactScope::Low => write!(f, "Low"),
            ImpactScope::Medium => write!(f, "Medium"),
            ImpactScope::High => write!(f, "High"),
            ImpactScope::Critical => write!(f, "Critical"),
        }
    }
}

/// Template for audit compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTemplate {
    pub name: String,
    pub required_fields: Vec<RequiredField>,
    pub risk_assessment_required: bool,
    pub approval_required_for_risk_above: Option<RiskLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredField {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl AuditTemplate {
    pub fn standard() -> Self {
        Self {
            name: "Standard Change Audit".to_string(),
            required_fields: vec![
                RequiredField {
                    name: "title".to_string(),
                    description: "Brief description of the change".to_string(),
                    required: true,
                },
                RequiredField {
                    name: "description".to_string(),
                    description: "Detailed description".to_string(),
                    required: true,
                },
                RequiredField {
                    name: "risk_level".to_string(),
                    description: "Risk assessment".to_string(),
                    required: true,
                },
                RequiredField {
                    name: "rollback_plan".to_string(),
                    description: "How to undo this change".to_string(),
                    required: true,
                },
                RequiredField {
                    name: "verification_evidence".to_string(),
                    description: "How to verify the change worked".to_string(),
                    required: true,
                },
                RequiredField {
                    name: "affected_components".to_string(),
                    description: "What this change affects".to_string(),
                    required: true,
                },
            ],
            risk_assessment_required: true,
            approval_required_for_risk_above: Some(RiskLevel::Medium),
        }
    }

    pub fn validate(&self, record: &ChangeRecord) -> ValidationResult {
        let mut missing_fields = Vec::new();
        let mut warnings = Vec::new();

        for field in &self.required_fields {
            if field.required {
                match field.name.as_str() {
                    "title" => {
                        if record.title.is_empty() {
                            missing_fields.push(field.name.clone());
                        }
                    }
                    "description" => {
                        if record.description.is_empty() {
                            missing_fields.push(field.name.clone());
                        }
                    }
                    "risk_level" => {
                        // Risk level always has a value
                    }
                    "rollback_plan" => {
                        if record.rollback_plan.is_empty() {
                            missing_fields.push(field.name.clone());
                        }
                    }
                    "verification_evidence" => {
                        if record.verification_evidence.is_empty() {
                            missing_fields.push(field.name.clone());
                        }
                    }
                    "affected_components" => {
                        if record.affected_components.is_empty() {
                            missing_fields.push(field.name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check approval requirement
        if let Some(min_risk) = self.approval_required_for_risk_above {
            if record.risk_level >= min_risk && !record.approved {
                warnings.push(format!("Change requires approval (risk >= {})", min_risk));
            }
        }

        if record.risk_level >= RiskLevel::High && record.verification_evidence.len() < 2 {
            warnings.push("High-risk changes should have multiple verification steps".to_string());
        }

        ValidationResult {
            valid: missing_fields.is_empty(),
            missing_fields,
            warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub missing_fields: Vec<String>,
    pub warnings: Vec<String>,
}

/// Change impact analyzer
#[derive(Debug, Clone)]
pub struct ChangeImpactAnalyzer {
    component_dependencies: std::collections::HashMap<String, Vec<String>>,
}

impl Default for ChangeImpactAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeImpactAnalyzer {
    pub fn new() -> Self {
        Self {
            component_dependencies: std::collections::HashMap::new(),
        }
    }

    pub fn register_dependency(&mut self, component: &str, depends_on: Vec<&str>) {
        self.component_dependencies.insert(
            component.to_string(),
            depends_on.iter().map(|s| s.to_string()).collect(),
        );
    }

    pub fn analyze(&self, change: &ChangeRecord) -> ImpactAnalysis {
        let mut impacted = change.affected_components.clone();
        let mut visited = std::collections::HashSet::new();
        let mut queue: Vec<String> = change.affected_components.clone();

        // BFS to find transitive dependencies
        while let Some(component) = queue.pop() {
            if visited.contains(&component) {
                continue;
            }
            visited.insert(component.clone());

            if let Some(deps) = self.component_dependencies.get(&component) {
                for dep in deps {
                    if !visited.contains(dep) {
                        impacted.push(dep.clone());
                        queue.push(dep.clone());
                    }
                }
            }
        }

        ImpactAnalysis {
            direct_impact: change.affected_components.clone(),
            transitive_impact: impacted.clone(),
            risk_assessment: self.assess_risk(&impacted, change.risk_level),
        }
    }

    fn assess_risk(&self, impacted: &[String], base_risk: RiskLevel) -> String {
        if impacted.len() > 10 || base_risk >= RiskLevel::Critical {
            "Very High - Consider breaking into smaller changes".to_string()
        } else if impacted.len() > 5 || base_risk >= RiskLevel::High {
            "High - Ensure rollback plan is tested".to_string()
        } else if impacted.len() > 2 || base_risk >= RiskLevel::Medium {
            "Medium - Standard monitoring recommended".to_string()
        } else {
            "Low - Standard procedures apply".to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    pub direct_impact: Vec<String>,
    pub transitive_impact: Vec<String>,
    pub risk_assessment: String,
}

/// Change audit manager
#[derive(Debug, Clone)]
pub struct ChangeAuditManager {
    records: Vec<ChangeRecord>,
    templates: Vec<AuditTemplate>,
}

impl Default for ChangeAuditManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeAuditManager {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            templates: vec![AuditTemplate::standard()],
        }
    }

    pub fn add_record(&mut self, record: ChangeRecord) {
        self.records.push(record);
    }

    pub fn get_record(&self, id: &str) -> Option<&ChangeRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    pub fn validate_record(&self, id: &str) -> Option<ValidationResult> {
        self.get_record(id)
            .and_then(|r| self.templates.first().map(|t| t.validate(r)))
    }

    pub fn all_records(&self) -> &[ChangeRecord] {
        &self.records
    }

    pub fn pending_approval(&self) -> Vec<&ChangeRecord> {
        self.records.iter().filter(|r| !r.approved).collect()
    }

    pub fn by_risk_level(&self, level: RiskLevel) -> Vec<&ChangeRecord> {
        self.records
            .iter()
            .filter(|r| r.risk_level == level)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_ordering() {
        assert!(RiskLevel::Low < RiskLevel::Medium);
        assert!(RiskLevel::Medium < RiskLevel::High);
        assert!(RiskLevel::High < RiskLevel::Critical);
    }

    #[test]
    fn test_change_record_creation() {
        let record = ChangeRecord::new(
            "C001",
            "Test Change",
            "Description",
            RiskLevel::Medium,
            "author1",
        );
        assert_eq!(record.id, "C001");
        assert_eq!(record.title, "Test Change");
        assert_eq!(record.risk_level, RiskLevel::Medium);
        assert!(!record.approved);
    }

    #[test]
    fn test_change_record_builder() {
        let record = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::Low, "auth")
            .with_components(vec!["component1", "component2"])
            .with_change_type(ChangeType::Feature)
            .with_rollback_plan("Rollback steps")
            .with_verification_evidence(vec!["test1", "test2"]);

        assert_eq!(record.affected_components.len(), 2);
        assert_eq!(record.change_type, ChangeType::Feature);
        assert_eq!(record.rollback_plan, "Rollback steps");
        assert_eq!(record.verification_evidence.len(), 2);
    }

    #[test]
    fn test_change_record_approval() {
        let mut record = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::High, "auth");
        assert!(!record.approved);
        record.approve("approver1");
        assert!(record.approved);
        assert_eq!(record.approver, Some("approver1".to_string()));
        assert!(record.approval_timestamp.is_some());
    }

    #[test]
    fn test_change_record_impact_scope() {
        let record = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::Critical, "auth")
            .with_components(vec!["a", "b", "c", "d", "e"]);
        assert_eq!(record.impact_scope(), ImpactScope::High);
    }

    #[test]
    fn test_change_type_display() {
        assert_eq!(ChangeType::Feature.to_string(), "Feature");
        assert_eq!(ChangeType::BugFix.to_string(), "Bug Fix");
        assert_eq!(ChangeType::SecurityFix.to_string(), "Security Fix");
    }

    #[test]
    fn test_audit_template_validation_pass() {
        let template = AuditTemplate::standard();
        let mut record = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::Low, "auth")
            .with_rollback_plan("Rollback")
            .with_verification_evidence(vec!["test"]);
        record.affected_components.push("component1".to_string());

        let result = template.validate(&record);
        assert!(result.valid);
        assert!(result.missing_fields.is_empty());
    }

    #[test]
    fn test_audit_template_validation_fail() {
        let template = AuditTemplate::standard();
        let record = ChangeRecord::new("", "", "", RiskLevel::Low, "auth"); // Missing required fields

        let result = template.validate(&record);
        assert!(!result.valid);
        assert!(!result.missing_fields.is_empty());
    }

    #[test]
    fn test_audit_template_approval_warning() {
        let template = AuditTemplate::standard();
        let mut record = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::High, "auth")
            .with_rollback_plan("Rollback")
            .with_verification_evidence(vec!["test1", "test2"]);
        // Not approved

        let result = template.validate(&record);
        assert!(!result.warnings.is_empty());
    }

    #[test]
    fn test_impact_analyzer_direct_impact() {
        let analyzer = ChangeImpactAnalyzer::new();
        let change = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::Medium, "auth")
            .with_components(vec!["component1"]);

        let impact = analyzer.analyze(&change);
        assert_eq!(impact.direct_impact, vec!["component1"]);
    }

    #[test]
    fn test_impact_analyzer_transitive_impact() {
        let mut analyzer = ChangeImpactAnalyzer::new();
        analyzer.register_dependency("A", vec!["B", "C"]);
        analyzer.register_dependency("B", vec!["D"]);

        let change = ChangeRecord::new("C001", "Test", "Desc", RiskLevel::Medium, "auth")
            .with_components(vec!["A"]);

        let impact = analyzer.analyze(&change);
        assert!(impact.transitive_impact.contains(&"A".to_string()));
        assert!(impact.transitive_impact.contains(&"B".to_string()));
        assert!(impact.transitive_impact.contains(&"C".to_string()));
        assert!(impact.transitive_impact.contains(&"D".to_string()));
    }

    #[test]
    fn test_change_audit_manager() {
        let mut manager = ChangeAuditManager::new();
        manager.add_record(ChangeRecord::new(
            "C001",
            "Test1",
            "Desc",
            RiskLevel::Low,
            "auth1",
        ));
        manager.add_record(ChangeRecord::new(
            "C002",
            "Test2",
            "Desc",
            RiskLevel::High,
            "auth2",
        ));

        assert_eq!(manager.all_records().len(), 2);
        assert_eq!(manager.get_record("C001").unwrap().title, "Test1");
    }

    #[test]
    fn test_change_audit_manager_pending() {
        let mut manager = ChangeAuditManager::new();
        let mut record1 = ChangeRecord::new("C001", "Test1", "Desc", RiskLevel::Low, "auth1");
        record1.approve("approver");

        manager.add_record(record1);
        manager.add_record(ChangeRecord::new(
            "C002",
            "Test2",
            "Desc",
            RiskLevel::High,
            "auth2",
        ));

        assert_eq!(manager.pending_approval().len(), 1);
    }

    #[test]
    fn test_change_audit_manager_by_risk() {
        let mut manager = ChangeAuditManager::new();
        manager.add_record(ChangeRecord::new(
            "C001",
            "Test1",
            "Desc",
            RiskLevel::Low,
            "auth1",
        ));
        manager.add_record(ChangeRecord::new(
            "C002",
            "Test2",
            "Desc",
            RiskLevel::High,
            "auth2",
        ));
        manager.add_record(ChangeRecord::new(
            "C003",
            "Test3",
            "Desc",
            RiskLevel::High,
            "auth3",
        ));

        let high_risk = manager.by_risk_level(RiskLevel::High);
        assert_eq!(high_risk.len(), 2);
    }
}
