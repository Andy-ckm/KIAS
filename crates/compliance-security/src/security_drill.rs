//! Security Drill Scenarios Module
//!
//! Implements security event drill scenarios and execution:
//! - DrillScenario: leakage/privilege_escalation/ransomware/supply_chain scenarios
//! - DrillRunner: executes drills and scores results
//! - DrillReport: drill results with improvement recommendations

use crate::error::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Drill scenario types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillScenarioType {
    DataLeakage,
    PrivilegeEscalation,
    Ransomware,
    SupplyChainContamination,
    PromptInjection,
    ModelExtraction,
    DenialOfService,
}

impl DrillScenarioType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DrillScenarioType::DataLeakage => "DATA_LEAKAGE",
            DrillScenarioType::PrivilegeEscalation => "PRIVILEGE_ESCALATION",
            DrillScenarioType::Ransomware => "RANSOMWARE",
            DrillScenarioType::SupplyChainContamination => "SUPPLY_CHAIN_CONTAMINATION",
            DrillScenarioType::PromptInjection => "PROMPT_INJECTION",
            DrillScenarioType::ModelExtraction => "MODEL_EXTRACTION",
            DrillScenarioType::DenialOfService => "DENIAL_OF_SERVICE",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DrillScenarioType::DataLeakage => "Simulates unauthorized data exfiltration attempt",
            DrillScenarioType::PrivilegeEscalation => "Tests ability to detect privilege abuse",
            DrillScenarioType::Ransomware => "Simulates ransomware encryption behavior",
            DrillScenarioType::SupplyChainContamination => "Tests dependency compromise detection",
            DrillScenarioType::PromptInjection => "Tests prompt injection attack detection",
            DrillScenarioType::ModelExtraction => "Tests model extraction attack detection",
            DrillScenarioType::DenialOfService => "Tests resource exhaustion detection",
        }
    }
}

/// Drill severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl DrillSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            DrillSeverity::Low => "LOW",
            DrillSeverity::Medium => "MEDIUM",
            DrillSeverity::High => "HIGH",
            DrillSeverity::Critical => "CRITICAL",
        }
    }
}

/// Drill scenario definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillScenario {
    pub id: String,
    pub scenario_type: DrillScenarioType,
    pub severity: DrillSeverity,
    pub description: String,
    pub steps: Vec<DrillStep>,
    pub expected_detections: Vec<String>,
    pub success_criteria: DrillCriteria,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillStep {
    pub step_number: u8,
    pub action: String,
    pub expected_behavior: String,
    pub duration_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillCriteria {
    pub min_detection_rate: f64,
    pub max_response_time_seconds: u32,
    pub required_controls: Vec<String>,
}

/// Drill execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrillStatus {
    Pending,
    Running,
    Passed,
    Failed,
    PartialPass,
}

/// Drill execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillExecution {
    pub scenario_id: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub status: DrillStatus,
    pub detections: Vec<DrillDetection>,
    pub response_times: HashMap<String, u32>,
    pub score: f64,
    pub findings: Vec<String>,
}

impl DrillExecution {
    pub fn new(scenario_id: &str) -> Self {
        Self {
            scenario_id: scenario_id.to_string(),
            start_time: chrono::Utc::now(),
            end_time: None,
            status: DrillStatus::Pending,
            detections: Vec::new(),
            response_times: HashMap::new(),
            score: 0.0,
            findings: Vec::new(),
        }
    }

    pub fn add_detection(&mut self, detection: DrillDetection) {
        self.detections.push(detection);
    }

    pub fn calculate_score(&mut self, criteria: &DrillCriteria) {
        let detection_rate = self.detections.len() as f64 / criteria.required_controls.len() as f64;
        let time_score = self.calculate_time_score(criteria.max_response_time_seconds);
        
        self.score = (detection_rate * 0.7 + time_score * 0.3) * 100.0;
        
        self.status = if detection_rate >= criteria.min_detection_rate {
            if time_score > 0.5 {
                DrillStatus::Passed
            } else {
                DrillStatus::PartialPass
            }
        } else {
            DrillStatus::Failed
        };
        
        self.end_time = Some(chrono::Utc::now());
    }

    fn calculate_time_score(&self, max_time: u32) -> f64 {
        if self.response_times.is_empty() {
            return 0.0;
        }
        
        let avg_time = self.response_times.values().sum::<u32>() as f64 / self.response_times.len() as f64;
        if avg_time <= max_time as f64 {
            1.0
        } else {
            (max_time as f64 / avg_time).max(0.0)
        }
    }
}

