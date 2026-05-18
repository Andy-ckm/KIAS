# KIAS IT 变更管理系统 - 使用指南

> 医药/医疗器械企业IT系统变更管理
> 符合 FDA 21 CFR Part 11, EU Annex 11, GAMP 5

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
kias-it-change-management = { path = "crates/it-change-management" }
```

### 2. 基本使用

```rust
use kias_it_change_management::*;

fn main() {
    // 创建变更管理器
    let mut manager = ItChangeManager::new();

    // 创建变更请求
    let change = manager.create_change_request(
        "升级LIMS到v3.0".to_string(),
        "将LIMS系统从v2.5升级到v3.0".to_string(),
        ChangeType::Application,
        ChangeCategory::Normal,
        RiskLevel::High,
        "it.admin".to_string(),
        "IT部门".to_string(),
        "回滚到v2.5备份".to_string(),
        "1. 备份数据\n2. 停止服务\n3. 安装v3.0".to_string(),
        ImpactAssessment {
            affected_systems: vec!["LIMS".to_string()],
            affected_users: vec!["QC部门".to_string()],
            downtime_estimate_minutes: 120,
            risk_mitigation: vec!["完整备份".to_string()],
            testing_requirements: vec!["功能测试".to_string()],
            gxp_impact: GxpImpact::Direct,
            requires_csv_validation: true,
            affects_data_integrity: true,
        },
    );

    // 提交审批
    manager.submit_for_review(&change.id, "it.admin", None, None).unwrap();

    // 添加审批人
    manager.add_approver(
        &change.id,
        "qa.head".to_string(),
        "QA主管".to_string(),
        "QA审批".to_string(),
    ).unwrap();

    // 审批通过（带电子签名）
    let signature = ElectronicSignature {
        meaning: SignatureMeaning::Approval,
        signed_at: Utc::now(),
        auth_factor1_hash: sha256_hash("password123"),
        auth_factor2_hash: sha256_hash("token456"),
        linked_record_id: change.id.clone(),
        signer_name: "张三".to_string(),
        signer_title: "QA主管".to_string(),
    };

    manager.approve_change(
        &change.id,
        "qa.head",
        Decision::Approved,
        signature,
        None,
        None,
    ).unwrap();

    // 实施变更
    manager.implement_change(&change.id, "it.admin", None, None).unwrap();
    manager.complete_implementation(&change.id, "it.admin", None, None).unwrap();

    // 验证
    manager.verify_change(&change.id, "qa.tester", None, None).unwrap();
    manager.complete_verification(&change.id, "qa.tester", None, None).unwrap();

    // 关闭
    manager.close_change(&change.id, "it.admin", None, None).unwrap();

    // 获取审计日志
    let audit_log = manager.get_audit_log(&change.id);
    println!("审计日志条数: {}", audit_log.len());
}
```

### 3. 紧急变更

```rust
// 创建紧急变更
let change = manager.create_change_request(
    "紧急修复LIMS".to_string(),
    "LIMS系统宕机".to_string(),
    ChangeType::Infrastructure,
    ChangeCategory::Emergency,  // 紧急类别
    RiskLevel::Critical,
    "ops.lead".to_string(),
    "运维部门".to_string(),
    "恢复备份".to_string(),
    "紧急修复步骤".to_string(),
    impact_assessment,
);

// 提交后直接紧急实施（跳过审批）
manager.submit_for_review(&change.id, "ops.lead", None, None).unwrap();
manager.emergency_implement(
    &change.id,
    "ops.lead",
    "生产系统宕机，需要立即修复",
    None,
    None,
).unwrap();

// 事后补充审批（72小时内）
```

### 4. CAPA联动

```rust
// 验证过程中发现问题，触发CAPA
let capa_id = manager.trigger_capa(
    &change.id,
    "qa.inspector",
    "发现配置偏差".to_string(),
    "验证过程中发现配置参数超出预期范围".to_string(),
    None,
    None,
).unwrap();
```

### 5. SQLite持久化

```rust
use kias_it_change_management::storage::ChangeStorage;

