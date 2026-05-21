# AgentGuard 竞品全景分析与超越方案

> 日期：2026-05-21
> 数据来源：GitHub API (254 repos) + 292 篇论文 + EMQ 44 客户 + 行业调研

---

## 第一部分：竞品全景

### 一、Agent 框架/平台层（我们不竞争，但要集成）

| 项目 | Stars | 定位 | 语言 | 许可证 |
|------|-------|------|------|--------|
| Dify | 142K | Agentic 工作流平台 | Python | Apache-2.0 |
| LangChain | 137K | Agent 工程平台 | Python | MIT |
| MetaGPT | 68K | 多 Agent 软件公司 | Python | MIT |
| AutoGen | 58K | 微软多 Agent 框架 | Python | MIT |
| CrewAI | 52K | 角色扮演 Agent 编排 | Python | MIT |
| Huginn | 49K | Agent 监控自动化 | Ruby | MIT |
| LiteLLM | 48K | LLM 网关/代理 | Python | MIT |
| LlamaIndex | 50K | 文档 Agent + OCR | Python | MIT |
| LangGraph | 33K | 弹性 Agent 图 | Python | MIT |
| OpenAI Agents | 27K | 轻量多 Agent | Python | MIT |
| Mastra | 24K | Gatsby 团队 Agent 框架 | TS | MIT |
| Letta | 23K | 有状态 Agent | Python | Apache-2.0 |

**关键洞察：** 这些是 Agent 的"发动机"，AgentGuard 是 Agent 的"安全带+行车记录仪"。不竞争，互补。

---

### 二、Agent 安全/护栏层（直接竞争）

| 项目 | Stars | 定位 | 核心能力 | 弱点 |
|------|-------|------|----------|------|
| Guardrails AI | 5K+ | LLM 输出护栏 | 校验、重试、结构化输出 | 只管输出，不管行为 |
| NeMo Guardrails | 4K+ | NVIDIA 对话护栏 | Colang 规则、话题限制 | 只适合对话场景 |
| LLM Guard | 3K+ | LLM 安全扫描 | 提示注入检测、内容过滤 | 静态规则，无运行时治理 |
| Rebuff | 2K+ | 提示注入检测 | 多层检测、Canary Token | 只防注入，不管审计 |
| Lakera Guard | 商业 | 提示注入防护 | 实时检测、API 服务 | 闭源、按调用收费 |
| Prompt Armor | 商业 | 提示安全 | 企业级防护 | 闭源 |
| Robust Intelligence | 商业 | AI 安全平台 | 模型验证、运行时防护 | 通用 AI，不专注 Agent |
| Arthur AI | 商业 | AI 可观测性 | 模型监控、护栏 | 不专注 Agent 行为 |
| Galileo | 商业 | LLM 可观测性 | 幻觉检测、质量评估 | 不管合规 |
| WhyLabs | 商业 | AI 可观测性 | 数据漂移、模型监控 | 不专注 Agent |

**关键洞察：** 现有安全工具只做"输入/输出过滤"，不管 Agent 的**行为治理**。AgentGuard 管的是"Agent 做了什么、为什么做、谁授权的"。

---

### 三、Agent 可观测性层（部分竞争）

| 项目 | Stars | 定位 | 核心能力 | 弱点 |
|------|-------|------|----------|------|
| LangSmith | 商业 | LangChain 追踪 | 调用链、评估、数据集 | 绑定 LangChain |
| LangFuse | 8K+ | 开源 LLM 追踪 | 追踪、评估、Prompt 管理 | 只管 LLM 调用 |
| Helicone | 3K+ | LLM 代理 | 请求日志、缓存、限流 | 只代理 API |
| Arize Phoenix | 5K+ | LLM 可观测性 | 追踪、评估、嵌入分析 | 不管 Agent 行为 |
| AgentOps | 2K+ | Agent 会话回放 | 会话记录、成本追踪 | 浅层监控 |
| Braintrust | 商业 | AI 评估平台 | 评估、日志、数据集 | 不管合规 |
| Datadog LLM Obs | 商业 | LLM 监控 | 集成 Datadog 生态 | 通用，不专注 Agent |
| W&B Weave | 商业 | Agent 追踪 | 追踪、评估、实验管理 | 不管审计合规 |
| OpenLIT | 2K+ | OpenTelemetry AI | 原生 OTel 集成 | 只做遥测 |

**关键洞察：** 可观测性工具只做"看"，不做"控"。AgentGuard = 看 + 控 + 审 + 合规。

---

### 四、企业 AI 平台层（潜在集成伙伴）

