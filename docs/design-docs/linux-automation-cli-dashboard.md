# Linux 自动化 CLI + Web Dashboard 设计方案

> 日期：2026-05-19
> 四步法：评估→审视→方案→开发

## 一、评估

### 1.1 需求背景
医药企业需要合规的 Linux 自动化运维工具。现有竞品（Cockpit、AWX）都不专注医疗行业。

### 1.2 核心需求
1. **CLI 工具** — 运维人员批量操作
2. **Web Dashboard** — 直观监控和操作
3. **合规审计** — FDA 21 CFR Part 11 要求
4. **RBAC 权限** — 角色分离

## 二、审视

### 2.1 现有能力
| 模块 | 行数 | 函数数 | 能力 |
|------|------|--------|------|
| linux_auto.rs | 995 | 17 | 命令生成、任务记录、统计 |
| kias-cli | 1356 | - | Agent/Workflow/Tool/Skill 管理 |

### 2.2 缺失能力
- 真正的任务执行引擎（SSH 远程执行）
- 实时状态推送（WebSocket）
- 审计日志持久化
- RBAC 权限控制

## 三、方案

### 3.1 架构设计

```
┌─────────────────────────────────────────────────┐
│           AgentGuard Linux 自动化                 │
├─────────────────────────────────────────────────┤
│  交互层                                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
│  │ CLI      │  │ Web UI   │  │ IM 通知  │       │
│  │ (clap)   │  │ (React)  │  │ (微信)   │       │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘       │
│       │             │             │             │
│  ┌────┴─────────────┴─────────────┴────┐        │
│  │         REST API (axum)             │        │
│  └────┬─────────────┬─────────────┬────┘        │
│       │             │             │             │
│  ┌────┴────┐  ┌─────┴────┐  ┌─────┴────┐       │
│  │ 任务    │  │ 合规     │  │ 审计     │       │
│  │ 执行    │  │ 扫描     │  │ 日志     │       │
│  │ 引擎    │  │ 引擎     │  │ 引擎     │       │
│  └─────────┘  └──────────┘  └──────────┘       │
└─────────────────────────────────────────────────┘
```

### 3.2 CLI 命令设计

```bash
# 任务管理
agentguard linux scan --host 192.168.1.1 --profile cis
agentguard linux patch --host all --packages openssl
agentguard linux deploy --playbook hardening.yml
agentguard linux status --task-id xxx

# 合规报告
agentguard linux report --format pdf --period 30d
agentguard linux audit --host 192.168.1.1

# 批量操作
agentguard linux exec --hosts hosts.txt --command "df -h"
```

### 3.3 Web Dashboard 页面

1. **总览页** — 服务器状态、合规评分、告警
2. **任务中心** — 创建/查看/取消任务
3. **合规报告** — CIS/STIG 扫描结果
4. **审计日志** — 操作历史、变更追踪
5. **资产管理** — 服务器列表、分组

### 3.4 数据模型

```rust
// 任务
struct Task {
    id: Uuid,
    task_type: TaskType,
    status: TaskStatus,
    hosts: Vec<String>,
    created_at: DateTime<Utc>,
    created_by: String,  // RBAC
    audit_trail: Vec<AuditEntry>,
}

// 审计日志
struct AuditEntry {
    timestamp: DateTime<Utc>,
    user: String,
    action: String,
    target: String,
    result: String,
    signature: Option<String>,  // 电子签名
}
```

## 四、开发计划

### Phase 1: 执行引擎（2天）
- [ ] SSH 远程执行（用 tokio::process）
- [ ] 任务队列（SQLite 持久化）
- [ ] 结果收集和聚合

### Phase 2: CLI 命令（1天）
- [ ] 添加 `linux` 子命令到 kias-cli
- [ ] 实现 scan/patch/deploy/status 命令
- [ ] 输出格式化（JSON/Table）

### Phase 3: REST API（1天）
- [ ] 任务 CRUD API
- [ ] 合规报告 API
- [ ] 审计日志 API

### Phase 4: Web Dashboard（2天）
- [ ] React 前端框架
- [ ] 总览页、任务中心
- [ ] WebSocket 实时推送

### Phase 5: 合规增强（1天）
- [ ] RBAC 权限控制
- [ ] 电子签名
- [ ] 审计日志不可变存储

## 五、参考项目

| 项目 | 借鉴点 |
|------|--------|
| Cockpit | Web UI 设计、多机管理 |
| AWX | RBAC、审计日志 |
| Ansible | CLI 命令设计 |

## 六、验收标准

1. CLI 能远程执行命令并返回结果
2. Web Dashboard 能实时显示任务状态
3. 所有操作有审计日志
4. 139 个现有测试继续通过
5. 新增 50+ 测试
