# 车联网 Agent 合规治理方案
## —— V2X AI Agent 可追溯、透明、可控

> 日期：2026-05-23
> 任务：车联网 Agent 合规治理方案
> 定位：让 V2X AI Agent 在受监管车联网环境中可部署、可审计、可追责

---

## 一、行业背景与合规压力

### 1.1 车联网 AI Agent 现状

车联网（Connected Vehicle / V2X）中的 AI Agent 正在快速渗透：

| 场景 | Agent 类型 | 安全等级 |
|------|-----------|---------|
| 自动驾驶决策 | 感知→规划→控制 | ASIL-D |
| V2X 消息转发 | 路侧单元/车载单元 | SIL-2 |
| 驾驶员监控 | DMS (Driver Monitoring) | ASIL-B |
| 预测性维护 | 车辆健康诊断 | QM |
| OTA 更新审批 | 软件版本验证 | ASIL-B |
| 车队协同 | 多车路径规划 | ASIL-C |

**问题**：这些 Agent 的行为缺乏统一治理——谁做了什么决策、基于什么数据、有无篡改，统统黑盒。

### 1.2 核心监管框架

#### 国际：UNECE WP.29 + ISO/SAE

| 法规 | 约束内容 | 对 Agent 的影响 |
|------|---------|----------------|
| UNECE R155 | 网络安全管理体系 (CSMS) | 所有联网 Agent 必须有安全审计轨迹 |
| UNECE R156 | 软件更新管理 (SUMS) | OTA Agent 必须记录更新前后的状态差异 |
| ISO/SAE 21434 | 汽车网络安全工程 | TARA 分析 + 安全设计 + 渗透测试 |
| ISO 26262 | 功能安全 (ASIL A-D) | 安全相关 Agent 必须满足 ASIL 等级 |
| ISO 21448 SOTIF | 预期功能安全 | L2+ 自动驾驶 Agent 的 ODD 边界分析 |

#### 区域：GDPR / 中国 MIIT / 美国 NHTSA

| 法规 | 约束内容 | 对 Agent 的影响 |
|------|---------|----------------|
| GDPR | 驾驶数据 = 个人数据 | Agent 采集的轨迹、行为数据必须合规 |
| 中国 MIIT SCMS | 车联网安全信任体系 | V2X 消息必须使用 PKI 证书签名 |
| 美国 NHTSA | 事故数据记录 (EDR) | Agent 决策必须可回放、可重构 |
| California DMV | 自动驾驶测试许可 | 每台车的 Agent 决策日志必须保存 5 年 |

### 1.3 合规痛点矩阵

```
痛点                    WP.29   ISO21434   GDPR    MIIT
─────────────────────────────────────────────────────
决策不可追溯            ★★★       ★★       ★      ★★
消息无签名验证          ★★★       ★★★      ★      ★★★
OTA 更新无审批          ★★★       ★★       ★      ★★
驾驶数据跨境传输        ★         ★        ★★★     ★
V2X 通信身份伪造        ★★★       ★★★      ★      ★★★
Agent 决策偏差无告警     ★★        ★★       ★       ★
```

---

## 二、AgentGuard 车联网合规架构

### 2.1 总体架构：四层治理

```
┌─────────────────────────────────────────────────────────┐
│              Layer 4: 问责与审计层                       │
│  AccountabilityGraph · 电子签名 · 区块链锚定               │
├─────────────────────────────────────────────────────────┤
│              Layer 3: 安全与合规层                        │
│  V2XPkiProvider · OtaApprovalGate · ComplianceGate     │
├─────────────────────────────────────────────────────────┤
│              Layer 2: 通信与协作层                       │
│  V2XRouter · VehicleSwarm · UnfiedNamespace            │
├─────────────────────────────────────────────────────────┤
│              Layer 1: 车辆执行层                         │
│  CanBusAgent · TelemetryAgent · SafetyMonitorAgent     │
└─────────────────────────────────────────────────────────┘
```

### 2.2 V2X Agent 类型扩展

在 AgentGuard 现有 Agent 类型基础上，新增车联网专用类型：

