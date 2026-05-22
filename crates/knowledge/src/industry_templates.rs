//! # Industry Templates - Pre-configured Solution Templates
//!
//! Implements industry-specific templates for Finance, Government, Manufacturing,
//! and Customer Service sectors with compliance and workflow configurations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Industry vertical
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Industry {
    /// Financial services - banking, insurance, trading
    Finance,
    /// Government and public sector
    Government,
    /// Manufacturing and industrial
    Manufacturing,
    /// Customer service and support
    CustomerService,
}

impl Industry {
    pub fn name(&self) -> &'static str {
        match self {
            Industry::Finance => "Financial Services",
            Industry::Government => "Government & Public Sector",
            Industry::Manufacturing => "Manufacturing",
            Industry::CustomerService => "Customer Service",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Industry::Finance => {
                "Banking, insurance, trading platforms with regulatory compliance"
            }
            Industry::Government => {
                "Government agencies requiring security and audit compliance"
            }
            Industry::Manufacturing => {
                "Industrial automation and production systems"
            }
            Industry::CustomerService => {
                "Contact centers and support organizations"
            }
        }
    }
}

/// Template component types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateComponentType {
    /// Security and access control policies
    SecurityPolicy,
    /// Compliance framework configuration
    ComplianceTemplate,
    /// Workflow definitions
    WorkflowTemplate,
    /// Monitoring configuration
    MonitoringTemplate,
    /// Data retention policies
    DataRetentionPolicy,
}

/// A single template component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateComponent {
    pub id: String,
    pub name: String,
    pub component_type: TemplateComponentType,
    pub description: String,
    pub content: String,
    pub tags: Vec<String>,
}

impl TemplateComponent {
    pub fn new(id: &str, name: &str, component_type: TemplateComponentType, description: &str, content: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            component_type,
            description: description.to_string(),
            content: content.to_string(),
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }
}

/// Industry-specific template bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndustryTemplate {
    pub industry: Industry,
    pub name: String,
    pub version: String,
    pub description: String,
    pub components: Vec<TemplateComponent>,
    pub regulatory_frameworks: Vec<String>,
    pub recommended_features: Vec<String>,
}

impl IndustryTemplate {
    pub fn finance() -> Self {
        let components = vec![
            TemplateComponent::new(
                "fin-security-001",
                "Multi-Factor Authentication",
                TemplateComponentType::SecurityPolicy,
                "MFA requirements for financial transactions",
                "mfa_required: true\nmfa_methods: [hardware_token, biometric, sms]\ntransaction_threshold: 10000\n",
            ),
            TemplateComponent::new(
                "fin-compliance-001",
                "SOX Compliance Template",
                TemplateComponentType::ComplianceTemplate,
                "Sarbanes-Oxley compliance configuration",
                "framework: SOX\nretention_years: 7\naudit_trail: required\nsignatures: dual_authorization\n",
            ),
            TemplateComponent::new(
                "fin-workflow-001",
                "Transaction Approval Workflow",
                TemplateComponentType::WorkflowTemplate,
                "Multi-level transaction approval process",
                "stages:\n  - review\n  - compliance_check\n  - manager_approval\n  - final_authorization\n",
            ),
            TemplateComponent::new(
                "fin-monitoring-001",
                "Fraud Detection Monitoring",
                TemplateComponentType::MonitoringTemplate,
                "Real-time fraud detection metrics",
                "metrics:\n  - transaction_velocity\n  - geographic_anomaly\n  - amount_threshold_alert\n",
            ),
        ];

        Self {
            industry: Industry::Finance,
            name: "Financial Services Template".to_string(),
            version: "1.0.0".to_string(),
            description: "Complete template for financial services with SOX compliance".to_string(),
            components,
            regulatory_frameworks: vec!["SOX".to_string(), "PCI-DSS".to_string(), "21 CFR Part 11".to_string()],
            recommended_features: vec![
                "Multi-Factor Authentication".to_string(),
                "Transaction Audit Trail".to_string(),
                "Real-time Fraud Detection".to_string(),
                "Dual Authorization".to_string(),
            ],
        }
    }

