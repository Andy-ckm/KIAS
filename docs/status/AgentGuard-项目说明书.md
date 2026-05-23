# AgentGuard 项目说明书

> 项目名称：AgentGuard（原KIAS）
> 版本：v1.0
> 更新日期：2026-05-23
> 仓库：github.com/Andy-ckm/KIAS

---

## 一、项目概述

### 1.1 一句话定位

**AgentGuard = AI Agent 的合规治理框架**

让企业在生产环境敢用、用好、能监管 AI Agent。

### 1.2 解决的问题

| 企业痛点 | AgentGuard 方案 |
|---------|----------------|
| Agent 行为不可控 | Harness 多层约束体系 |
| 合规审计难追溯 | GxP/ALCOA+ 审计链，决策可回溯 |
| 多 Agent 协作无治理 | Owner-Worker-Verifier 三方对抗质量门禁 |
| LLM 调用成本不透明 | Token 级成本归因与异常检测 |
| 变更控制不合规 | CCR 审批流 + 电子签名（21 CFR Part 11） |
| Agent 输出质量不稳定 | self_repair + quality_gate 自循环保障 |

### 1.3 目标客户

**一级：国际制药/医疗器械巨头**
- J&J（强生）、Pfizer（辉瑞）、Roche（罗氏）
- 需求：GxP 合规、21 CFR Part 11、审计不可篡改

**二级：金融/保险合规机构**
- 需求：决策可解释、操作可追溯、模型治理

**三级：大型企业 AI 平台团队**
- 需求：多 Agent 协作治理、成本控制、质量保障

---

## 二、技术架构

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                    L4: 自我进化 Harness                       │
│         auto-loop + quality_gate + self_repair              │
├─────────────────────────────────────────────────────────────┤
│                    L3: 安全 Harness                          │
│   compliance-security + gxp_audit + approval + autonomy      │
├─────────────────────────────────────────────────────────────┤
│                    L2: 知识 Harness                          │
│      knowledge + graphrag + entity_extractor + skills        │
├─────────────────────────────────────────────────────────────┤
│                    L1: 执行 Harness                          │
│     team-engine + tool-executor + workflow-engine + skills    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 核心子系统

| 子系统 | 技术栈 | 核心能力 |
|--------|-------|---------|
| **API Server** | Rust (axum) | RESTful + gRPC，多协议网关 |
| **Scheduler** | Rust | RR/LL/RA/CA 四算法 + 亲和性调度 |
| **Controller** | Rust | Agent 生命周期、心跳监控、故障恢复 |
| **Workflow Engine** | Rust | DAG 执行、条件分支、重试、检查点 |
| **Team Engine** | Rust | Owner-Worker-Verifier 对抗式质量门禁 |
| **Goal Engine** | Rust | 目标驱动循环、自动迭代 |
| **Auto Loop** | Rust | self_repair、side_effect_gate、quality_gate |
| **LangGraph Engine** | Rust | 状态图、并行扇出、中断恢复 |
| **MCP Protocol** | Rust | JSON-RPC 2.0、浏览器自动化 |
| **LLM Engine** | Rust | MiniMax/OpenAI/Anthropic 多 Provider |
| **Model Router** | Rust | Key 轮换、可用性检查、成本优化 |
| **Compliance Security** | Rust | GxP 审计、ALCOA+、Prompt 防御 |
| **Monitor** | Rust | 遥测、异常检测（Z-score + IQR）、slow_trace |
| **IM Integration** | Rust | 微信、飞书、钉钉集成 |
| **IT Change Management** | Rust | CCR 审批流、电子签名 |

### 2.3 技术指标

| 指标 | 数值 |
|------|------|
| **语言** | Rust |
| **Crate 数量** | 36 |
| **Rust LOC** | ~143,300 |
| **测试函数** | 5241+ |
| **clippy 警告** | 0 |
| **production unwrap()** | 0 |
| **测试密度** | 所有非 benchmark crate ≥ 2.0 |

---

## 三、核心特性

### 3.1 GxP 合规审计

- **ALCOA+ 原则**：可追溯、清晰、同步、原始、准确
- **21 CFR Part 11**：电子签名、电子记录不可篡改
- **GxP 审计链**：每次操作生成不可变审计事件
- **审计字段**：who/when/what/why/后果/批准人

### 3.2 多层 Harness 体系

```
约束层：定义 Agent 能做什么、不能做什么
参照层：提供已有实现作为模仿对象
验证层：分阶段检查，确保符合预期
进化层：auto-loop 持续优化、自我修复
```

### 3.3 Token 成本归因

- **Agent 级**：每个 Agent 的 LLM 调用成本
- **Task 级**：每个任务的 Token 消耗
- **Model 级**：不同模型的费用对比
- **异常检测**：IQR + Z-score 双模式异常告警

### 3.4 IT 变更管理（CCR）

- **9 状态审批流**：Draft → Reviewing → Approved → Published
- **多级审批**：QA 主管 → IT 主管 → 质量总监
- **电子签名**：TOTP 二因素认证
- **副作用预演**：Dry-run 验证变更影响

### 3.5 团队协作模式

- **Owner**：任务创建者，负责验收
- **Worker**：执行者，负责交付
- **Verifier**：质量门禁，对抗式审查
- **Crew**：智能编排，自动匹配技能与任务