| 平台 | 厂商 | Agent 能力 | 缺什么 |
|------|------|-----------|--------|
| Claude Console | Anthropic | 治理 API、审计日志 | 只管自家模型 |
| OpenAI Platform | OpenAI | Assistants API、函数调用 | 只管自家模型 |
| Vertex AI Agent Builder | Google | Agent 构建、Grounding | GCP 锁定 |
| Bedrock Agents | AWS | Agent 构建、知识库 | AWS 锁定 |
| Azure AI Agent Service | Azure | Agent 构建、集成 | Azure 锁定 |
| Coze | 字节跳动 | Agent 构建、发布 | 中国市场 |

**关键洞察：** 云厂商只管自己生态里的 Agent。AgentGuard 是**跨模型、跨云、跨框架**的治理层。

---

### 五、EMQ/EMQX（IoT 数据管道，互补定位）

| 维度 | EMQ | AgentGuard |
|------|-----|-----------|
| 核心 | MQTT 数据路由 | Agent 行为治理 |
| 连接 | 100M+ 设备连接 | 100M+ Agent 动作 |
| 协议 | MQTT/CoAP/LwM2M | A2A/MCP/自定义 |
| 模块 | 124 个 app | 31 crate |
| 客户 | 44 个（13 行业） | 0 个（待开拓） |
| 商业 | 开源+企业+云 | 开源+企业（待定） |
| AI | A2A 注册表 | 治理+审计+合规 |
| 安全 | TLS/mTLS/RBAC | Prompt 防御+沙箱+PKI |

**EMQ 的 44 个客户 = AgentGuard 的潜在客户池。** 他们已有 Agent/IoT 基础设施，缺的是治理层。

---

## 第二部分：市场空白分析

### 现有竞品的 5 大盲区

| 盲区 | 说明 | AgentGuard 机会 |
|------|------|----------------|
| **1. 行为审计** | 没人管 Agent "做了什么" | AgentGuard 核心能力 |
| **2. GxP/FDA 合规** | 没有专注医疗的 Agent 治理 | 蓝海市场 |
| **3. 跨框架治理** | 每个框架只管自己 | AgentGuard 跨一切 |
| **4. 自主度控制** | 没人做 Suggest/Auto/Full 三模式 | 独特卖点 |
| **5. 成本归因** | 没人做每 Agent 每任务成本 | CFO 最爱 |

### 对比矩阵：AgentGuard vs 所有竞品

| 能力 | Guardrails | NeMo | LangSmith | Datadog | AgentGuard |
|------|-----------|------|-----------|---------|-----------|
| 输入过滤 | ✅ | ✅ | ❌ | ❌ | ✅ |
| 输出校验 | ✅ | ✅ | ❌ | ❌ | ✅ |
| 行为审计 | ❌ | ❌ | ⚠️ | ⚠️ | ✅ |
| 合规追踪 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 自主度控制 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 成本归因 | ❌ | ❌ | ⚠️ | ⚠️ | ✅ |
| 沙箱隔离 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 数字签名 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 跨框架 | ✅ | ❌ | ❌ | ✅ | ✅ |
| GxP 合规 | ❌ | ❌ | ❌ | ❌ | ✅ |

---

## 第三部分：超越方案

### 战略定位

```
AgentGuard = Agent 的 "SAP + JIRA + Splunk"

SAP  → 合规管理（GxP/FDA/EU AI Act）
JIRA → 行为追踪（谁在什么时候做了什么）
Splunk → 可观测性（实时监控 + 异常检测）
```

### 超越路线图（6 个月）

#### Phase 1: 核心壁垒（Month 1-2）

**目标：建立别人抄不了的核心能力**

| 任务 | 优先级 | 工作量 | 竞品做不到 |
|------|--------|--------|-----------|
| 行为审计引擎 | P0 | 2 周 | 竞品只管 LLM 调用，我们管 Agent 全行为 |
| 三模式自主度 | P0 | 1 周 | 没人做 Suggest/Auto/Full |
| 成本归因引擎 | P0 | 1 周 | 每 Agent 每任务 token 成本 |
| Prompt Injection 防御 | P0 | 1 周 | 运行时检测，不是静态规则 |
| Agent 沙箱隔离 | P0 | 1 周 | seccomp + cgroup |

#### Phase 2: 合规护城河（Month 2-3）

**目标：GxP/FDA 合规 = 谁也抢不走的市场**

| 任务 | 优先级 | 工作量 | 商业价值 |
|------|--------|--------|----------|
| EU AI Act 自动合规 | P0 | 2 周 | 欧洲市场入场券 |
| 21 CFR Part 11 电子签名 | P0 | 1 周 | 医疗市场入场券 |
| Annex IV 报告生成 | P1 | 1 周 | 自动化合规报告 |
| RBAC 审计 | P0 | 1 周 | 企业必备 |
| ISO 42001 AI 管理体系 | P1 | 1 周 | 国际认证 |