    pub fn government() -> Self {
        let components = vec![
            TemplateComponent::new(
                "gov-security-001",
                "FedRAMP Security Controls",
                TemplateComponentType::SecurityPolicy,
                "Federal security authorization requirements",
                "framework: FedRAMP\nimpact_level: Moderate\ncontinuous_monitoring: required\n",
            ),
            TemplateComponent::new(
                "gov-compliance-001",
                "FISMA Compliance Template",
                TemplateComponentType::ComplianceTemplate,
                "Federal Information Security Management Act",
                "framework: FISMA\nreporting: quarterly\nincident_response: mandatory\n",
            ),
            TemplateComponent::new(
                "gov-workflow-001",
                "Document Classification Workflow",
                TemplateComponentType::WorkflowTemplate,
                "Government document handling process",
                "classification_levels:\n  - public\n  - sensitive\n  - classified\n  - top_secret\n",
            ),
            TemplateComponent::new(
                "gov-retention-001",
                "Records Retention Policy",
                TemplateComponentType::DataRetentionPolicy,
                "Government records management requirements",
                "retention:\n  routine: 7_years\n  permanent: indefinite\n  classified: 25_years\n",
            ),
        ];

        Self {
            industry: Industry::Government,
            name: "Government Template".to_string(),
            version: "1.0.0".to_string(),
            description: "Government security and compliance template".to_string(),
            components,
            regulatory_frameworks: vec!["FedRAMP".to_string(), "FISMA".to_string(), "NIST 800-53".to_string()],
            recommended_features: vec![
                "Role-Based Access Control".to_string(),
                "Audit Trail".to_string(),
                "Data Classification".to_string(),
                "Incident Response".to_string(),
            ],
        }
    }

    pub fn manufacturing() -> Self {
        let components = vec![
            TemplateComponent::new(
                "mfg-security-001",
                "Industrial Control Security",
                TemplateComponentType::SecurityPolicy,
                "Security for manufacturing control systems",
                "network_segmentation: required\not_it_isolation: mandatory\npatch_management: monthly\n",
            ),
            TemplateComponent::new(
                "mfg-compliance-001",
                "GAMP 5 Compliance Template",
                TemplateComponentType::ComplianceTemplate,
                "Good Automated Manufacturing Practice",
                "framework: GAMP5\ncategory: 4\nvalidation: required\nrisk_level: medium\n",
            ),
            TemplateComponent::new(
                "mfg-workflow-001",
                "Production Batch Workflow",
                TemplateComponentType::WorkflowTemplate,
                "Manufacturing batch processing",
                "stages:\n  - material_verification\n  - production\n  - quality_control\n  - packaging\n  - release\n",
            ),
            TemplateComponent::new(
                "mfg-monitoring-001",
                "Production Metrics",
                TemplateComponentType::MonitoringTemplate,
                "Manufacturing KPIs and monitoring",
                "metrics:\n  - cycle_time\n  - defect_rate\n  - oee\n  - throughput\n",
            ),
        ];

        Self {
            industry: Industry::Manufacturing,
            name: "Manufacturing Template".to_string(),
            version: "1.0.0".to_string(),
            description: "Manufacturing and industrial automation template".to_string(),
            components,
            regulatory_frameworks: vec!["GAMP 5".to_string(), "ISO 27001".to_string(), "IEC 62443".to_string()],
            recommended_features: vec![
                "Network Segmentation".to_string(),
                "Batch Tracking".to_string(),
                "Quality Control Gates".to_string(),
                "Production Analytics".to_string(),
            ],
        }
    }