---

## 四、商业化路径

### 4.1 三层商业模式

| 层级 | 产品 | 目标客户 | 价值主张 |
|------|------|---------|---------|
| **开源核心** | AgentGuard Open Source | 开发者社区 | 免费使用，吸引用户 |
| **商业版** | AgentGuard Enterprise | 企业 AI 团队 | 合规 + 企业功能，付费 |
| **服务** | AgentGuard Services | 受监管行业 | 定制开发 + 培训 + 运维 |

### 4.2 差异化优势

| 维度 | 竞品 | AgentGuard |
|------|------|-----------|
| **合规深度** | 无 GxP 支持 | ALCOA+ / 21 CFR Part 11 原生 |
| **审计不可篡改** | 日志可改 | 哈希链 + 签名 |
| **决策可追溯** | 黑盒 | 完整决策链记录 |
| **多模型治理** | 单模型 | MiniMax/OpenAI/Anthropic 统一 |
| **开源** | 闭源为主 | Apache 2.0 开源 |

---

## 五、项目结构

```
AgentGuard/
├── crates/                    # 36 个 Rust crate
│   ├── api-server/            # API 网关
│   ├── scheduler/             # 调度器
│   ├── controller/            # 控制器
│   ├── workflow-engine/       # 工作流引擎
│   ├── team-engine/           # 团队引擎
│   ├── goal-engine/           # 目标引擎
│   ├── auto-loop/             # 自循环
│   ├── autonomy-controller/   # 自主控制
│   ├── langgraph-engine/      # LangGraph 引擎
│   ├── mcp-protocol/          # MCP 协议
│   ├── llm-engine/            # LLM 引擎
│   ├── model-router/          # 模型路由
│   ├── compliance-security/   # 合规安全
│   ├── monitor/               # 监控
│   ├── im-integration/        # IM 集成
│   ├── it-change-management/  # IT 变更管理
│   ├── document-management/   # 文档管理
│   ├── linux-automation/     # Linux 自动化
│   ├── cache/                 # 缓存
│   ├── data-store/            # 数据存储
│   ├── data-governance/       # 数据治理
│   ├── skills/                # 技能系统
│   ├── knowledge/            # 知识图谱
│   ├── data-aggregator/      # 数据聚合
│   ├── tool-executor/         # 工具执行
│   ├── agent-runtime/        # Agent 运行时
│   ├── a2a-registry/         # A2A 注册
│   ├── agent-view/            # Agent 视图
│   ├── agent-runtime/        # Agent 运行时
│   ├── agentsight/            # Agent 视野
│   ├── kias-cli/             # CLI 工具
│   ├── gdpr-compliance/       # GDPR 合规
│   ├── harness-registry/     # Harness 注册
│   ├── benchmarks/           # 性能基准
│   ├── common/               # 通用库
│   ├── gxp-compliance/        # GxP 合规
│   └── kias-main/            # 主服务
├── dashboard/                 # React 前端
├── docs/                     # 文档
│   ├── business/             # 商业文档
│   ├── design-docs/          # 设计文档
│   ├── papers/               # 论文索引
│   └── status/               # 状态报告
└── scripts/                  # 构建/启动脚本
```

---

## 六、核心设计原则

### 6.1 铁律

1. **四步开发法**：评估 → 审视 → 方案 → 开发，违反 = 返工
2. **研究先行**：设计功能前必须 GitHub 搜 Top5 竞品 + 下载源码
3. **灵魂 > 骨架**：先定义问题再写代码，不接受假实现
4. **无死代码**：所有模块必须有明确接入点，写了没接入 = 没写
5. **生产零 unwrap()**：2026-05-21 已清零

### 6.2 方法论

- **钱学森系统工程**：从整体到局部，从上而下
- **马斯克第一性原理**：从物理本质出发，不接受类比推理
- **论文/源码支撑**：所有设计必须有竞品源码或论文依据

---

## 七、质量保障

### 7.1 CI/CD 流水线

```bash
make build    # 构建所有组件
make lint     # clippy 检查
make format   # rustfmt 格式化
make test     # 单元测试 + 集成测试
make lint-arch # 分层依赖检查
```

### 7.2 质量门禁

- **密度 ≥ 2.0**：所有非 benchmark crate 测试密度必须 ≥ 2.0
- **clippy 0 警告**：所有 crate 必须通过 clippy
- **production unwrap 0**：生产代码禁止 unwrap/expect/panic
- **分层依赖**：L0 ← L1 ← L2 ← L3，禁止跨层依赖

---

## 八、当前状态

| 维度 | 状态 |
|------|------|
| **代码完成度** | 核心功能完成，部分集成待完善 |
| **测试覆盖** | 5241+ 测试，高密度覆盖 |
| **合规功能** | GxP 审计链、电子签名、CCR 审批流 |
| **LLM 集成** | MiniMax 骨架已完成，API key 待接入 |
| **文档完整度** | 架构/商业/方法论文档齐全 |
| **商业化** | 商业化路径已规划，目标客户已明确 |

---

## 九、联系方式

- **仓库**：github.com/Andy-ckm/KIAS
- **问题反馈**：GitHub Issues
- **核心维护**：零（AgentGuard 团队）
