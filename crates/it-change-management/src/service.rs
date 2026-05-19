//! IT变更管理系统 HTTP API 服务
//!
//! 提供RESTful API接口，用于生产环境部署

use crate::storage::ChangeStorage;
use crate::*;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// API服务配置
pub struct ApiServiceConfig {
    pub host: String,
    pub port: u16,
    pub db_path: String,
}

impl Default for ApiServiceConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            db_path: "/data/changes.db".to_string(),
        }
    }
}

/// API服务状态
pub struct ApiService {
    config: ApiServiceConfig,
    manager: Arc<Mutex<ItChangeManager>>,
    storage: Arc<ChangeStorage>,
}

impl ApiService {
    /// 创建新的API服务
    pub fn new(config: ApiServiceConfig) -> Result<Self, String> {
        let storage = ChangeStorage::new(Path::new(&config.db_path))
            .map_err(|e| format!("Failed to create storage: {}", e))?;

        Ok(Self {
            config,
            manager: Arc::new(Mutex::new(ItChangeManager::new())),
            storage: Arc::new(storage),
        })
    }

    /// 获取服务地址
    pub fn address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }

    /// 获取变更管理器引用
    pub fn manager(&self) -> &Arc<Mutex<ItChangeManager>> {
        &self.manager
    }

    /// 获取存储引用
    pub fn storage(&self) -> &Arc<ChangeStorage> {
        &self.storage
    }
}

/// API路由定义
pub mod routes {
    use super::*;
    use crate::api::*;

    /// 创建变更请求
    pub fn create_change(
        manager: &mut ItChangeManager,
        request: CreateChangeRequest,
    ) -> Result<ItChangeRequest, String> {
        Ok(manager.create_change_request(
            request.title,
            request.description,
            request.change_type,
            request.change_category,
            request.risk_level,
            request.requester,
            request.requester_department,
            request.rollback_plan,
            request.implementation_plan,
            request.impact_assessment,
        ))
    }

    /// 提交审批
    pub fn submit_for_review(
        manager: &mut ItChangeManager,
        change_id: &str,
        submitter: &str,
    ) -> Result<(), String> {
        manager.submit_for_review(change_id, submitter, None, None)
    }

    /// 审批变更
    pub fn approve_change(
        manager: &mut ItChangeManager,
        change_id: &str,
        request: ApproveChangeRequest,
    ) -> Result<(), String> {
        let signature = ElectronicSignature {
            meaning: request.signature_meaning,
            signed_at: chrono::Utc::now(),
            auth_factor1_hash: request.password_hash,
            auth_factor2_hash: request.token_hash,
            linked_record_id: change_id.to_string(),
            signer_name: request.signer_name,
            signer_title: request.signer_title,
        };

        manager.approve_change(
            change_id,
            &request.approver_id,
            request.decision,
            signature,
            None,
            None,
        )
    }

    /// 获取变更详情
    pub fn get_change(
        manager: &ItChangeManager,
        change_id: &str,
    ) -> Result<ItChangeRequest, String> {
        manager.get_change(change_id).cloned()
    }

    /// 获取变更列表
    pub fn list_changes(manager: &ItChangeManager) -> Vec<ItChangeRequest> {
        manager.list_changes().into_iter().cloned().collect()
    }

    /// 获取审计日志
    pub fn get_audit_log(manager: &ItChangeManager, change_id: &str) -> Vec<AuditEntry> {
        manager
            .get_audit_log(change_id)
            .into_iter()
            .cloned()
            .collect()
    }

    /// 获取统计数据
    pub fn get_statistics(manager: &ItChangeManager) -> ChangeStatistics {
        manager.get_statistics()
    }

    /// 实施变更
    pub fn implement_change(
        manager: &mut ItChangeManager,
        change_id: &str,
        implementer: &str,
    ) -> Result<(), String> {
        manager.implement_change(change_id, implementer, None, None)
    }