#### Phase 3: 生态集成（Month 3-4）

**目标：接入所有主流框架，成为"治理层标准"**

| 任务 | 优先级 | 工作量 | 集成目标 |
|------|--------|--------|----------|
| LangChain 回调 | P0 | 3 天 | 最大 Agent 框架 |
| Dify 插件 | P0 | 1 周 | 最大 Agent 平台 |
| AutoGen 集成 | P1 | 3 天 | 微软生态 |
| CrewAI 集成 | P1 | 3 天 | 角色编排 |
| OpenTelemetry 导出 | P0 | 1 周 | 可观测性标准 |
| Kafka 审计桥接 | P1 | 1 周 | 企业数据管道 |

#### Phase 4: 商业化（Month 4-5）

**目标：从开源到付费**

| 任务 | 优先级 | 工作量 | 商业模式 |
|------|--------|--------|----------|
| 企业版差异化 | P0 | 2 周 | RBAC + 多租户 + SLA |
| 云服务 MVP | P1 | 2 周 | 托管版，按 Agent 数收费 |
| 定价模型 | P0 | 1 周 | 开源免费 / 企业 $X/Agent/月 |
| 行业方案包 | P1 | 1 周 | 医疗/金融/制造 |

#### Phase 5: 市场推广（Month 5-6）

**目标：获取前 10 个客户**

| 任务 | 优先级 | 工作量 | 目标 |
|------|--------|--------|------|
| EMQ 客户转化 | P0 | 持续 | 44 个已有客户 |
| 开源社区建设 | P0 | 持续 | GitHub Stars → 企业转化 |
| 顶会论文 | P0 | 持续 | 学术背书 |
| 行业会议 | P1 | 持续 | KubeCon / RSA / HIMSS |

---

## 第四部分：技术超越方案

### 4.1 竞品没有的 10 个杀手级功能

| # | 功能 | 竞品状态 | AgentGuard 方案 | 差异化 |
|---|------|---------|----------------|--------|
| 1 | 行为审计图 | ❌ 没人有 | AccountabilityGraph：因果归因 | 论文可发 |
| 2 | 三模式自主度 | ❌ 没人有 | Suggest/Auto/Full + 渐进信任 | 独特卖点 |
| 3 | GxP 合规 | ❌ 没人做 | ALCOA+ 审计 + 电子签名 | 蓝海市场 |
| 4 | 成本归因 | ⚠️ 浅层 | 每 Agent 每任务 token 追踪 | CFO 最爱 |
| 5 | Agent 沙箱 | ❌ 没人做 | seccomp + cgroup + 资源限制 | 安全壁垒 |
| 6 | Prompt 防御 | ⚠️ 静态 | 运行时多层检测 | 更强 |
| 7 | 数字签名 | ❌ 没人做 | PKI + 不可否认性 | 合规必备 |
| 8 | 能力图谱 | ❌ 没人有 | Agent 技能依赖映射 | 运维利器 |
| 9 | 异常检测 | ⚠️ 通用 | Agent 行为统计离群点 | 专用 |
| 10 | 跨框架治理 | ❌ 没人做 | LangChain/Dify/AutoGen 统一治理 | 平台级 |

### 4.2 论文驱动的创新（292 篇论文 → 代码）

| 论文 | 核心思想 | AgentGuard 实现 |
|------|---------|----------------|
| Governance by Construction | 5 层治理检查点 | IntentGuard 中间件 |
| Mechanical Enforcement | 治理解耦 | 硬编码门禁 |
| Progressive Autonomy | 信任校准 | 三模式自主度 |
| Agent Security is Systems Problem | 系统级安全 | 沙箱 + mTLS |
| SSGM Framework | 记忆治理 | 纵向记忆安全 |
| CASPIAN | 级联攻击检测 | 跨通道因果监控 |
| PropGuard | 传播感知探索 | 传播修复 |
| Code as Agent Harness | 代码即治理 | Rust 硬编码 |

---

## 第五部分：顶会论文规划

### 目标会议

| 会议 | 方向 | CCF | 接受率 | Deadline |
|------|------|-----|--------|----------|
| USENIX Security | 系统安全 | A | 16% | 2月/6月/10月 |
| CCS | 通信安全 | A | 16% | 5月 |
| S&P | 安全综合 | A | 14% | 5月/11月 |
| NDSS | 网络安全 | A | 15% | 7月 |
| ICSE | 软件工程 | A | 20% | 9月 |
| FSE | 软件工程 | A | 20% | 5月 |
| ASE | 自动化软件工程 | A | 20% | 5月 |
| NeurIPS | 机器学习 | A | 26% | 5月 |
| ICLR | 表示学习 | A | 30% | 9月 |

