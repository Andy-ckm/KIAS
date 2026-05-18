# KIAS 可视化方案（四步法审视）

> 创建时间：2026-05-18
> 状态：方案阶段（Step 3）
> 原则：论文+源码支撑，先深后广

---

## 一、丰田五问法分析（Step 1 评估）

```
需求：KIAS 需要可视化
  ↓
Q1: 为什么需要可视化？
A1: 因为知识关系、合规状态、审计事件看不见，难以调试和验证
  ↓
Q2: 为什么看不见就难以调试？
A2: 因为 GraphRAG 查询结果是文本，不直观；合规状态散落在各处
  ↓
Q3: 为什么不直观就影响开发？
A3: 因为无法快速验证图谱结构是否正确、合规是否达标
  ↓
Q4: 为什么需要快速验证？
A4: 因为迭代速度决定竞争力
  ↓
Q5: 为什么迭代速度这么重要？
A5: 因为 KIAS 的核心价值是"用得越久越智能"，需要快速验证进化效果
  ↓
结论：可视化是刚需，不是装饰
```

---

## 二、钱学森系统工程论审视（Step 2 审视）

### 2.1 四层架构定位

```
L0: common                    ← 基础类型、错误、配置
L1: data-store                ← SQLite 持久化层
L2: knowledge, team-engine    ← 知识图谱、合规引擎
L3: api-server                ← HTTP API + 可视化服务
```

**可视化模块定位**：L3 层（api-server），作为 HTTP 服务的一部分

### 2.2 现有功能检查

| 功能 | 现状 | 差距 |
|------|------|------|
| 知识图谱可视化 | ❌ 无 | 需新建 |
| 合规仪表盘 | ❌ 无 | 需新建 |
| 审计时间线 | ❌ 无 | 需新建 |
| API 文档 | ✅ rustdoc | 可扩展 |
| 依赖图 | ❌ 无 | 可用 cargo tree |

### 2.3 依赖方向检查

```
可视化模块 (L3)
  ├── 依赖 knowledge (L2) ← 正确
  ├── 依赖 data-store (L1) ← 正确
  └── 依赖 common (L0) ← 正确
```

✅ 无循环依赖，无跨层

---

## 三、论文+源码支撑（Step 3 方案）

### 3.1 知识图谱可视化

**论文支撑**：
| 论文 | 年份 | 要点 |
|------|------|------|
| Force-Directed Graph Drawing (Fruchterman & Reingold) | 1991 | 力导向布局奠基论文 |
| D3: Data-Driven Documents (Bostock et al.) | 2011 | D3.js 设计哲学 |
| A Survey of Knowledge Graph Visualization | 2021 | KG 可视化方法分类 |
| WebVOWL: Web-based Visualization of Ontologies (Lohmann et al.) | 2015 | OWL 本体可视化标准 |

**源码参考**：
| 项目 | Stars | 代码行数 | 适用场景 |
|------|-------|---------|---------|
| **cytoscape.js** | 10k+ | 生产级 | 多布局算法，适合复杂图谱 |
| **Antv G6** | 10k+ | 国产首选 | 中文文档好，蚂蚁金服出品 |
| **d3-force-graph** | 4k+ | 快速原型 | 50行代码出效果 |
| **sigma.js** | 10k+ | 大规模图 | WebGL 渲染，10万+节点 |
| **vis-network** | 2k+ | 简单易用 | vis.js 生态 |

**推荐方案**：`cytoscape.js`（生产级，布局算法丰富）

### 3.2 合规仪表盘

**论文支撑**：
| 论文 | 年份 | 要点 |
|------|------|------|
| Information Dashboard Design (Few) | 2006 | 仪表盘设计原则 |
| The Visual Display of Quantitative Information (Tufte) | 1983 | 数据可视化经典 |
| FDA 21 CFR Part 11 Guidance | 2003 | 合规仪表盘要求 |

**源码参考**：
| 项目 | Stars | 适用场景 |
|------|-------|---------|
| **Grafana** | 60k+ | 通用监控仪表盘 |
| **Apache Superset** | 60k+ | BI/数据可视化 |
| **ECharts** | 60k+ | 图表库，国产 |
| **Recharts** | 20k+ | React 图表组件 |

**推荐方案**：`ECharts`（国产，文档好，图表丰富）

### 3.3 审计时间线

**论文支撑**：
| 论文 | 年份 | 要点 |
|------|------|------|
| Temporal Visualization of Event Sequences | 2014 | 时间线设计原则 |
| LifeLines: Visualizing Personal Histories | 1996 | 时间线交互设计 |