    /// 完成实施
    pub fn complete_implementation(
        manager: &mut ItChangeManager,
        change_id: &str,
        implementer: &str,
    ) -> Result<(), String> {
        manager.complete_implementation(change_id, implementer, None, None)
    }

    /// 验证变更
    pub fn verify_change(
        manager: &mut ItChangeManager,
        change_id: &str,
        verifier: &str,
    ) -> Result<(), String> {
        manager.verify_change(change_id, verifier, None, None)
    }

    /// 完成验证
    pub fn complete_verification(
        manager: &mut ItChangeManager,
        change_id: &str,
        verifier: &str,
    ) -> Result<(), String> {
        manager.complete_verification(change_id, verifier, None, None)
    }

    /// 关闭变更
    pub fn close_change(
        manager: &mut ItChangeManager,
        change_id: &str,
        closer: &str,
    ) -> Result<(), String> {
        manager.close_change(change_id, closer, None, None)
    }

    /// 回滚变更
    pub fn rollback_change(
        manager: &mut ItChangeManager,
        change_id: &str,
        rollback_by: &str,
        reason: &str,
    ) -> Result<(), String> {
        manager.rollback_change(change_id, rollback_by, reason, None, None)
    }

    /// 紧急实施
    pub fn emergency_implement(
        manager: &mut ItChangeManager,
        change_id: &str,
        implementer: &str,
        reason: &str,
    ) -> Result<(), String> {
        manager.emergency_implement(change_id, implementer, reason, None, None)
    }

    /// 触发CAPA
    pub fn trigger_capa(
        manager: &mut ItChangeManager,
        change_id: &str,
        request: TriggerCapaRequest,
    ) -> Result<String, String> {
        manager.trigger_capa(
            change_id,
            &request.triggered_by,
            request.title,
            request.description,
            None,
            None,
        )
    }