### 3 个论文选题

#### 论文 1：AgentGuard — Runtime Governance for Autonomous AI Agents
- **目标会议：** USENIX Security 2026 / CCS 2026
- **核心贡献：**
  1. 形式化定义 Agent 行为治理问题
  2. 提出三模式自主度控制模型
  3. 实现 AccountabilityGraph 因果归因
  4. 在 182K LOC Rust 代码 + 4752 测试上验证
- **独特优势：** 唯一一个有真实大规模 Agent 治理系统数据的论文
- **预计页数：** 12 页

#### 论文 2：Harness Engineering — A Methodology for Safe Agent Deployment
- **目标会议：** ICSE 2027 / FSE 2027
- **核心贡献：**
  1. 定义 Harness Engineering 方法论
  2. 提出机器可读安全门禁规范
  3. 在 GxP 合规场景验证
  4. 与 EMQ/IoT 场景对比
- **独特优势：** 有真实医疗行业合规数据
- **预计页数：** 10 页

#### 论文 3：Mechanical Enforcement of Agent Governance via Rust Type System
- **目标会议：** NeurIPS 2026 / ICLR 2027
- **核心贡献：**
  1. 利用 Rust 类型系统实现编译期治理
  2. 零运行时开销的安全保证
  3. 形式化证明治理属性
  4. 与 Python/Node.js 方案对比
- **独特优势：** 唯一用系统语言实现 Agent 治理的方案
- **预计页数：** 8 页

---

## 第六部分：商业超越方案

### 目标客户画像

| 客户类型 | 痛点 | AgentGuard 解决方案 | 定价 |
|----------|------|-------------------|------|
| 医疗器械公司 | FDA 审计要求 | GxP 合规 + 审计追踪 | $50K/年 |
| 制药企业 | GAMP5 要求 | 生命周期管理 + 验证 | $80K/年 |
| 金融机构 | 监管合规 | RBAC + 审计 + 成本归因 | $60K/年 |
| AI 初创公司 | 安全担忧 | 快速集成 + 合规证明 | $10K/年 |
| 云服务商 | 多租户治理 | 白标集成 | $100K+/年 |

### EMQ 客户转化策略

```
EMQ 44 个客户 → AgentGuard 潜在客户池

转化路径：
1. MQTT 数据管道已有 → 缺 Agent 治理层
2. AgentGuard 提供 Agent 行为审计
3. 与 EMQ 集成：Agent 动作 → EMQ 路由 → AgentGuard 审计
4. 联合销售：EMQ + AgentGuard = 完整 Agent 基础设施
```

### 开源策略

```
Phase 1: 开源核心（现在）
  - 审计引擎、自主度控制、基础护栏
  - 目标：GitHub Stars → 社区

Phase 2: 企业版差异化（Month 3）
  - 多租户、RBAC、SLA、优先支持
  - 目标：付费转化

Phase 3: 云服务（Month 5）
  - 托管版，按 Agent 数收费
  - 目标：规模化收入
```

---

## 第七部分：执行计划

### 立即执行（本周）

| # | 任务 | 负责 | 产出 |
|---|------|------|------|
| 1 | 修复编译错误 | dev-1~5 | 全绿测试 |
| 2 | 行为审计引擎 MVP | dev-1 | 可演示 |
| 3 | 三模式自主度 | dev-2 | 可演示 |
| 4 | 成本归因 MVP | dev-3 | 可演示 |
| 5 | Prompt 防御 MVP | dev-4 | 可演示 |
| 6 | LangChain 集成 | dev-5 | 可集成 |

### 本月目标

| 指标 | 当前 | 目标 |
|------|------|------|
| 测试 | 4752 | 5500+ |
| Crate | 31 | 35 |
| unwrap | 3585 | <1000 |
| 功能完整度 | 60% | 85% |
| 客户 | 0 | 3 个 POC |

### 6 个月目标

| 指标 | 目标 |
|------|------|
| GitHub Stars | 1000+ |
| 企业客户 | 10+ |
| 论文 | 2 篇投稿 |
| 收入 | $100K ARR |
| 竞品差距 | 全面超越 |

---

## 附录：竞品源码参考

```
/mnt/reference-projects/
├── casbin-rs/      # RBAC 权限控制
├── cockpit/        # Linux Web 管理
├── awx/            # Ansible Web UI
└── docs/           # 文档管理
```

## 附录：292 篇论文索引

见 `docs/papers/paper-index.md`

## 附录：EMQ 竞品分析

见 `docs/competitive-analysis-emq-detailed.md`