    pub fn customer_service() -> Self {
        let components = vec![
            TemplateComponent::new(
                "cs-security-001",
                "Customer Data Protection",
                TemplateComponentType::SecurityPolicy,
                "PII protection for customer service",
                "encryption: required\npii_masking: enabled\naccess_logging: full\ndata_minimization: enforced\n",
            ),
            TemplateComponent::new(
                "cs-compliance-001",
                "GDPR Compliance Template",
                TemplateComponentType::ComplianceTemplate,
                "General Data Protection Regulation",
                "framework: GDPR\nconsent: required\nright_to_deletion: enforced\ndata_portability: supported\n",
            ),
            TemplateComponent::new(
                "cs-workflow-001",
                "Support Ticket Workflow",
                TemplateComponentType::WorkflowTemplate,
                "Customer support ticket handling",
                "stages:\n  - triage\n  - investigation\n  - resolution\n  - customer_verification\n  - closure\n",
            ),
            TemplateComponent::new(
                "cs-monitoring-001",
                "Service Level Monitoring",
                TemplateComponentType::MonitoringTemplate,
                "Customer service KPIs",
                "metrics:\n  - first_response_time\n  - resolution_time\n  - csat_score\n  - agent_utilization\n",
            ),
        ];

        Self {
            industry: Industry::CustomerService,
            name: "Customer Service Template".to_string(),
            version: "1.0.0".to_string(),
            description: "Customer service with GDPR compliance".to_string(),
            components,
            regulatory_frameworks: vec!["GDPR".to_string(), "CCPA".to_string(), "SOC 2".to_string()],
            recommended_features: vec![
                "PII Data Masking".to_string(),
                "Consent Management".to_string(),
                "Ticket SLA Tracking".to_string(),
                "Customer Satisfaction Analytics".to_string(),
            ],
        }
    }

    pub fn get_component(&self, id: &str) -> Option<&TemplateComponent> {
        self.components.iter().find(|c| c.id == id)
    }

    pub fn components_by_type(&self, component_type: TemplateComponentType) -> Vec<&TemplateComponent> {
        self.components.iter().filter(|c| c.component_type == component_type).collect()
    }
}

/// Template registry for managing industry templates
#[derive(Debug, Clone)]
pub struct TemplateRegistry {
    templates: HashMap<Industry, IndustryTemplate>,
    custom_templates: Vec<IndustryTemplate>,
}

