//! # IT Change Management Module
//!
//! 医药/医疗器械企业IT系统变更管理模块
//! 符合 FDA 21 CFR Part 11, EU Annex 11, GAMP 5

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// IT变更请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItChangeRequest {
    pub id: String,
    pub title: String,
    pub description: String,
    pub change_type: ChangeType,
    pub risk_level: RiskLevel,
    pub status: ChangeStatus,
    pub requester: String,
    pub approvers: Vec<Approver>,
    pub impact_assessment: ImpactAssessment,
    pub rollback_plan: String,
    pub implementation_plan: String,
    pub verification_steps: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub implemented_at: Option<DateTime<Utc>>,
    pub verified_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

/// 变更类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Configuration, // 配置变更
    Software,      // 软件部署
    Hardware,      // 硬件变更
    Security,      // 安全变更
    Data,          // 数据变更
}

/// 风险等级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,      // 低风险
    Medium,   // 中风险
    High,     // 高风险
    Critical, // 关键风险
}

/// 变更状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeStatus {
    Draft,        // 草稿
    Submitted,    // 已提交
    UnderReview,  // 审核中
    Approved,     // 已批准
    Rejected,     // 已拒绝
    Implementing, // 实施中
    Implemented,  // 已实施
    Verifying,    // 验证中
    Verified,     // 已验证
    Closed,       // 已关闭
    RolledBack,   // 已回滚
}

/// 审批人
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Approver {
    pub user_id: String,
    pub name: String,
    pub role: String,
    pub decision: Option<Decision>,
    pub signed_at: Option<DateTime<Utc>>,
    pub signature: Option<String>,
}

/// 审批决策
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Decision {
    Approved,
    Rejected,
    RequestChanges,
}

/// 影响评估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    pub affected_systems: Vec<String>,
    pub affected_users: Vec<String>,
    pub downtime_estimate_minutes: u32,
    pub risk_mitigation: Vec<String>,
    pub testing_requirements: Vec<String>,
}

/// 审计日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub change_id: String,
    pub actor: String,
    pub action: AuditAction,
    pub detail: String,
    pub timestamp: DateTime<Utc>,
    pub previous_hash: String,
    pub hash: String,
}

/// 审计操作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuditAction {
    Created,
    Submitted,
    Reviewed,
    Approved,
    Rejected,
    Implemented,
    Verified,
    Closed,
    RolledBack,
    Signed,
}

/// IT变更管理器
pub struct ItChangeManager {
    changes: Vec<ItChangeRequest>,
    audit_log: Vec<AuditEntry>,
}