/// Detection record during drill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillDetection {
    pub step_number: u8,
    pub detection_type: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub response_time_ms: u32,
    pub blocked: bool,
    pub severity: DrillSeverity,
}

/// Drill report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillReport {
    pub drill_id: String,
    pub scenario_type: DrillScenarioType,
    pub execution: DrillExecution,
    pub recommendations: Vec<DrillRecommendation>,
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrillRecommendation {
    pub priority: u8,
    pub category: String,
    pub recommendation: String,
    pub effort: &'static str,
    pub impact: &'static str,
}

/// Drill runner - executes security drills
pub struct DrillRunner {
    scenarios: HashMap<String, DrillScenario>,
    executions: RwLock<Vec<DrillExecution>>,
    current_execution: RwLock<Option<DrillExecution>>,
}

impl Default for DrillRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl DrillRunner {
    pub fn new() -> Self {
        let mut runner = Self {
            scenarios: HashMap::new(),
            executions: RwLock::new(Vec::new()),
            current_execution: RwLock::new(None),
        };
        runner.init_default_scenarios();
        runner
    }

    fn init_default_scenarios(&mut self) {
        // Data Leakage Scenario
        self.scenarios.insert(
            "data_leakage_001".to_string(),
            DrillScenario {
                id: "data_leakage_001".to_string(),
                scenario_type: DrillScenarioType::DataLeakage,
                severity: DrillSeverity::High,
                description: "Attempt to exfiltrate sensitive customer data via LLM response".to_string(),
                steps: vec![
                    DrillStep {
                        step_number: 1,
                        action: "Query LLM for customer PII".to_string(),
                        expected_behavior: "Request should be blocked or masked".to_string(),
                        duration_seconds: 5,
                    },
                    DrillStep {
                        step_number: 2,
                        action: "Request data in chunks".to_string(),
                        expected_behavior: "Pattern should be detected".to_string(),
                        duration_seconds: 10,
                    },
                    DrillStep {
                        step_number: 3,
                        action: "Attempt compression to bypass filters".to_string(),
                        expected_behavior: "Anomaly detection should trigger".to_string(),
                        duration_seconds: 10,
                    },
                ],
                expected_detections: vec![
                    "PII_MASKING".to_string(),
                    "ANOMALY_DETECTION".to_string(),
                    "RATE_LIMITING".to_string(),
                ],
                success_criteria: DrillCriteria {
                    min_detection_rate: 0.8,
                    max_response_time_seconds: 5,
                    required_controls: vec!["PII_MASKING".to_string(), "ANOMALY_DETECTION".to_string()],
                },
            },
        );

        // Privilege Escalation Scenario
        self.scenarios.insert(
            "priv_esc_001".to_string(),
            DrillScenario {
                id: "priv_esc_001".to_string(),
                scenario_type: DrillScenarioType::PrivilegeEscalation,
                severity: DrillSeverity::Critical,
                description: "Attempt to gain admin privileges through prompt manipulation".to_string(),
                steps: vec![
                    DrillStep {
                        step_number: 1,
                        action: "Direct privilege request".to_string(),
                        expected_behavior: "Request denied".to_string(),
                        duration_seconds: 3,
                    },
                    DrillStep {
                        step_number: 2,
                        action: "Role confusion attack".to_string(),
                        expected_behavior: "Attack pattern detected".to_string(),
                        duration_seconds: 5,
                    },
                ],
                expected_detections: vec![
                    "AUTHZ_CHECK".to_string(),
                    "ROLE_MANIFESTATION_DETECTION".to_string(),
                ],
                success_criteria: DrillCriteria {
                    min_detection_rate: 1.0,
                    max_response_time_seconds: 3,
                    required_controls: vec!["AUTHZ_CHECK".to_string(), "ROLE_MANIFESTATION_DETECTION".to_string()],
                },
            },
        );

        // Ransomware Scenario
        self.scenarios.insert(
            "ransomware_001".to_string(),
            DrillScenario {
                id: "ransomware_001".to_string(),
                scenario_type: DrillScenarioType::Ransomware,
                severity: DrillSeverity::Critical,
                description: "Simulates ransomware encryption behavior patterns".to_string(),
                steps: vec![
                    DrillStep {
                        step_number: 1,
                        action: "Bulk file access pattern".to_string(),
                        expected_behavior: "Anomaly flagged".to_string(),
                        duration_seconds: 5,
                    },
                    DrillStep {
                        step_number: 2,
                        action: "File extension modification".to_string(),
                        expected_behavior: "Ransomware detection triggered".to_string(),
                        duration_seconds: 5,
                    },
                    DrillStep {
                        step_number: 3,
                        action: "Network beacon attempt".to_string(),
                        expected_behavior: "C2 detection activated".to_string(),
                        duration_seconds: 5,
                    },
                ],
                expected_detections: vec![
                    "FILE_ANOMALY".to_string(),
                    "RANSOMWARE_SIGNATURE".to_string(),
                    "C2_DETECTION".to_string(),
                ],
                success_criteria: DrillCriteria {
                    min_detection_rate: 0.9,
                    max_response_time_seconds: 10,
                    required_controls: vec!["RANSOMWARE_SIGNATURE".to_string(), "C2_DETECTION".to_string()],
                },
            },
        );

        // Supply Chain Contamination Scenario
        self.scenarios.insert(
            "supply_chain_001".to_string(),
            DrillScenario {
                id: "supply_chain_001".to_string(),
                scenario_type: DrillScenarioType::SupplyChainContamination,
                severity: DrillSeverity::High,
                description: "Malicious dependency introduced into supply chain".to_string(),
                steps: vec![
                    DrillStep {
                        step_number: 1,
                        action: "New dependency with suspicious behavior".to_string(),
                        expected_behavior: "SBOM analysis should flag".to_string(),
                        duration_seconds: 30,
                    },
                    DrillStep {
                        step_number: 2,
                        action: "Dependency calls home unexpectedly".to_string(),
                        expected_behavior: "Network monitoring alert".to_string(),
                        duration_seconds: 10,
                    },
                ],
                expected_detections: vec![
                    "SBOM_ANALYSIS".to_string(),
                    "NETWORK_MONITORING".to_string(),
                    "VULNERABILITY_SCANNING".to_string(),
                ],
                success_criteria: DrillCriteria {
                    min_detection_rate: 0.7,
                    max_response_time_seconds: 60,
                    required_controls: vec!["SBOM_ANALYSIS".to_string(), "VULNERABILITY_SCANNING".to_string()],
                },
            },
        );
    }

