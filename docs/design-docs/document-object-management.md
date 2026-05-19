# 文件管理与对象定义 — 产品/工艺/质量管理系统

> 来源：用户灵感注入，2026-05-18
> 关联：GBrain 模式吸收方案、EntityPage 设计

---

## 核心定义

### 文件管理
对产品、工艺和质量管理系统的文件进行**分类、识别、创建、修订、审阅、批准、执行和控制**的过程。

### 对象
系统中管理的**唯一标识项**的通用名称（例如，文档、零件、CAD）。这些项目可以添加到变更请求和变更通知中。

---

## 映射到 AgentGuard

| 文件管理步骤 | AgentGuard 模块 | 状态 |
|-------------|----------|------|
| 分类、识别 | EntityExtractor（零LLM正则提取） | ✅ 已完成 |
| 创建 | EntityPage 结构（Compiled Truth + Timeline） | 📋 Phase 2 |
| 修订 | Compiled Truth 可覆盖重写 | 📋 Phase 2 |
| 审阅 | quality_pipeline.rs 质量管线 | ✅ 已有 |
| 批准 | **待设计** — 审批流转机制 | ❌ 缺失 |
| 执行 | DreamConsolidator + Minions | 📋 Phase 2 |
| 控制 | 版本控制 + 变更追踪 | ❌ 缺失 |

| 对象概念 | AgentGuard 映射 | 说明 |
|---------|----------|------|
| 唯一标识项 | EntityPage.id | 每个实体有唯一 ID |
| 文档/零件/CAD | EntityType 枚举 | Person/Company/Concept/Project/Meeting/Document |
| 变更请求 | **待设计** — ChangeRequest 结构 | 记录"谁在什么时候改了什么" |
| 变更通知 | TimelineEntry（追加式时间线） | 只增不改的审计日志 |

---

## 缺失能力（需设计）

### 1. 审批流转
当前 AgentGuard 知识层没有"审阅→批准→执行"的流转机制。知识写入后直接生效。

**建议**：为 EntityPage 增加 `approval_state` 字段：
```rust
pub enum ApprovalState {
    Draft,      // 草稿 — Agent 写入，待审阅
    Reviewing,  // 审阅中 — 人工或自动审阅
    Approved,   // 已批准 — 生效
    Rejected,   // 已拒绝 — 不生效
    Archived,   // 已归档 — 历史版本
}
```

### 2. 变更控制
当前 Compiled Truth 直接覆盖，没有变更请求/通知机制。

**建议**：增加 ChangeRequest 结构：
```rust
pub struct ChangeRequest {
    pub id: String,
    pub entity_id: String,           // 目标实体
    pub change_type: ChangeType,     // Create/Update/Delete
    pub proposed_content: String,    // 提议内容
    pub reason: String,              // 变更原因
    pub requested_by: String,        // 请求者
    pub approval_state: ApprovalState,
    pub created_at: DateTime<Utc>,
}
```

### 3. 版本控制
当前没有实体版本历史。

**建议**：Compiled Truth 每次修改时保存快照：
```rust
pub struct EntityVersion {
    pub version: u32,
    pub content: String,
    pub changed_by: String,
    pub change_request_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

---

## 设计原则

1. **唯一标识** — 每个对象（文档、零件、概念）有全局唯一 ID
2. **变更可追溯** — 每次修改都有变更请求 + 时间戳 + 操作者
3. **审批可控** — 关键知识需要审批才能生效
4. **历史可查** — 所有版本都保留，可回溯
5. **分类可检索** — EntityExtractor 自动分类，零 LLM 成本