impl Default for ItChangeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ItChangeManager {
    /// 创建新的变更管理器
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            audit_log: Vec::new(),
        }
    }

    /// 创建变更请求
    #[allow(clippy::too_many_arguments)]
    pub fn create_change_request(
        &mut self,
        title: String,
        description: String,
        change_type: ChangeType,
        risk_level: RiskLevel,
        requester: String,
        rollback_plan: String,
        implementation_plan: String,
        impact_assessment: ImpactAssessment,
    ) -> ItChangeRequest {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        let change = ItChangeRequest {
            id: id.clone(),
            title,
            description,
            change_type,
            risk_level,
            status: ChangeStatus::Draft,
            requester,
            approvers: Vec::new(),
            impact_assessment,
            rollback_plan,
            implementation_plan,
            verification_steps: Vec::new(),
            created_at: now,
            updated_at: now,
            approved_at: None,
            implemented_at: None,
            verified_at: None,
            closed_at: None,
        };

        self.changes.push(change.clone());
        self.add_audit_entry(
            &id,
            &change.requester,
            AuditAction::Created,
            "变更请求已创建",
        );

        change
    }

    /// 提交审批
    pub fn submit_for_review(&mut self, change_id: &str, submitter: &str) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Draft {
            return Err("只有草稿状态的变更才能提交审批".to_string());
        }

        change.status = ChangeStatus::Submitted;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            submitter,
            AuditAction::Submitted,
            "变更已提交审批",
        );

        Ok(())
    }

    /// 添加审批人
    pub fn add_approver(
        &mut self,
        change_id: &str,
        user_id: String,
        name: String,
        role: String,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Submitted && change.status != ChangeStatus::UnderReview {
            return Err("只有已提交或审核中的变更才能添加审批人".to_string());
        }

        change.approvers.push(Approver {
            user_id,
            name,
            role,
            decision: None,
            signed_at: None,
            signature: None,
        });

        change.status = ChangeStatus::UnderReview;
        change.updated_at = Utc::now();

        Ok(())
    }

    /// 审批变更
    pub fn approve_change(
        &mut self,
        change_id: &str,
        approver_id: &str,
        decision: Decision,
        signature: String,
    ) -> Result<(), String> {
        // 先检查状态和更新审批人
        let (should_reject, should_approve) = {
            let change = self.get_change_mut(change_id)?;

            if change.status != ChangeStatus::UnderReview {
                return Err("只有审核中的变更才能审批".to_string());
            }

            // 找到审批人并更新决策
            let approver = change
                .approvers
                .iter_mut()
                .find(|a| a.user_id == approver_id)
                .ok_or("未找到该审批人")?;

            approver.decision = Some(decision.clone());
            approver.signed_at = Some(Utc::now());
            approver.signature = Some(signature);

            // 检查是否所有审批人都已审批
            let all_approved = change
                .approvers
                .iter()
                .all(|a| a.decision == Some(Decision::Approved));

            let any_rejected = change
                .approvers
                .iter()
                .any(|a| a.decision == Some(Decision::Rejected));

            (any_rejected, all_approved)
        };

        // 根据结果更新状态和添加审计日志
        if should_reject {
            let change = self.get_change_mut(change_id)?;
            change.status = ChangeStatus::Rejected;
            change.updated_at = Utc::now();
            self.add_audit_entry(
                change_id,
                approver_id,
                AuditAction::Rejected,
                "变更已被拒绝",
            );
        } else if should_approve {
            let change = self.get_change_mut(change_id)?;
            change.status = ChangeStatus::Approved;
            change.approved_at = Some(Utc::now());
            change.updated_at = Utc::now();
            self.add_audit_entry(change_id, approver_id, AuditAction::Approved, "变更已批准");
        } else {
            let change = self.get_change_mut(change_id)?;
            change.updated_at = Utc::now();
        }

        Ok(())
    }

    /// 实施变更
    pub fn implement_change(&mut self, change_id: &str, implementer: &str) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Approved {
            return Err("只有已批准的变更才能实施".to_string());
        }

        change.status = ChangeStatus::Implementing;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            implementer,
            AuditAction::Implemented,
            "变更开始实施",
        );

        Ok(())
    }

    /// 完成实施
    pub fn complete_implementation(
        &mut self,
        change_id: &str,
        implementer: &str,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implementing {
            return Err("只有实施中的变更才能完成实施".to_string());
        }

        change.status = ChangeStatus::Implemented;
        change.implemented_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            implementer,
            AuditAction::Implemented,
            "变更实施完成",
        );

        Ok(())
    }

    /// 验证变更
    pub fn verify_change(&mut self, change_id: &str, verifier: &str) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implemented {
            return Err("只有已实施的变更才能验证".to_string());
        }

        change.status = ChangeStatus::Verifying;
        change.updated_at = Utc::now();

        self.add_audit_entry(change_id, verifier, AuditAction::Verified, "变更开始验证");

        Ok(())
    }

    /// 完成验证
    pub fn complete_verification(&mut self, change_id: &str, verifier: &str) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Verifying {
            return Err("只有验证中的变更才能完成验证".to_string());
        }

        change.status = ChangeStatus::Verified;
        change.verified_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(change_id, verifier, AuditAction::Verified, "变更验证完成");

        Ok(())
    }

    /// 关闭变更
    pub fn close_change(&mut self, change_id: &str, closer: &str) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Verified {
            return Err("只有已验证的变更才能关闭".to_string());
        }

        change.status = ChangeStatus::Closed;
        change.closed_at = Some(Utc::now());
        change.updated_at = Utc::now();

        self.add_audit_entry(change_id, closer, AuditAction::Closed, "变更已关闭");

        Ok(())
    }

    /// 回滚变更
    pub fn rollback_change(
        &mut self,
        change_id: &str,
        rollback_by: &str,
        reason: &str,
    ) -> Result<(), String> {
        let change = self.get_change_mut(change_id)?;

        if change.status != ChangeStatus::Implementing
            && change.status != ChangeStatus::Implemented
            && change.status != ChangeStatus::Verifying
        {
            return Err("只有实施中、已实施或验证中的变更才能回滚".to_string());
        }

        change.status = ChangeStatus::RolledBack;
        change.updated_at = Utc::now();

        self.add_audit_entry(
            change_id,
            rollback_by,
            AuditAction::RolledBack,
            &format!("变更已回滚: {}", reason),
        );

        Ok(())
    }

    /// 获取变更详情
    pub fn get_change(&self, change_id: &str) -> Result<&ItChangeRequest, String> {
        self.changes
            .iter()
            .find(|c| c.id == change_id)
            .ok_or_else(|| format!("未找到变更请求: {}", change_id))
    }

    /// 获取变更列表
    pub fn list_changes(&self) -> Vec<&ItChangeRequest> {
        self.changes.iter().collect()
    }

    /// 按状态筛选变更
    pub fn list_changes_by_status(&self, status: &ChangeStatus) -> Vec<&ItChangeRequest> {
        self.changes
            .iter()
            .filter(|c| c.status == *status)
            .collect()
    }

    /// 获取变更的审计日志
    pub fn get_audit_log(&self, change_id: &str) -> Vec<&AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.change_id == change_id)
            .collect()
    }

    // 内部方法

    fn get_change_mut(&mut self, change_id: &str) -> Result<&mut ItChangeRequest, String> {
        self.changes
            .iter_mut()
            .find(|c| c.id == change_id)
            .ok_or_else(|| format!("未找到变更请求: {}", change_id))
    }

    fn add_audit_entry(&mut self, change_id: &str, actor: &str, action: AuditAction, detail: &str) {
        let now = Utc::now();
        let id = Uuid::new_v4().to_string();

        // 计算前一个哈希
        let previous_hash = self
            .audit_log
            .last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| "0".repeat(64));

        // 计算当前哈希
        let hash_input = format!(
            "{}{}{}{}{}{}",
            id,
            change_id,
            actor,
            serde_json::to_string(&action).unwrap_or_default(),
            detail,
            now.to_rfc3339()
        );
        let hash = sha256_hash(&hash_input);

        let entry = AuditEntry {
            id,
            change_id: change_id.to_string(),
            actor: actor.to_string(),
            action,
            detail: detail.to_string(),
            timestamp: now,
            previous_hash,
            hash,
        };

        self.audit_log.push(entry);
    }
}

