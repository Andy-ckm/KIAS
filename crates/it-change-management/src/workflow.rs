//! 变更审批工作流引擎
//! 基于状态机的多级审批流程

use serde::{Deserialize, Serialize};

/// 审批工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalWorkflow {
    pub id: String,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
    pub escalation_rules: Vec<EscalationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub step_id: u32,
    pub name: String,
    pub approver_role: String,
    pub required: bool,
    pub timeout_hours: Option<u32>,
    pub auto_approve_on_timeout: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub trigger_hours: u32,
    pub escalate_to: String,
    pub notification_method: String,
}

/// 工作流引擎
pub struct WorkflowEngine;

impl WorkflowEngine {
    /// 创建标准变更审批流程（3级）
    pub fn standard_approval() -> ApprovalWorkflow {
        ApprovalWorkflow {
            id: "wf-standard-001".into(),
            name: "标准变更审批流程".into(),
            steps: vec![
                WorkflowStep {
                    step_id: 1,
                    name: "部门主管审批".into(),
                    approver_role: "dept_manager".into(),
                    required: true,
                    timeout_hours: Some(24),
                    auto_approve_on_timeout: false,
                },
                WorkflowStep {
                    step_id: 2,
                    name: "IT经理审批".into(),
                    approver_role: "it_manager".into(),
                    required: true,
                    timeout_hours: Some(48),
                    auto_approve_on_timeout: false,
                },
                WorkflowStep {
                    step_id: 3,
                    name: "QA审批".into(),
                    approver_role: "qa_manager".into(),
                    required: true,
                    timeout_hours: Some(72),
                    auto_approve_on_timeout: false,
                },
            ],
            escalation_rules: vec![EscalationRule {
                trigger_hours: 24,
                escalate_to: "it_director".into(),
                notification_method: "email".into(),
            }],
        }
    }

    /// 创建紧急变更审批流程（简化）
    pub fn emergency_approval() -> ApprovalWorkflow {
        ApprovalWorkflow {
            id: "wf-emergency-001".into(),
            name: "紧急变更审批流程".into(),
            steps: vec![
                WorkflowStep {
                    step_id: 1,
                    name: "值班经理审批".into(),
                    approver_role: "duty_manager".into(),
                    required: true,
                    timeout_hours: Some(1),
                    auto_approve_on_timeout: false,
                },
                WorkflowStep {
                    step_id: 2,
                    name: "事后补充审批".into(),
                    approver_role: "it_manager".into(),
                    required: true,
                    timeout_hours: Some(72),
                    auto_approve_on_timeout: false,
                },
            ],
            escalation_rules: vec![EscalationRule {
                trigger_hours: 1,
                escalate_to: "cto".into(),
                notification_method: "sms".into(),
            }],
        }
    }

    /// 验证工作流是否完整
    pub fn validate_workflow(workflow: &ApprovalWorkflow) -> Result<(), String> {
        if workflow.steps.is_empty() {
            return Err("工作流至少需要一个审批步骤".to_string());
        }
        let has_required = workflow.steps.iter().any(|s| s.required);
        if !has_required {
            return Err("工作流至少需要一个必填审批步骤".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_approval_steps() {
        let wf = WorkflowEngine::standard_approval();
        assert_eq!(wf.steps.len(), 3);
        assert_eq!(wf.id, "wf-standard-001");
    }

    #[test]
    fn test_standard_approval_step_roles() {
        let wf = WorkflowEngine::standard_approval();
        assert_eq!(wf.steps[0].approver_role, "dept_manager");
        assert_eq!(wf.steps[1].approver_role, "it_manager");
        assert_eq!(wf.steps[2].approver_role, "qa_manager");
    }

    #[test]
    fn test_standard_approval_all_required() {
        let wf = WorkflowEngine::standard_approval();
        assert!(wf.steps.iter().all(|s| s.required));
    }

    #[test]
    fn test_standard_approval_timeouts() {
        let wf = WorkflowEngine::standard_approval();
        assert_eq!(wf.steps[0].timeout_hours, Some(24));
        assert_eq!(wf.steps[1].timeout_hours, Some(48));
        assert_eq!(wf.steps[2].timeout_hours, Some(72));
    }

    #[test]
    fn test_standard_approval_escalation() {
        let wf = WorkflowEngine::standard_approval();
        assert_eq!(wf.escalation_rules.len(), 1);
        assert_eq!(wf.escalation_rules[0].escalate_to, "it_director");
    }

    #[test]
    fn test_emergency_approval_steps() {
        let wf = WorkflowEngine::emergency_approval();
        assert_eq!(wf.steps.len(), 2);
        assert_eq!(wf.id, "wf-emergency-001");
    }

    #[test]
    fn test_emergency_approval_fast_timeout() {
        let wf = WorkflowEngine::emergency_approval();
        assert_eq!(wf.steps[0].timeout_hours, Some(1));
        assert_eq!(wf.escalation_rules[0].trigger_hours, 1);
    }

    #[test]
    fn test_emergency_approval_escalates_to_cto() {
        let wf = WorkflowEngine::emergency_approval();
        assert_eq!(wf.escalation_rules[0].escalate_to, "cto");
        assert_eq!(wf.escalation_rules[0].notification_method, "sms");
    }

    #[test]
    fn test_validate_standard_workflow_ok() {
        let wf = WorkflowEngine::standard_approval();
        assert!(WorkflowEngine::validate_workflow(&wf).is_ok());
    }

    #[test]
    fn test_validate_empty_steps_fails() {
        let wf = ApprovalWorkflow {
            id: "test".into(),
            name: "empty".into(),
            steps: vec![],
            escalation_rules: vec![],
        };
        assert!(WorkflowEngine::validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_validate_no_required_steps_fails() {
        let wf = ApprovalWorkflow {
            id: "test".into(),
            name: "no-required".into(),
            steps: vec![WorkflowStep {
                step_id: 1,
                name: "optional".into(),
                approver_role: "anyone".into(),
                required: false,
                timeout_hours: None,
                auto_approve_on_timeout: false,
            }],
            escalation_rules: vec![],
        };
        assert!(WorkflowEngine::validate_workflow(&wf).is_err());
    }

    #[test]
    fn test_validate_with_one_required_passes() {
        let wf = ApprovalWorkflow {
            id: "test".into(),
            name: "minimal".into(),
            steps: vec![
                WorkflowStep {
                    step_id: 1,
                    name: "optional".into(),
                    approver_role: "anyone".into(),
                    required: false,
                    timeout_hours: None,
                    auto_approve_on_timeout: false,
                },
                WorkflowStep {
                    step_id: 2,
                    name: "required".into(),
                    approver_role: "manager".into(),
                    required: true,
                    timeout_hours: Some(24),
                    auto_approve_on_timeout: false,
                },
            ],
            escalation_rules: vec![],
        };
        assert!(WorkflowEngine::validate_workflow(&wf).is_ok());
    }
}
