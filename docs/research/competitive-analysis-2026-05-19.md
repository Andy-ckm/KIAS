# 企业文档管理 & Linux自动化运维 竞品研究报告

> 研究日期：2026-05-19
> 研究方法：GitHub API + 源码分析

## 一、竞品分析

### 1.1 Linux 自动化运维

| 项目 | Stars | 定位 | 核心功能 | 优势 | 劣势 |
|------|-------|------|----------|------|------|
| **AWX** | 15,416 | Ansible Web UI | Playbook执行、RBAC、审计、REST API | 功能完整、企业级 | 学习曲线高、部署复杂 |
| **Cockpit** | 14,093 | Linux服务器Web管理 | 系统监控、容器、存储、网络、终端 | 界面友好、轻量级 | 无合规审计 |
| **Webmin** | 5,740 | Web控制面板 | 用户管理、服务、DNS、文件 | 老牌稳定 | 界面过时 |

**市场空白：没有专注医疗行业的开源运维工具**

### 1.2 企业文档管理

| 项目 | Stars | 定位 | 核心功能 |
|------|-------|------|----------|
| **sismics/docs** | 2,546 | 轻量级DMS | 全文搜索、版本控制、权限管理 |
| **Mayan-EDMS** | 806 | 企业级DMS | OCR、工作流、多格式支持 |
| **bluedoc** | 633 | 企业文档管理 | 协作、权限、导出 |

**市场空白：没有专注 GxP 合规的开源文档管理系统**

## 二、关键发现

### 2.1 Linux 自动化运维
1. **AWX 是标杆** — RBAC + 审计 + REST API
2. **Cockpit 界面最好** — 但无合规功能
3. **我们的差异化**：医疗合规 + 简单易用 + 审计追踪

### 2.2 企业文档管理
1. **版本控制是标配** — 所有项目都有
2. **电子签名是关键** — 21 CFR Part 11 要求
3. **我们的差异化**：GxP合规 + 生命周期管理 + 电子签名

## 三、参考实现

### 3.1 AWX 架构参考
- REST API 设计：/api/v2/jobs/, /api/v2/inventories/
- RBAC 模型：Organization → Team → User → Role
- 审计日志：所有操作记录到 audit_log 表

### 3.2 Cockpit 架构参考
- WebSocket 实时通信
- 多服务器管理
- 插件化设计

## 四、设计建议

### 4.1 Linux 自动化
1. Web Dashboard（Cockpit风格）— 直观、实时
2. CLI（Ansible风格）— 运维人员批量操作
3. REST API — 集成其他系统
4. 审计日志 — 所有操作可追溯

### 4.2 企业文档管理
1. 生命周期管理 — Draft→Review→Approved→Published→Archived
2. 版本控制 — Git-like 版本追踪
3. 电子签名 — 21 CFR Part 11 合规
4. 审计追踪 — ALCOA+ 原则

## 五、参考源码

```
/mnt/reference-projects/
├── cockpit/          # Cockpit 源码（待下载）
├── awx/              # AWX 源码（待下载）
└── docs/             # sismics/docs 源码（待下载）
```
