# AgentGuard IT 变更管理系统

> 医药/医疗器械企业IT系统变更管理
> 符合 FDA 21 CFR Part 11, EU Annex 11, GAMP 5

## 核心特性

- **GxP影响分级**：直接影响/间接影响/无影响
- **紧急变更通道**：事后补充审批，符合FDA要求
- **CAPA联动**：变更中发现问题自动触发CAPA
- **电子签名**：符合21 CFR Part 11 §11.50/§11.70/§11.100/§11.200
- **审计追踪**：SHA-256哈希链，不可篡改
- **SLA跟踪**：超时自动升级
- **SQLite持久化**：完整数据存储
- **Linux自动化**：Ansible/OpenSCAP/Lynis集成

## 快速开始

```bash
# 运行演示
cargo run -p kias-it-change-management --example demo

# 运行测试
cargo test -p kias-it-change-management
```

## 使用示例

```rust
use kias_it_change_management::*;

let mut manager = ItChangeManager::new();

// 创建变更
let change = manager.create_change_request(
    "升级LIMS".to_string(),
    "将LIMS升级到v3.0".to_string(),
    ChangeType::Application,
    ChangeCategory::Normal,
    RiskLevel::High,
    "it.admin".to_string(),
    "IT部门".to_string(),
    "回滚计划".to_string(),
    "实施计划".to_string(),
    impact_assessment,
);

// 完整流程
manager.submit_for_review(&change.id, "it.admin", None, None).unwrap();
manager.add_approver(&change.id, "qa.head".to_string(), "QA主管".to_string(), "QA".to_string()).unwrap();
manager.approve_change(&change.id, "qa.head", Decision::Approved, signature, None, None).unwrap();
manager.implement_change(&change.id, "it.admin", None, None).unwrap();
manager.complete_implementation(&change.id, "it.admin", None, None).unwrap();
manager.verify_change(&change.id, "qa.tester", None, None).unwrap();
manager.complete_verification(&change.id, "qa.tester", None, None).unwrap();
manager.close_change(&change.id, "it.admin", None, None).unwrap();
```

## 合规标准

| 标准 | 要求 | AgentGuard实现 |
|------|------|----------|
| FDA 21 CFR Part 11 §11.10(e) | 审计追踪 | SHA-256哈希链 |
| FDA 21 CFR Part 11 §11.50 | 电子签名含义 | SignatureMeaning枚举 |
| FDA 21 CFR Part 11 §11.70 | 签名唯一性 | 双因子认证 |
| GAMP 5 | 验证级别 | IQ/OQ/PQ |
| CIS Benchmark | Linux安全基线 | Ansible playbook |

## 数据模型

```
变更状态机:
Draft → Submitted → UnderReview → Approved → Implementing → Implemented → Verifying → Verified → Closed
                    ↓
                 Rejected
                    
紧急变更:
Submitted → EmergencyImplemented → Verifying → Verified → Closed
```

## 代码统计

| 文件 | 行数 | 功能 |
|------|------|------|
| lib.rs | 2,513 | 核心业务逻辑 |
| storage.rs | 1,469 | SQLite持久化 |
| linux_auto.rs | 669 | Linux自动化 |
| api.rs | 223 | API数据结构 |
| demo.rs | 192 | 演示程序 |
| **总计** | **5,066** | |

## 测试覆盖

- 23个单元测试全部通过
- 覆盖：完整变更生命周期、电子签名、紧急变更、CAPA触发、审计链完整性、SLA违规、持久化读写、Linux自动化命令生成

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
- **AgentGuard优势**：开源、合规、可定制、Rust高性能

## 文档

- [使用指南](USAGE.md)
- [IT变更管理研究](../../docs/research/it-change-management-research.md)
- [Linux自动化研究](../../docs/research/linux-automation-research.md)
- [文档处理研究](../../docs/research/document-management-research.md)
