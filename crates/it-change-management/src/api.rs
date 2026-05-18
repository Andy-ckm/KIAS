//! # IT Change Management API
//!
//! RESTful API 端点，用于 IT 变更管理系统的外部集成

use crate::linux_auto::ComplianceReport;
use crate::*;
use serde::{Deserialize, Serialize};

/// API 请求：创建变更请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateChangeRequest {
    pub title: String,
    pub description: String,
    pub change_type: ChangeType,
    pub change_category: ChangeCategory,
    pub risk_level: RiskLevel,
    pub requester: String,
    pub requester_department: String,
    pub rollback_plan: String,
    pub implementation_plan: String,
    pub impact_assessment: ImpactAssessment,
}

/// API 请求：审批变更
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveChangeRequest {
    pub approver_id: String,
    pub decision: Decision,
    pub signature_meaning: SignatureMeaning,
    pub password_hash: String,
    pub token_hash: String,
    pub signer_name: String,
    pub signer_title: String,
}

/// API 请求：添加评论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCommentRequest {
    pub author: String,
    pub content: String,
    pub is_internal: bool,
}

/// API 请求：触发 CAPA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCapaRequest {
    pub triggered_by: String,
    pub title: String,
    pub description: String,
}

/// API 响应：变更详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResponse {
    pub success: bool,
    pub data: Option<ItChangeRequest>,
    pub error: Option<String>,
}

/// API 响应：变更列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeListResponse {
    pub success: bool,
    pub data: Vec<ItChangeRequest>,
    pub total: usize,
}

/// API 响应：审计日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogResponse {
    pub success: bool,
    pub data: Vec<AuditEntry>,
    pub chain_integrity: bool,
}

/// API 响应：统计数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResponse {
    pub success: bool,
    pub data: ChangeStatistics,
}

/// API 响应：通用成功
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

/// API 响应：CAPA
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapaResponse {
    pub success: bool,
    pub capa_id: Option<String>,
    pub error: Option<String>,
}

/// API 响应：Linux 自动化
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationResponse {
    pub success: bool,
    pub command: String,
    pub task_id: Option<String>,
}

/// API 响应：合规报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReportResponse {
    pub success: bool,
    pub report: Option<ComplianceReport>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ChangeCategory, ChangeType, Decision, GxpImpact, ImpactAssessment, RiskLevel,
        SignatureMeaning,
    };

    #[test]
    fn test_create_change_request_serde_roundtrip() {
        let req = CreateChangeRequest {
            title: "升级LIMS".to_string(),
            description: "升级到v3.0".to_string(),
            change_type: ChangeType::Application,
            change_category: ChangeCategory::Normal,
            risk_level: RiskLevel::Medium,
            requester: "admin".to_string(),
            requester_department: "IT".to_string(),
            rollback_plan: "回滚到v2.0".to_string(),
            implementation_plan: "逐步升级".to_string(),
            impact_assessment: ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 30,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: CreateChangeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, req.title);
        assert_eq!(deserialized.change_type, req.change_type);
    }

    #[test]
    fn test_approve_change_request_serde_roundtrip() {
        let req = ApproveChangeRequest {
            approver_id: "approver-1".to_string(),
            decision: Decision::Approved,
            signature_meaning: SignatureMeaning::Approval,
            password_hash: "hash123".to_string(),
            token_hash: "token456".to_string(),
            signer_name: "张三".to_string(),
            signer_title: "QA经理".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: ApproveChangeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.approver_id, req.approver_id);
        assert_eq!(deserialized.decision, req.decision);
        assert_eq!(deserialized.signer_name, req.signer_name);
    }

    #[test]
    fn test_change_response_serde_roundtrip() {
        let resp = ChangeResponse {
            success: true,
            data: None,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: ChangeResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert!(deserialized.data.is_none());
    }

    #[test]
    fn test_api_response_serde() {
        let resp = ApiResponse {
            success: true,
            message: "操作成功".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("操作成功"));
        let deserialized: ApiResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
    }

    #[test]
    fn test_stats_response_serde() {
        let resp = StatsResponse {
            success: true,
            data: crate::ChangeStatistics {
                total: 10,
                draft: 2,
                submitted: 3,
                under_review: 0,
                approved: 1,
                implementing: 1,
                implemented: 1,
                verifying: 0,
                verified: 1,
                closed: 1,
                rejected: 0,
                rolled_back: 0,
                emergency_implemented: 0,
                low_risk: 3,
                medium_risk: 4,
                high_risk: 2,
                critical_risk: 1,
                sla_violations: 0,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: StatsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.data.total, 10);
    }
}
