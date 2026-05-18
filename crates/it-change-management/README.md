# KIAS IT Change Management

医药/医疗器械企业IT系统变更管理模块，符合 FDA 21 CFR Part 11, EU Annex 11, GAMP 5。

## 功能特性

- ✅ 变更请求管理（CRUD）
- ✅ 审批工作流（9状态机）
- ✅ 电子签名（TOTP二因素认证）
- ✅ 审计追踪（SHA-256哈希链）
- ✅ 风险评估
- ✅ 回滚计划
- ✅ 验证确认

## 合规标准

| 标准 | 功能 |
|------|------|
| FDA 21 CFR Part 11 §11.10 | 审计追踪、电子签名 |
| FDA 21 CFR Part 11 §11.50 | 电子签名含义记录 |
| FDA 21 CFR Part 11 §11.70 | 签名与记录绑定 |
| FDA 21 CFR Part 11 §11.100 | 电子签名唯一性 |
| EU Annex 11 Clause 9 | 变更控制 |
| EU Annex 11 Clause 10 | 定期审查 |
| GAMP 5 | 风险评估、验证确认 |

## 快速开始

```rust
use kias_it_change_management::*;

fn main() {
    let mut manager = ItChangeManager::new();
    
    // 创建变更请求
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
    
    // 提交审批
    manager.submit_for_review(&change.id, "zhang.qa").unwrap();
    
    // 添加审批人
    manager.add_approver(
        &change.id,
        "approver1".to_string(),
        "审批人1".to_string(),
        "QA主管".to_string(),
    ).unwrap();
    
    // 审批通过
    manager.approve_change(
        &change.id,
        "approver1",
        Decision::Approved,
        "signature123".to_string(),
    ).unwrap();
    
    // 实施变更
    manager.implement_change(&change.id, "zhang.qa").unwrap();
    manager.complete_implementation(&change.id, "zhang.qa").unwrap();
    
    // 验证变更
    manager.verify_change(&change.id, "zhang.qa").unwrap();
    manager.complete_verification(&change.id, "zhang.qa").unwrap();
    
    // 关闭变更
    manager.close_change(&change.id, "zhang.qa").unwrap();
    
    // 获取审计日志
    let audit_log = manager.get_audit_log(&change.id);
    println!("审计日志: {:?}", audit_log);
}
```

## API端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/changes` | GET | 获取变更列表 |
| `/api/v1/changes` | POST | 创建变更请求 |
| `/api/v1/changes/:id` | GET | 获取变更详情 |
| `/api/v1/changes/:id/submit` | POST | 提交审批 |
| `/api/v1/changes/:id/approvers` | POST | 添加审批人 |
| `/api/v1/changes/:id/approve` | POST | 审批变更 |
| `/api/v1/changes/:id/implement` | POST | 实施变更 |
| `/api/v1/changes/:id/complete-implementation` | POST | 完成实施 |
| `/api/v1/changes/:id/verify` | POST | 验证变更 |
| `/api/v1/changes/:id/complete-verification` | POST | 完成验证 |
| `/api/v1/changes/:id/close` | POST | 关闭变更 |
| `/api/v1/changes/:id/rollback` | POST | 回滚变更 |
| `/api/v1/changes/:id/audit` | GET | 获取审计日志 |

## 审批工作流

```
Draft → Submitted → UnderReview → Approved → Implementing → Implemented → Verifying → Verified → Closed
                                    ↓
                                 Rejected
```

## 适用场景

1. **系统配置变更** — 修改服务器配置、网络配置、应用配置
2. **软件部署变更** — 部署新版本软件、补丁更新
3. **硬件变更** — 服务器扩容、网络设备更换
4. **安全变更** — 防火墙规则、访问控制变更
5. **数据变更** — 数据库结构变更、数据迁移

## 适用行业

- 制药企业
- 医疗器械企业
- 生物技术企业
- 金融企业
- 其他受监管行业

## 许可证

MIT License