    /// 添加评论
    pub fn add_comment(
        manager: &mut ItChangeManager,
        change_id: &str,
        request: AddCommentRequest,
    ) -> Result<(), String> {
        manager.add_comment(
            change_id,
            &request.author,
            request.content,
            request.is_internal,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::*;

    #[test]
    fn test_api_service_creation() {
        let config = ApiServiceConfig {
            host: "127.0.0.1".to_string(),
            port: 9090,
            db_path: ":memory:".to_string(),
        };

        // 注意：这个测试只验证配置，不实际创建服务
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);
    }

    #[test]
    fn test_routes_create_change() {
        let mut manager = ItChangeManager::new();

        let request = CreateChangeRequest {
            title: "测试变更".to_string(),
            description: "测试描述".to_string(),
            change_type: ChangeType::Configuration,
            change_category: ChangeCategory::Normal,
            risk_level: RiskLevel::Low,
            requester: "test.user".to_string(),
            requester_department: "IT".to_string(),
            rollback_plan: "回滚".to_string(),
            implementation_plan: "实施".to_string(),
            impact_assessment: ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        };

        let change = routes::create_change(&mut manager, request).unwrap();
        assert_eq!(change.status, ChangeStatus::Draft);
    }

    #[test]
    fn test_routes_list_changes() {
        let manager = ItChangeManager::new();
        let changes = routes::list_changes(&manager);
        assert_eq!(changes.len(), 0);
    }

    fn create_test_request(title: &str) -> CreateChangeRequest {
        CreateChangeRequest {
            title: title.to_string(),
            description: "测试描述".to_string(),
            change_type: ChangeType::Configuration,
            change_category: ChangeCategory::Normal,
            risk_level: RiskLevel::Low,
            requester: "test.user".to_string(),
            requester_department: "IT".to_string(),
            rollback_plan: "回滚".to_string(),
            implementation_plan: "实施".to_string(),
            impact_assessment: ImpactAssessment {
                affected_systems: vec!["LIMS".to_string()],
                affected_users: vec!["QC".to_string()],
                downtime_estimate_minutes: 30,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        }
    }

    #[test]
    fn test_routes_submit_for_review() {
        let mut manager = ItChangeManager::new();
        let change = routes::create_change(&mut manager, create_test_request("提交测试")).unwrap();
        assert_eq!(change.status, ChangeStatus::Draft);

        let result = routes::submit_for_review(&mut manager, &change.id, "user");
        assert!(result.is_ok());

        let updated = routes::get_change(&manager, &change.id).unwrap();
        assert_eq!(updated.status, ChangeStatus::Submitted);
    }

    #[test]
    fn test_routes_get_change() {
        let mut manager = ItChangeManager::new();
        let change = routes::create_change(&mut manager, create_test_request("查询测试")).unwrap();

        let found = routes::get_change(&manager, &change.id);
        assert!(found.is_ok());
        assert_eq!(found.unwrap().title, "查询测试");

        let not_found = routes::get_change(&manager, "nonexistent");
        assert!(not_found.is_err());
    }

    #[test]
    fn test_routes_get_statistics() {
        let mut manager = ItChangeManager::new();
        routes::create_change(&mut manager, create_test_request("统计1")).unwrap();
        routes::create_change(&mut manager, create_test_request("统计2")).unwrap();

        let stats = routes::get_statistics(&manager);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.draft, 2);
    }

    #[test]
    fn test_routes_add_comment() {
        let mut manager = ItChangeManager::new();
        let change = routes::create_change(&mut manager, create_test_request("评论测试")).unwrap();

        let comment_request = AddCommentRequest {
            author: "reviewer".to_string(),
            content: "已审阅变更计划".to_string(),
            is_internal: false,
        };
        let result = routes::add_comment(&mut manager, &change.id, comment_request);
        assert!(result.is_ok());

        // Verify the change was updated (comments are stored on the change, not in audit log)
        let updated = routes::get_change(&manager, &change.id).unwrap();
        assert_eq!(updated.comments.len(), 1);
        assert_eq!(updated.comments[0].author, "reviewer");
        assert_eq!(updated.comments[0].content, "已审阅变更计划");
    }

    #[test]
    fn test_routes_full_lifecycle() {
        let mut manager = ItChangeManager::new();
        let change =
            routes::create_change(&mut manager, create_test_request("全流程测试")).unwrap();

        // Submit
        routes::submit_for_review(&mut manager, &change.id, "user").unwrap();

        // Add approver via manager
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人".to_string(),
                "IT经理".to_string(),
            )
            .unwrap();

        // Approve
        let approve_req = ApproveChangeRequest {
            approver_id: "approver1".to_string(),
            decision: Decision::Approved,
            signature_meaning: SignatureMeaning::Approval,
            password_hash: "hash1".to_string(),
            token_hash: "hash2".to_string(),
            signer_name: "审批人".to_string(),
            signer_title: "IT经理".to_string(),
        };
        routes::approve_change(&mut manager, &change.id, approve_req).unwrap();

        let approved = routes::get_change(&manager, &change.id).unwrap();
        assert_eq!(approved.status, ChangeStatus::Approved);

        // Implement
        routes::implement_change(&mut manager, &change.id, "implementer").unwrap();
        routes::complete_implementation(&mut manager, &change.id, "implementer").unwrap();

        // Verify
        routes::verify_change(&mut manager, &change.id, "verifier").unwrap();
        routes::complete_verification(&mut manager, &change.id, "verifier").unwrap();

        // Close
        routes::close_change(&mut manager, &change.id, "closer").unwrap();

        let closed = routes::get_change(&manager, &change.id).unwrap();
        assert_eq!(closed.status, ChangeStatus::Closed);
        assert!(closed.closed_at.is_some());

        // Verify audit trail exists
        let audit = routes::get_audit_log(&manager, &change.id);
        assert!(audit.len() >= 6); // submit + approve + implement + complete_impl + verify + close
    }
}