// 创建持久化存储
let storage = ChangeStorage::new(std::path::Path::new("/data/changes.db")).unwrap();

// 保存变更
storage.save_change(&change).unwrap();

// 查询变更
let loaded = storage.get_change(&change.id).unwrap();

// 获取审计日志
let audit_log = storage.get_audit_log(&change.id).unwrap();

// 验证审计链完整性
let is_valid = storage.verify_audit_chain_integrity().unwrap();
```

### 6. Linux自动化

```rust
use kias_it_change_management::linux_auto::*;

// 创建配置
let config = LinuxAutomationConfig {
    playbook_dir: PathBuf::from("/etc/ansible/playbooks"),
    compliance_tool: ComplianceTool::OpenScap,
    target_hosts: vec!["192.168.1.10".to_string()],
    ssh_key_path: Some(PathBuf::from("/root/.ssh/id_rsa")),
    log_dir: PathBuf::from("/var/log/compliance"),
};

let manager = LinuxAutomationManager::new(config);

// 生成合规扫描命令
let cmd = manager.generate_openscap_command("192.168.1.10", "cis_level2");

// 生成Ansible命令
let task = AutomationTask::ComplianceScan {
    profile: "cis_level2".to_string(),
    hosts: vec!["192.168.1.10".to_string()],
};
let cmd = manager.generate_ansible_command(&task);
```

## 合规标准

### FDA 21 CFR Part 11

| 条款 | 要求 | KIAS实现 |
|------|------|----------|
| §11.10(e) | 审计追踪 | SHA-256哈希链，不可篡改 |
| §11.50 | 电子签名含义 | SignatureMeaning枚举 |
| §11.70 | 签名唯一性 | 双因子认证哈希 |
| §11.100 | 签名链接记录 | linked_record_id字段 |
| §11.200 | 签名要素 | 日期时间+双因子认证 |

### GAMP 5

| 验证级别 | 说明 | KIAS支持 |
|----------|------|----------|
| IQ | 安装确认 | ValidationLevel::InstallationQualification |
| OQ | 运行确认 | ValidationLevel::OperationalQualification |
| PQ | 性能确认 | ValidationLevel::PerformanceQualification |

## 数据模型

### 变更状态机

```
Draft → Submitted → UnderReview → Approved → Implementing → Implemented → Verifying → Verified → Closed
                    ↓
                 Rejected
                    
Emergency: Submitted → EmergencyImplemented → Verifying → Verified → Closed
```

### 风险分级

| 级别 | SLA | 审批要求 | 验证要求 |
|------|-----|---------|---------|
| Critical | 30天 | CAB+QA+生产+法规 | IQ/OQ/PQ |
| High | 14天 | CAB+QA | OQ/PQ |
| Medium | 7天 | 变更经理+QA | 回归测试 |
| Low | 3天 | 预审批 | 功能测试 |

## API数据结构

### CreateChangeRequest

```rust
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
```

### ApproveChangeRequest

```rust
pub struct ApproveChangeRequest {
    pub approver_id: String,
    pub decision: Decision,
    pub signature_meaning: SignatureMeaning,
    pub password_hash: String,
    pub token_hash: String,
    pub signer_name: String,
    pub signer_title: String,
}
```

## 测试

```bash
# 运行测试
cargo test -p kias-it-change-management

# 运行全量测试
cargo test --workspace
```

## 源码参考

| 项目 | Stars | 参考点 |
|------|-------|--------|
| Flowable | 9,266⭐ | BPMN工作流引擎 |
| GLPI | 5,893⭐ | ITSM/资产管理 |
| iTop | 1,115⭐ | ITIL全流程/CMDB |
| Ralph | 2,493⭐ | CMDB/资产生命周期 |

## 商业价值

- **市场空白**：没有专注医药GxP合规的开源变更管理平台
- **商业产品价格**：TrackWise/Veeva数十万美元起
- **KIAS优势**：开源、合规、可定制、Rust高性能
