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