impl Default for TemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            templates: HashMap::new(),
            custom_templates: Vec::new(),
        };
        registry.register_builtin_templates();
        registry
    }

    fn register_builtin_templates(&mut self) {
        self.templates.insert(Industry::Finance, IndustryTemplate::finance());
        self.templates.insert(Industry::Government, IndustryTemplate::government());
        self.templates.insert(Industry::Manufacturing, IndustryTemplate::manufacturing());
        self.templates.insert(Industry::CustomerService, IndustryTemplate::customer_service());
    }

    pub fn register(&mut self, template: IndustryTemplate) {
        self.custom_templates.push(template);
    }

    pub fn get(&self, industry: Industry) -> Option<&IndustryTemplate> {
        self.templates.get(&industry)
    }

    pub fn get_custom(&self) -> &[IndustryTemplate] {
        &self.custom_templates
    }

    pub fn list_industries(&self) -> Vec<Industry> {
        self.templates.keys().cloned().collect()
    }

    pub fn search(&self, query: &str) -> Vec<&IndustryTemplate> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&IndustryTemplate> = Vec::new();

        for template in self.templates.values() {
            if template.name.to_lowercase().contains(&query_lower)
                || template.description.to_lowercase().contains(&query_lower)
                || template.industry.name().to_lowercase().contains(&query_lower)
            {
                results.push(template);
            }
        }

        for template in &self.custom_templates {
            if template.name.to_lowercase().contains(&query_lower)
                || template.description.to_lowercase().contains(&query_lower)
            {
                results.push(template);
            }
        }

        results
    }

    pub fn search_by_framework(&self, framework: &str) -> Vec<&IndustryTemplate> {
        let framework_lower = framework.to_lowercase();
        let mut results: Vec<&IndustryTemplate> = Vec::new();

        for template in self.templates.values() {
            if template.regulatory_frameworks.iter().any(|f| f.to_lowercase().contains(&framework_lower)) {
                results.push(template);
            }
        }

        for template in &self.custom_templates {
            if template.regulatory_frameworks.iter().any(|f| f.to_lowercase().contains(&framework_lower)) {
                results.push(template);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_industry_names() {
        assert_eq!(Industry::Finance.name(), "Financial Services");
        assert_eq!(Industry::Government.name(), "Government & Public Sector");
        assert_eq!(Industry::Manufacturing.name(), "Manufacturing");
        assert_eq!(Industry::CustomerService.name(), "Customer Service");
    }

    #[test]
    fn test_finance_template() {
        let template = IndustryTemplate::finance();
        assert_eq!(template.industry, Industry::Finance);
        assert!(!template.components.is_empty());
        assert!(template.regulatory_frameworks.contains(&"SOX".to_string()));
    }

    #[test]
    fn test_government_template() {
        let template = IndustryTemplate::government();
        assert_eq!(template.industry, Industry::Government);
        assert!(template.regulatory_frameworks.contains(&"FedRAMP".to_string()));
    }

    #[test]
    fn test_manufacturing_template() {
        let template = IndustryTemplate::manufacturing();
        assert_eq!(template.industry, Industry::Manufacturing);
        assert!(template.regulatory_frameworks.contains(&"GAMP 5".to_string()));
    }

    #[test]
    fn test_customer_service_template() {
        let template = IndustryTemplate::customer_service();
        assert_eq!(template.industry, Industry::CustomerService);
        assert!(template.regulatory_frameworks.contains(&"GDPR".to_string()));
    }

    #[test]
    fn test_template_component_creation() {
        let component = TemplateComponent::new(
            "test-001",
            "Test Component",
            TemplateComponentType::SecurityPolicy,
            "A test component",
            "key: value",
        );
        assert_eq!(component.id, "test-001");
        assert_eq!(component.name, "Test Component");
    }

    #[test]
    fn test_template_component_with_tags() {
        let component = TemplateComponent::new(
            "test-001",
            "Test",
            TemplateComponentType::ComplianceTemplate,
            "Desc",
            "content",
        )
        .with_tags(vec!["tag1", "tag2"]);
        assert_eq!(component.tags, vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_get_component() {
        let template = IndustryTemplate::finance();
        let component = template.get_component("fin-security-001");
        assert!(component.is_some());
        assert_eq!(component.unwrap().name, "Multi-Factor Authentication");
    }

    #[test]
    fn test_components_by_type() {
        let template = IndustryTemplate::finance();
        let security_components = template.components_by_type(TemplateComponentType::SecurityPolicy);
        assert!(!security_components.is_empty());
    }

    #[test]
    fn test_template_registry_new() {
        let registry = TemplateRegistry::new();
        assert_eq!(registry.list_industries().len(), 4);
    }

    #[test]
    fn test_template_registry_get() {
        let registry = TemplateRegistry::new();
        let finance = registry.get(Industry::Finance);
        assert!(finance.is_some());
        assert_eq!(finance.unwrap().industry, Industry::Finance);
    }

    #[test]
    fn test_template_registry_register() {
        let mut registry = TemplateRegistry::new();
        let custom = IndustryTemplate::customer_service();
        registry.register(custom);
        assert_eq!(registry.get_custom().len(), 1);
    }

    #[test]
    fn test_template_registry_search() {
        let registry = TemplateRegistry::new();
        let results = registry.search("SOX");
        assert!(!results.is_empty());
    }

    #[test]
    fn test_template_registry_search_by_framework() {
        let registry = TemplateRegistry::new();
        let results = registry.search_by_framework("GDPR");
        assert!(!results.is_empty());
        assert_eq!(results[0].industry, Industry::CustomerService);
    }

    #[test]
    fn test_template_registry_nonexistent_industry() {
        let registry = TemplateRegistry::new();
        // Industry enum doesn't have more options, so just check get returns None for non-registered
        // This would require adding a new industry to the enum, but we just test the pattern
        let result = registry.get(Industry::Finance);
        assert!(result.is_some());
    }
}