```rust
// 扩展 AuthProviderType
pub enum AgentType {
    // ... 现有类型 ...
    /// V2X 消息路由 Agent
    V2xRouter,
    /// 自动驾驶决策 Agent
    AutonomyDriving,
    /// OTA 更新审批 Agent
    OtaApproval,
    /// 车辆健康诊断 Agent
    VehicleHealth,
    /// 驾驶员监控 Agent
    DriverMonitoring,
    /// 车队协同 Agent
    FleetCoordination,
    /// 路侧单元 Agent
    RoadSideUnit,
}
```

### 2.3 核心新增模块

#### 模块 1: V2X 消息治理（V2X Message Governance）

V2X 通信（SCC/CAM/DENM/V2I）的完整生命周期审计：

```
消息生命周期：
  生成 → 签名 → 广播/路由 → 接收验证 → 决策触发 → 归档

每个节点记录：
  - 发送者身份（PKI 证书 hash）
  - 消息类型 + 内容 hash
  - 时间戳（可信时间源）
  - 接收方列表
  - 关联的 Agent 决策
```

**实现位置**：`crates/v2x-governance/`

#### 模块 2: OTA 更新审批门（OtaApprovalGate）

```rust
/// OTA 更新审批门 — 对标 UNECE R156 SUMS
pub struct OtaApprovalGate {
    /// 软件版本元数据仓库
    version_registry: VersionRegistry,
    /// 签名验证器（使用 MIIT SCMS PKI）
    signature_verifier: V2XPkiVerifier,
    /// ASIL 等级验证器
    asil_classifier: AsilClassifier,
    /// 变更影响评估器
    impact_analyzer: ChangeImpactAnalyzer,
    /// 审批工作流引擎
    approval_workflow: WorkflowEngine,
}

impl OtaApprovalGate {
    /// 评估 OTA 更新是否可执行
    pub async fn evaluate_update(&self, update: OtaUpdate) -> ApprovalResult {
        // 1. 签名验证（MIIT SCMS 证书链）
        // 2. ASIL 等级分类
        // 3. 变更影响评估（是否影响安全相关功能）
        // 4. 回滚计划验证
        // 5. 审批链记录（RBAR — Role-Based Access with Review）
    }
}
```

#### 模块 3: V2X PKI 身份认证（V2XPkiProvider）

车联网专用身份认证，对接各国 SCMS（Security Credential Management System）：

```rust
/// V2X PKI 认证提供者 — 对标 IEEE 1609.2 / MIIT SCMS
pub struct V2XPkiProvider {
    /// 可信 CA 证书列表
    trusted_ca_pems: Vec<String>,
    /// CRL 分布点
    crl_urls: Vec<String>,
    /// 证书有效期缓存
    cert_validity_cache: HashMap<String, CertValidity>,
}

#[async_trait]
impl AuthProvider for V2XPkiProvider {
    /// 验证 V2X 消息证书（IEEE 1609.2 格式）
    async fn authenticate(&self, cred: AuthCredential) -> Result<AuthResult, AuthProviderError> {
        match cred {
            AuthCredential::V2xCertificate(pem) => {
                // 1. 解析证书（OER 编码）
                // 2. 验证签名链（ECQV / ECDSA）
                // 3. 检查 CRL（是否有撤销）
                // 4. 验证时间窗口
                // 5. 映射到 Agent 身份
            }
            _ => Err(AuthProviderError::InvalidCredential),
        }
    }
}
```

#### 模块 4: 事故数据重构（Incident Reconstruction）

满足 NHTSA EDR 要求 + 中国 GB 39732：

```rust
/// 事故重构器 — 满足 EDR 合规
pub struct IncidentReconstructor {
    /// 不可变决策日志（哈希链）
    decision_chain: ImmutableLog,
    /// V2X 消息存档
    v2x_archive: V2xMessageStore,
    /// 传感器原始数据锚定
    sensor_anchors: HashChain,
}

impl IncidentReconstructor {
    /// 从哈希链重构事故前 N 秒的完整 Agent 决策序列
    pub async fn reconstruct(&self, incident_id: &str) -> ReconstructionResult {
        // 1. 找到事故触发点
        // 2. 向前回溯决策链（哈希链验证完整性）
        // 3. 关联 V2X 消息（谁广播了什么）
        // 4. 关联传感器锚定数据
        // 5. 生成法证报告（满足 NHTSA EDR 格式）
    }
}
```

---

## 三、现有资产复用

### 3.1 可直接复用的模块