**源码参考**：
| 项目 | Stars | 适用场景 |
|------|-------|---------|
| **vis-timeline** | 2k+ | 交互式时间线 |
| **D3.js Timeline** | - | 自定义时间线 |
| **Apache ECharts** | 60k+ | 时间线、甘特图 |

**推荐方案**：`vis-timeline`（开源，交互性强，支持缩放/拖拽）

### 3.4 审计追踪可视化（GxP 专用）

**源码参考**：
| 项目 | 适用场景 | GxP 合规 |
|------|---------|---------|
| **OpenClinica** | 临床试验数据管理 | ✅ 21 CFR Part 11 原生 |
| **OpenSpecimen** | 生物样本管理 | ✅ FDA 合规设计 |
| **ELK Stack** | 审计日志可视化 | ✅ 广泛用于 FDA 合规 |

**推荐方案**：参考 `OpenClinica` 的审计追踪设计模式

---

## 四、技术选型决策

### 4.1 整体架构

```
┌─────────────────────────────────────────────────┐
│                  KIAS 可视化架构                   │
├─────────────────────────────────────────────────┤
│  前端层: 静态 HTML + JS（无需 React/Vue）          │
│  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ 知识图谱      │  │ 合规仪表盘               │ │
│  │ cytoscape.js │  │ ECharts                  │ │
│  └──────────────┘  └──────────────────────────┘ │
│  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ 审计时间线    │  │ 依赖图                   │ │
│  │ vis-timeline │  │ Graphviz DOT             │ │
│  └──────────────┘  └──────────────────────────┘ │
├─────────────────────────────────────────────────┤
│  后端层: Rust axum HTTP 服务                      │
│  /api/v1/viz/* 端点返回 JSON 数据                 │
│  /viz/* 端点返回静态 HTML 页面                    │
├─────────────────────────────────────────────────┤
│  数据层:                                         │
│  - KnowledgeGraph (L2) → 知识图谱数据            │
│  - SqliteAuditLog (L1) → 审计事件数据            │
│  - cargo metadata → 依赖关系数据                 │
└─────────────────────────────────────────────────┘
```

### 4.2 API 端点设计

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/v1/viz/knowledge-graph` | GET | 知识图谱数据（JSON） |
| `/api/v1/viz/compliance-status` | GET | 合规状态数据（JSON） |
| `/api/v1/viz/audit-timeline` | GET | 审计事件时间线（JSON） |
| `/api/v1/viz/dependencies` | GET | 依赖关系数据（JSON） |
| `/viz/knowledge-graph` | GET | 知识图谱页面（HTML） |
| `/viz/compliance-dashboard` | GET | 合规仪表盘页面（HTML） |
| `/viz/audit-timeline` | GET | 审计时间线页面（HTML） |
| `/viz/dependencies` | GET | 依赖图页面（HTML） |

### 4.3 实现优先级（先深后广）

| 优先级 | 功能 | 理由 |
|--------|------|------|
| P0 | 知识图谱可视化 | KIAS 核心价值验证 |
| P1 | 审计时间线 | GxP 合规刚需 |
| P2 | 合规仪表盘 | 受监管行业需求 |
| P3 | 依赖图 | 开发辅助 |

---

## 五、实施计划

### Phase 1: 知识图谱可视化（1-2天）
1. 创建 `/static/knowledge-graph.html`
2. 使用 cytoscape.js 渲染力导向图
3. 创建 `/api/v1/viz/knowledge-graph` 端点
4. 从 KnowledgeGraph 提取节点和边数据

### Phase 2: 审计时间线（1-2天）
1. 创建 `/static/audit-timeline.html`
2. 使用 vis-timeline 渲染时间线
3. 创建 `/api/v1/viz/audit-timeline` 端点
4. 从 SqliteAuditLog 提取事件数据

### Phase 3: 合规仪表盘（2-3天）
1. 创建 `/static/compliance-dashboard.html`
2. 使用 ECharts 渲染图表
3. 创建 `/api/v1/viz/compliance-status` 端点
4. 汇总合规状态数据

### Phase 4: 依赖图（1天）
1. 创建 `/static/dependencies.html`
2. 使用 Graphviz DOT 渲染依赖图
3. 创建 `/api/v1/viz/dependencies` 端点
4. 从 cargo metadata 提取依赖数据

---

## 六、质量门禁

- [ ] 所有可视化页面支持响应式设计
- [ ] 所有 API 端点有单元测试
- [ ] 所有 HTML 页面有 CSP 安全头
- [ ] 所有图表支持交互（缩放、拖拽、点击）
- [ ] 所有数据端点支持分页和过滤