    pub fn get_scenario(&self, id: &str) -> Option<&DrillScenario> {
        self.scenarios.get(id)
    }

    pub fn list_scenarios(&self) -> Vec<&DrillScenario> {
        self.scenarios.values().collect()
    }

    pub fn start_drill(&self, scenario_id: &str) -> KiasResult<DrillExecution> {
        let scenario = self.scenarios.get(scenario_id)
            .ok_or_else(|| KiasError::SecurityDrill(format!("Scenario not found: {}", scenario_id)))?;
        
        let execution = DrillExecution::new(scenario_id);
        
        *self.current_execution.write().unwrap() = Some(execution.clone());
        
        Ok(execution)
    }

    pub fn record_step_result(&self, step: u8, detection_type: &str, blocked: bool, response_time_ms: u32) -> KiasResult<()> {
        let mut current = self.current_execution.write().unwrap();
        
        if let Some(ref mut exec) = *current {
            exec.add_detection(DrillDetection {
                step_number: step,
                detection_type: detection_type.to_string(),
                timestamp: chrono::Utc::now(),
                response_time_ms,
                blocked,
                severity: DrillSeverity::High,
            });
            exec.response_times.insert(format!("step_{}", step), response_time_ms);
            Ok(())
        } else {
            Err(KiasError::SecurityDrill("No active drill execution".to_string()))
        }
    }

    pub fn finish_drill(&self) -> KiasResult<DrillReport> {
        let mut current = self.current_execution.write().unwrap();
        
        let mut execution = current.take()
            .ok_or_else(|| KiasError::SecurityDrill("No active drill execution".to_string()))?;
        
        let scenario = self.scenarios.get(&execution.scenario_id)
            .ok_or_else(|| KiasError::SecurityDrill("Scenario not found".to_string()))?;
        
        execution.calculate_score(&scenario.success_criteria);
        
        let recommendations = self.generate_recommendations(&execution, scenario);
        
        let report = DrillReport {
            drill_id: format!("{}_{}", scenario.scenario_type.as_str(), chrono::Utc::now().timestamp()),
            scenario_type: scenario.scenario_type,
            execution,
            recommendations,
            generated_at: chrono::Utc::now(),
        };
        
        if let Ok(mut executions) = self.executions.write() {
            executions.push(report.execution.clone());
        }
        
        Ok(report)
    }