| 现有模块 | 复用方式 |
|---------|---------|
| `UnifiedNamespace` | 作为 V2X topic 的命名空间（已有 ISA-95 结构） |
| `adversarial_validation` | 用作自动驾驶决策的多 Agent 对抗验证 |
| `SwarmOrchestrator` | 车队协同多车并行决策 |
| `accountability.rs` | 扩展为车辆决策因果图 |
| `auth_providers` | 扩展 V2XPkiProvider（刚刚实现的 Kerberos/LDAP 可作参考） |
| `quality_pipeline` | OTA 更新的质量门禁 |
| `audit::MemoryAuditLog` | 扩展为车载不可变审计日志 |

### 3.2 需要新建的模块

| 新模块 | 对应规范 | 优先级 |
|-------|---------|--------|
| `v2x-governance` crate | WP.29 R155/R156 | P0 |
| `v2x-message-store` | IEEE 1609.2 | P0 |
| `ota-approval-gate` | UNECE R156 | P0 |
| `incident-reconstructor` | NHTSA EDR / GB 39732 | P1 |
| `vehicle-telemetry-privacy` | GDPR | P1 |
| `asil-classifier` | ISO 26262 | P2 |

---

## 四、合规覆盖矩阵

| 合规要求 | 实现方式 | 覆盖度 |
|---------|---------|--------|
| WP.29 CSMS 决策审计 | AccountabilityGraph 扩展 | 90% |
| R156 SUMS OTA 审批 | OtaApprovalGate | 85% |
| ISO/SAE 21434 TARA | ThreatAnalysis 模块 | 60% |
| ISO 26262 ASIL 分级 | AsilClassifier | 70% |
| GDPR 驾驶数据合规 | VehicleTelemetryPrivacy | 80% |
| MIIT SCMS 证书验证 | V2XPkiProvider | 75% |
| NHTSA EDR 数据保存 | IncidentReconstructor | 90% |
| V2X 消息完整性 | HashChain + 签名 | 95% |

---

## 五、商业价值

### 目标客户

| 客户类型 | 年收入 | 痛点 | 预算 |
|---------|--------|------|------|
| 整车厂（OEM） | >$50B | WP.29 认证 | $200K-$500K/年 |
| Tier 1 供应商 | $10B-$50B | ISO 21434 审计 | $100K-$300K/年 |
| 车联网 MSP | $1B-$10B | GDPR + MIIT | $50K-$150K/年 |
| Robotaxi 运营商 | $100M-$1B | NHTSA 事故重构 | $80K-$200K/年 |

### 定价策略

```
车联网合规套件：
1. V2X 消息治理（基础）
2. OTA 审批门禁
3. PKI 证书验证
4. 事故重构（法证报告）
5. 多域 Agent 协调（Swarm）

基础版：$80K/年（V2X 消息 + OTA 审批）
专业版：$150K/年（+ PKI + 事故重构）
企业版：$300K/年（+ 多域 Swarm + 定制合规）
```

---

## 六、实施路径

### Phase 1（1-2月）：核心基础设施
- 新建 `v2x-governance` crate
- 实现 V2XPkiProvider（对接 MIIT SCMS）
- 实现 OtaApprovalGate
- 复用 UnifiedNamespace 建立 V2X topic 层级

### Phase 2（2-3月）：合规增强
- 扩展 AccountabilityGraph 支持 V2X 消息因果链
- 实现 IncidentReconstructor
- 实现 VehicleTelemetryPrivacy（GDPR 合规）

### Phase 3（3-4月）：生产验证
- 在模拟车队环境中验证
- 对接真实 SCMS PKI
- 认证：完成 WP.29 CSMS 自评估报告

---

## 七、总结

车联网是 AgentGuard 下一个高价值垂直场景。核心壁垒：

1. **V2X 消息 PKI** — 对接各国 SCMS（MIIT / IEEE 1609.2），这是竞争对手的空白
2. **OTA 审批门禁** — UNECE R156 强制要求，是 OEM 认证的必备项
3. **事故重构法证** — 满足 NHTSA/中国 EDR 标准，是 Robotaxi 运营商的刚需
4. **ASIL 感知调度** — ISO 26262 功能安全分级调度，是自动驾驶合规的基础

车联网 Agent 治理 = AgentGuard 在汽车行业的"Harness"，让 OEM 和 Tier 1 敢在生产环境部署 AI Agent。
