//! IT变更管理系统演示
//!
//! 模拟一个完整的医药企业IT变更管理流程

use chrono::Utc;
use kias_it_change_management::*;

fn main() {
    println!("=== AgentGuard IT变更管理系统演示 ===\n");

    // 创建变更管理器
    let mut manager = ItChangeManager::new();

    // 场景1: LIMS系统升级
    println!("【场景1】LIMS系统升级变更");
    let lims_change = manager.create_change_request(
        "升级LIMS到v3.0".to_string(),
        "将LIMS系统从v2.5升级到v3.0，新增数据完整性检查功能".to_string(),
        ChangeType::Application,
        ChangeCategory::Normal,
        RiskLevel::High,
        "zhang.it".to_string(),
        "IT部门".to_string(),
        "回滚到v2.5备份版本".to_string(),
        "1. 完整备份数据库\n2. 停止LIMS服务\n3. 安装v3.0\n4. 迁移数据\n5. 启动服务\n6. 验证功能"
            .to_string(),
        ImpactAssessment {
            affected_systems: vec!["LIMS".to_string(), "MES".to_string()],
            affected_users: vec!["QC部门".to_string(), "生产部门".to_string()],
            downtime_estimate_minutes: 120,
            risk_mitigation: vec!["完整备份".to_string(), "回滚计划".to_string()],
            testing_requirements: vec!["功能测试".to_string(), "性能测试".to_string()],
            gxp_impact: GxpImpact::Direct,
            requires_csv_validation: true,
            affects_data_integrity: true,
        },
    );
    println!("  变更编号: {}", lims_change.change_number);
    println!("  风险等级: {:?}", lims_change.risk_level);
    println!("  GxP影响: 直接影响\n");

    // 提交审批
    manager
        .submit_for_review(&lims_change.id, "zhang.it", None, None)
        .unwrap();
    println!("  → 已提交审批");

    // 添加审批人
    manager
        .add_approver(
            &lims_change.id,
            "qa.head".to_string(),
            "QA主管".to_string(),
            "QA审批".to_string(),
        )
        .unwrap();
    manager
        .add_approver(
            &lims_change.id,
            "it.manager".to_string(),
            "IT经理".to_string(),
            "IT审批".to_string(),
        )
        .unwrap();
    println!("  → 已添加审批人: QA主管, IT经理");

    // QA主管审批
    let qa_sig = ElectronicSignature {
        meaning: SignatureMeaning::Approval,
        signed_at: Utc::now(),
        auth_factor1_hash: sha256_hash("qa_password"),
        auth_factor2_hash: sha256_hash("qa_token"),
        linked_record_id: lims_change.id.clone(),
        signer_name: "李四".to_string(),
        signer_title: "QA主管".to_string(),
    };
    manager
        .approve_change(
            &lims_change.id,
            "qa.head",
            Decision::Approved,
            qa_sig,
            None,
            None,
        )
        .unwrap();
    println!("  → QA主管已审批");

    // IT经理审批
    let it_sig = ElectronicSignature {
        meaning: SignatureMeaning::Approval,
        signed_at: Utc::now(),
        auth_factor1_hash: sha256_hash("it_password"),
        auth_factor2_hash: sha256_hash("it_token"),
        linked_record_id: lims_change.id.clone(),
        signer_name: "王五".to_string(),
        signer_title: "IT经理".to_string(),
    };
    manager
        .approve_change(
            &lims_change.id,
            "it.manager",
            Decision::Approved,
            it_sig,
            None,
            None,
        )
        .unwrap();
    println!("  → IT经理已审批");
    println!("  → 变更已批准\n");

    // 实施变更
    manager
        .implement_change(&lims_change.id, "zhang.it", None, None)
        .unwrap();
    println!("  → 开始实施...");
    manager
        .complete_implementation(&lims_change.id, "zhang.it", None, None)
        .unwrap();
    println!("  → 实施完成");

    // 验证
    manager
        .verify_change(&lims_change.id, "qa.tester", None, None)
        .unwrap();
    println!("  → 开始验证...");
    manager
        .complete_verification(&lims_change.id, "qa.tester", None, None)
        .unwrap();
    println!("  → 验证通过");

    // 关闭
    manager
        .close_change(&lims_change.id, "zhang.it", None, None)
        .unwrap();
    println!("  → 变更已关闭\n");

    // 场景2: 紧急安全补丁
    println!("【场景2】紧急安全补丁");
    let security_change = manager.create_change_request(
        "紧急修复CVE-2026-1234".to_string(),
        "Linux内核提权漏洞，需要立即修补".to_string(),
        ChangeType::Security,
        ChangeCategory::Emergency,
        RiskLevel::Critical,
        "ops.lead".to_string(),
        "运维部门".to_string(),
        "回滚内核版本".to_string(),
        "1. 评估影响范围\n2. 测试补丁兼容性\n3. 分批部署".to_string(),
        ImpactAssessment {
            affected_systems: vec!["所有Linux服务器".to_string()],
            affected_users: vec!["全体员工".to_string()],
            downtime_estimate_minutes: 30,
            risk_mitigation: vec!["分批部署".to_string(), "回滚计划".to_string()],
            testing_requirements: vec!["补丁兼容性测试".to_string()],
            gxp_impact: GxpImpact::Indirect,
            requires_csv_validation: false,
            affects_data_integrity: false,
        },
    );
    println!("  变更编号: {}", security_change.change_number);
    println!("  类别: 紧急变更");

    // 紧急实施（跳过审批）
    manager
        .submit_for_review(&security_change.id, "ops.lead", None, None)
        .unwrap();
    manager
        .emergency_implement(
            &security_change.id,
            "ops.lead",
            "高危漏洞，生产系统面临风险",
            None,
            None,
        )
        .unwrap();
    println!("  → 紧急实施完成（事后补充审批）");
    println!("  → 事后审批截止: 72小时内\n");

    // 场景3: CAPA联动
    println!("【场景3】CAPA联动");
    let capa_change = manager.create_change_request(
        "更新检测阈值".to_string(),
        "调整LIMS样品检测阈值参数".to_string(),
        ChangeType::Configuration,
        ChangeCategory::Normal,
        RiskLevel::Medium,
        "qa.engineer".to_string(),
        "QA部门".to_string(),
        "恢复原阈值".to_string(),
        "1. 修改配置文件\n2. 重启服务".to_string(),
        ImpactAssessment {
            affected_systems: vec!["LIMS".to_string()],
            affected_users: vec!["QC部门".to_string()],
            downtime_estimate_minutes: 15,
            risk_mitigation: vec!["备份原配置".to_string()],
            testing_requirements: vec!["验证新阈值".to_string()],
            gxp_impact: GxpImpact::Direct,
            requires_csv_validation: false,
            affects_data_integrity: true,
        },
    );

    // 验证过程中发现问题
    let capa_id = manager
        .trigger_capa(
            &capa_change.id,
            "qa.inspector",
            "阈值超出验证范围".to_string(),
            "新阈值超出了验证时测试的范围，需要重新验证".to_string(),
            None,
            None,
        )
        .unwrap();
    println!("  CAPA已触发: {}", capa_id);
    println!("  原因: 阈值超出验证范围\n");

    // 输出统计
    let stats = manager.get_statistics();
    println!("=== 统计数据 ===");
    println!("  总变更数: {}", stats.total);
    println!("  草稿: {}", stats.draft);
    println!("  已关闭: {}", stats.closed);
    println!("  紧急实施: {}", stats.emergency_implemented);
    println!("  SLA违规: {}", stats.sla_violations);

    // 输出审计日志
    println!("\n=== 审计日志（LIMS升级）===");
    let audit_log = manager.get_audit_log(&lims_change.id);
    for entry in &audit_log {
        println!("  [{:?}] {} - {}", entry.action, entry.actor, entry.detail);
    }

    println!("\n演示完成！");
}

fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}