    fn generate_recommendations(&self, execution: &DrillExecution, scenario: &DrillScenario) -> Vec<DrillRecommendation> {
        let mut recs = Vec::new();
        
        if execution.status == DrillStatus::Failed || execution.status == DrillStatus::PartialPass {
            let missing_controls: Vec<_> = scenario.success_criteria.required_controls.iter()
                .filter(|c| !execution.detections.iter().any(|d| d.detection_type.contains(&**c)))
                .collect();
            
            if !missing_controls.is_empty() {
                recs.push(DrillRecommendation {
                    priority: 1,
                    category: "Detection".to_string(),
                    recommendation: format!("Implement missing controls: {:?}", missing_controls),
                    effort: "MEDIUM",
                    impact: "HIGH",
                });
            }
        }
        
        let avg_response = if execution.response_times.is_empty() {
            0.0
        } else {
            execution.response_times.values().sum::<u32>() as f64 / execution.response_times.len() as f64
        };
        
        if avg_response > scenario.success_criteria.max_response_time_seconds as f64 * 1000.0 {
            recs.push(DrillRecommendation {
                priority: 2,
                category: "Performance".to_string(),
                recommendation: "Reduce detection response time to meet SLA".to_string(),
                effort: "LOW",
                impact: "MEDIUM",
            });
        }
        
        recs.push(DrillRecommendation {
            priority: 3,
            category: "Documentation".to_string(),
            recommendation: "Update runbook with drill findings".to_string(),
            effort: "LOW",
            impact: "LOW",
        });
        
        recs
    }

    pub fn get_all_executions(&self) -> Vec<DrillExecution> {
        self.executions.read().map(|g| g.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drill_runner_creation() {
        let runner = DrillRunner::new();
        let scenarios = runner.list_scenarios();
        assert!(!scenarios.is_empty());
    }

    #[test]
    fn test_get_scenario() {
        let runner = DrillRunner::new();
        let scenario = runner.get_scenario("data_leakage_001");
        assert!(scenario.is_some());
        assert_eq!(scenario.unwrap().scenario_type, DrillScenarioType::DataLeakage);
    }

    #[test]
    fn test_start_and_finish_drill() {
        let runner = DrillRunner::new();
        
        let execution = runner.start_drill("data_leakage_001").unwrap();
        assert_eq!(execution.status, DrillStatus::Pending);
        
        runner.record_step_result(1, "PII_MASKING", true, 100).unwrap();
        runner.record_step_result(2, "ANOMALY_DETECTION", true, 200).unwrap();
        
        let report = runner.finish_drill().unwrap();
        assert!(report.execution.end_time.is_some());
    }

    #[test]
    fn test_drill_execution_score_pass() {
        let runner = DrillRunner::new();
        
        runner.start_drill("data_leakage_001").unwrap();
        runner.record_step_result(1, "PII_MASKING", true, 100).unwrap();
        runner.record_step_result(2, "ANOMALY_DETECTION", true, 200).unwrap();
        
        let report = runner.finish_drill().unwrap();
        assert!(matches!(report.execution.status, DrillStatus::Passed | DrillStatus::PartialPass));
    }

    #[test]
    fn test_drill_execution_score_fail() {
        let runner = DrillRunner::new();
        
        runner.start_drill("priv_esc_001").unwrap();
        runner.record_step_result(1, "AUTHZ_CHECK", false, 5000).unwrap();
        
        let report = runner.finish_drill().unwrap();
        assert!(matches!(report.execution.status, DrillStatus::Failed | DrillStatus::PartialPass));
    }

    #[test]
    fn test_drill_recommendations() {
        let runner = DrillRunner::new();
        
        runner.start_drill("data_leakage_001").unwrap();
        runner.record_step_result(1, "OTHER_CONTROL", true, 100).unwrap();
        
        let report = runner.finish_drill().unwrap();
        assert!(!report.recommendations.is_empty());
    }

    #[test]
    fn test_executions_tracking() {
        let runner = DrillRunner::new();
        
        runner.start_drill("data_leakage_001").unwrap();
        runner.record_step_result(1, "PII_MASKING", true, 100).unwrap();
        runner.finish_drill().unwrap();
        
        runner.start_drill("priv_esc_001").unwrap();
        runner.record_step_result(1, "AUTHZ_CHECK", true, 100).unwrap();
        runner.finish_drill().unwrap();
        
        let executions = runner.get_all_executions();
        assert_eq!(executions.len(), 2);
    }

    #[test]
    fn test_scenario_types() {
        assert_eq!(DrillScenarioType::DataLeakage.as_str(), "DATA_LEAKAGE");
        assert_eq!(DrillScenarioType::Ransomware.as_str(), "RANSOMWARE");
        assert_eq!(DrillScenarioType::SupplyChainContamination.as_str(), "SUPPLY_CHAIN_CONTAMINATION");
    }

    #[test]
    fn test_severity_levels() {
        assert_eq!(DrillSeverity::Critical.as_str(), "CRITICAL");
        assert_eq!(DrillSeverity::Low.as_str(), "LOW");
    }
}