/// SHA-256哈希函数
fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_change_request() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "更新LIMS配置".to_string(),
            "更新LIMS系统的样品检测阈值参数".to_string(),
            ChangeType::Configuration,
            RiskLevel::High,
            "zhang.qa".to_string(),
            "回滚到原配置文件".to_string(),
            "1. 停止LIMS服务\n2. 修改配置文件\n3. 重启服务".to_string(),
            ImpactAssessment {
                affected_systems: vec!["LIMS".to_string()],
                affected_users: vec!["QC部门".to_string()],
                downtime_estimate_minutes: 30,
                risk_mitigation: vec!["备份原配置".to_string()],
                testing_requirements: vec!["验证新阈值生效".to_string()],
            },
        );

        assert_eq!(change.status, ChangeStatus::Draft);
        assert_eq!(change.change_type, ChangeType::Configuration);
        assert_eq!(change.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_submit_for_review() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            RiskLevel::Low,
            "test.user".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
            },
        );

        let result = manager.submit_for_review(&change.id, "test.user");
        assert!(result.is_ok());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::Submitted);
    }

    #[test]
    fn test_approval_workflow() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            RiskLevel::Low,
            "test.user".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
            },
        );

        // 提交审批
        manager.submit_for_review(&change.id, "test.user").unwrap();

        // 添加审批人
        manager
            .add_approver(
                &change.id,
                "approver1".to_string(),
                "审批人1".to_string(),
                "QA主管".to_string(),
            )
            .unwrap();

        // 审批通过
        let result = manager.approve_change(
            &change.id,
            "approver1",
            Decision::Approved,
            "signature123".to_string(),
        );
        assert!(result.is_ok());

        let change = manager.get_change(&change.id).unwrap();
        assert_eq!(change.status, ChangeStatus::Approved);
    }

    #[test]
    fn test_audit_log() {
        let mut manager = ItChangeManager::new();

        let change = manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            RiskLevel::Low,
            "test.user".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
            },
        );

        let audit_log = manager.get_audit_log(&change.id);
        assert_eq!(audit_log.len(), 1);
        assert_eq!(audit_log[0].action, AuditAction::Created);
    }
}
