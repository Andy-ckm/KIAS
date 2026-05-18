# 医药IT变更管理系统设计

> 目标：明天交付完整实现，可商用，可上生产，适配美敦力级别企业

---

## 一、场景分析

### 1.1 医药企业IT变更管理需求

医药/医疗器械企业的IT系统变更需要符合：
- **FDA 21 CFR Part 11** — 电子记录和电子签名
- **EU Annex 11** — 计算机化系统验证
- **GAMP 5** — 良好自动化制造实践
- **ITIL** — IT服务管理最佳实践

### 1.2 核心场景

1. **系统配置变更** — 修改服务器配置、网络配置、应用配置
2. **软件部署变更** — 部署新版本软件、补丁更新
3. **硬件变更** — 服务器扩容、网络设备更换
4. **安全变更** — 防火墙规则、访问控制变更
5. **数据变更** — 数据库结构变更、数据迁移

### 1.3 合规要求

| 要求 | 说明 |
|------|------|
| 可追溯性 | 每个变更都要有完整的审计追踪 |
| 电子签名 | 关键变更需要电子签名确认 |
| 审批流程 | 变更需要经过审批才能执行 |
| 风险评估 | 变更前需要进行风险评估 |
| 回滚计划 | 每个变更都要有回滚计划 |
| 验证确认 | 变更后需要验证确认 |

---

## 二、系统设计

### 2.1 数据模型

```rust
/// IT变更请求
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
}

/// 变更类型
pub enum ChangeType {
    Configuration,  // 配置变更
    Software,       // 软件部署
    Hardware,       // 硬件变更
    Security,       // 安全变更
    Data,           // 数据变更
}

/// 风险等级
pub enum RiskLevel {
    Low,      // 低风险
    Medium,   // 中风险
    High,     // 高风险
    Critical, // 关键风险
}

/// 变更状态
pub enum ChangeStatus {
    Draft,           // 草稿
    Submitted,       // 已提交
    UnderReview,     // 审核中
    Approved,        // 已批准
    Rejected,        // 已拒绝
    Implementing,    // 实施中
    Implemented,     // 已实施
    Verifying,       // 验证中
    Verified,        // 已验证
    Closed,          // 已关闭
    RolledBack,      // 已回滚
}

/// 审批人
pub struct Approver {
    pub user_id: String,
    pub name: String,
    pub role: String,
    pub decision: Option<Decision>,
    pub signed_at: Option<DateTime<Utc>>,
    pub signature: Option<String>,
}

/// 审批决策
pub enum Decision {
    Approved,
    Rejected,
    RequestChanges,
}

/// 影响评估
pub struct ImpactAssessment {
    pub affected_systems: Vec<String>,
    pub affected_users: Vec<String>,
    pub downtime_estimate: Duration,
    pub risk_mitigation: Vec<String>,
    pub testing_requirements: Vec<String>,
}
```

### 2.2 API设计

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/changes` | GET | 获取变更列表 |
| `/api/v1/changes` | POST | 创建变更请求 |
| `/api/v1/changes/:id` | GET | 获取变更详情 |
| `/api/v1/changes/:id` | PUT | 更新变更请求 |
| `/api/v1/changes/:id` | DELETE | 删除变更请求 |
| `/api/v1/changes/:id/submit` | POST | 提交审批 |
| `/api/v1/changes/:id/approve` | POST | 审批变更 |
| `/api/v1/changes/:id/reject` | POST | 拒绝变更 |
| `/api/v1/changes/:id/implement` | POST | 实施变更 |
| `/api/v1/changes/:id/verify` | POST | 验证变更 |
| `/api/v1/changes/:id/rollback` | POST | 回滚变更 |
| `/api/v1/changes/:id/close` | POST | 关闭变更 |
| `/api/v1/changes/:id/sign` | POST | 电子签名 |
| `/api/v1/changes/:id/audit` | GET | 获取审计日志 |

### 2.3 审批工作流

```
Draft → Submitted → UnderReview → Approved → Implementing → Implemented → Verifying → Verified → Closed
                                    ↓
                                 Rejected
```

### 2.4 电子签名

- 使用TOTP二因素认证
- 签名包含：用户ID、时间戳、变更ID、签名含义
- 签名哈希使用SHA-256
- 签名记录不可篡改

### 2.5 审计追踪

- 每个操作都记录审计日志
- 审计日志包含：操作者、操作时间、操作类型、操作详情
- 审计日志使用SHA-256哈希链
- 审计日志不可篡改

---

## 三、实现计划

### 3.1 第一阶段：核心功能（4小时）

1. 数据模型定义
2. 变更请求CRUD
3. 审批工作流
4. 基础审计日志

### 3.2 第二阶段：合规功能（4小时）

1. 电子签名
2. 风险评估
3. 回滚计划
4. 验证确认

### 3.3 第三阶段：自动化功能（4小时）

1. Linux自动化检查
2. 系统健康检查
3. 变更影响分析
4. 自动化测试

### 3.4 第四阶段：集成功能（4小时）

1. 集成到KIAS
2. Web界面
3. API文档
4. 测试用例

---

## 四、技术选型

| 组件 | 技术 | 理由 |
|------|------|------|
| 后端 | Rust + Axum | 高性能、内存安全 |
| 数据库 | SQLite | 轻量级、易部署 |
| 认证 | JWT + TOTP | 安全、标准 |
| 审计 | SHA-256哈希链 | 不可篡改 |
| 前端 | HTML + JS | 简单、易维护 |

---

## 五、合规映射

| 合规要求 | 系统功能 |
|---------|---------|
| FDA 21 CFR Part 11 §11.10 | 审计追踪、电子签名 |
| FDA 21 CFR Part 11 §11.50 | 电子签名含义记录 |
| FDA 21 CFR Part 11 §11.70 | 签名与记录绑定 |
| FDA 21 CFR Part 11 §11.100 | 电子签名唯一性 |
| EU Annex 11 Clause 9 | 变更控制 |
| EU Annex 11 Clause 10 | 定期审查 |
| GAMP 5 | 风险评估、验证确认 |

---

## 六、明天交付清单

- [ ] 数据模型定义
- [ ] 变更请求CRUD API
- [ ] 审批工作流
- [ ] 电子签名
- [ ] 审计追踪
- [ ] 风险评估
- [ ] 回滚计划
- [ ] 验证确认
- [ ] Linux自动化检查
- [ ] Web界面
- [ ] 测试用例
- [ ] API文档
