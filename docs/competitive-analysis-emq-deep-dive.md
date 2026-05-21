# EMQ 深度竞品分析与 AgentGuard 超越方案

> 更新日期：2026-05-21
> 数据来源：EMQX 6.2 Release Notes + GitHub 源码分析 + 客户案例研究

---

## 一、EMQ 公司画像

| 维度 | 数据 |
|------|------|
| 成立时间 | 2012 年（GitHub 首次提交 2012-12-17） |
| 核心语言 | Erlang/OTP（高并发、软实时、分布式） |
| GitHub Stars | 16,298 |
| Forks | 2,504 |
| 开源许可 | BSL 1.1（Business Source License）→ 4 年后转 Apache 2.0 |
| 模块数量 | **117 个 app**（apps/ 目录下） |
| 定位 | "AI 与 IoT 数据流的统一 MQTT 平台" |
| 员工规模 | 200+（推测，基于产品线复杂度） |

### 商业模式

```
开源版（Apache 2.0）
  ↓ 社区用户 → GitHub Stars → 品牌认知
企业版（BSL 1.1，收费）
  ↓ 企业功能：RBAC、审计、集群、SSO、合规
云服务（EMQX Cloud / Platform）
  ↓ 按连接数/消息量收费 → 规模化收入
```

**关键洞察**：EMQ 的飞轮 = 开源获客 → 企业版转化 → 云服务规模化。AgentGuard 应复制此模式。

---

## 二、EMQX 产品矩阵

| 产品 | 定位 | 部署方式 | 目标场景 |
|------|------|----------|----------|
| EMQX Open Source | 高性能 MQTT Broker | 自部署 | 开发/中小规模 |
| EMQX Enterprise | 企业级 MQTT 平台 | 自部署 | 大规模生产 |
| EMQX Cloud | 全托管 MQTT 服务 | SaaS | 快速上线 |
| EMQX Platform | 私有化 PaaS | 自部署 | 运营商/大型企业 |
| Neuron | 工业协议网关 | 边缘设备 | 工业 IoT |
| NanoMQ | 轻量级 MQTT Broker | 边缘/嵌入式 | 资源受限设备 |
| EMQX Edge | 边缘计算版 | 边缘网关 | 边缘场景 |

### 产品架构

```
┌─────────────────────────────────────────────┐
│              EMQX Platform                   │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐ │
│  │Dashboard │  │ REST API│  │ CLI (emqx   │ │
│  │  (Web)   │  │  管理   │  │  ctl)       │ │
│  └─────────┘  └─────────┘  └─────────────┘ │
│  ┌──────────────────────────────────────┐   │
│  │         MQTT 5.0 Broker Core         │   │
│  │  (Erlang/OTP 分布式、软实时)          │   │
│  └──────────────────────────────────────┘   │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────────┐  │
│  │ Auth │ │ACL   │ │Rule  │ │Bridge    │  │
│  │认证  │ │授权  │ │Engine│ │数据桥接   │  │
│  └──────┘ └──────┘ └──────┘ └──────────┘  │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────────┐  │
│  │Gateway│ │A2A   │ │UNS   │ │Schema    │  │
│  │多协议 │ │Reg.  │ │Gov.  │ │Validation│  │
│  └──────┘ └──────┘ └──────┘ └──────────┘  │
└─────────────────────────────────────────────┘
         ↕ MQTT/CoAP/LwM2M/NATS/STOMP
┌─────────────────────────────────────────────┐
│  Neuron (工业协议) │ NanoMQ (轻量) │ Edge   │
└─────────────────────────────────────────────┘
```

---

## 三、EMQX 6.2 核心新特性分析

### 3.1 A2A over MQTT（最大威胁）

**这是对 AgentGuard 最直接的竞争。**

EMQX 6.2 在 MQTT Broker 内置了 A2A 注册表（Registry），让 AI Agent 可以直接通过 MQTT 完成注册、发现和协作，无需额外基础设施。

#### 技术细节

- **发现机制**：Agent 发布结构化 Agent Card 到 `$a2a/v1/discovery/{org_id}/{unit_id}/{agent_id}` 作为保留消息
- **在线状态**：Broker 自动附加 `a2a-status` 用户属性（online/offline/lwt）
- **通信模式**：通过 MQTT v5 的 Response Topic + Correlation Data 实现请求/响应、流式、多轮对话
- **Schema 校验**：Agent Card 注册时可校验合规性
- **管理界面**：Dashboard + `emqx ctl a2a-registry` CLI
- **机器可读 API**：新增 `/api-spec.md` 和 `/api-spec.html`，Claude Code/Codex 可直接调用

#### 典型场景（EMQX 官方示例）

> 工厂自动化：监控 Agent 检测到 7 号电机异常振动 → 通过订阅发现主题找到维修 Agent → 维修 Agent 流式推送状态（"分析振动特征" → "检测到轴承磨损"）→ 触发工单。两个 Agent 互不知晓对方网络地址，EMQX 认证授权统一生效。

#### 对 AgentGuard 的威胁

| 能力 | EMQX 6.2 | AgentGuard | 差距 |
|------|-----------|------------|------|
| Agent 注册/发现 | ✅ 内置 A2A Registry | ❌ 无 | **落后** |
| Agent 在线感知 | ✅ 自动附加状态 | ❌ 无 | **落后** |
| Agent 通信 | ✅ MQTT v5 请求/响应 | ⚠️ HTTP only | **落后** |
| Schema 校验 | ✅ 注册时校验 | ❌ 无 | **落后** |
| 边缘 Agent 支持 | ✅ MQTT 天然适合 | ❌ 无 | **落后** |
| Agent 行为审计 | ❌ 不管行为做了什么 | ✅ 核心能力 | **领先** |
| 合规治理 | ❌ 不管合规 | ✅ GxP/FDA | **领先** |
| 自主度控制 | ❌ 无 | ✅ 三模式 | **领先** |

**结论**：EMQX 做了 Agent 的"连接层"，但没做"治理层"。AgentGuard 要么集成 EMQX（互补），要么自建 A2A + 治理（竞争）。

### 3.2 订阅层面的消息过滤

- 客户端订阅时附加查询后缀：`sensor/+/temperature?location=roomA&value>25`
- Broker 在投递前过滤，节省带宽和客户端负载
- **AgentGuard 可借鉴**：Agent 订阅事件流时，可按风险等级、Agent ID、操作类型过滤

### 3.3 UNS 统一命名空间治理

- 在 ACL 检查阶段强制主题结构规范
- 不合规的发布在源头即被拦截（fail-fast）
- Payload Schema 校验可选
- **AgentGuard 可借鉴**：Agent 操作必须符合预定义的行为规范，不合规操作在执行前拦截

### 3.4 动态 Keep Alive 管理

- 运行时调整客户端心跳间隔，无需断开重连
- 批量更新设备集群
- **AgentGuard 可借鉴**：根据 Agent 风险等级动态调整监控频率

---

## 四、EMQX 源码架构分析（117 个 app）

### 4.1 核心模块

| 模块 | 职责 | 代码量级 |
|------|------|----------|
| `emqx` | Broker 核心（MQTT 协议处理、会话管理、消息路由） | 最大 |
| `emqx_conf` | 集群配置管理 | 中 |
| `emqx_machine` | 节点生命周期管理 | 中 |
| `emqx_management` | REST API 管理 | 中 |
| `emqx_dashboard` | Web 管理界面 | 中 |
| `emqx_ctl` | CLI 命令行工具 | 小 |

### 4.2 认证/授权体系（8 个模块）

```
emqx_auth (核心认证框架)
├── emqx_auth_http      (HTTP 认证)
├── emqx_auth_jwt       (JWT 认证)
├── emqx_auth_ldap      (LDAP 认证)
├── emqx_auth_mnesia    (内置数据库认证)
├── emqx_auth_mongodb   (MongoDB 认证)
├── emqx_auth_mysql     (MySQL 认证)
├── emqx_auth_postgresql (PostgreSQL 认证)
├── emqx_auth_redis     (Redis 认证)
├── emqx_auth_cinfo     (客户端信息认证)
└── emqx_auth_kerberos  (Kerberos 认证)
```

**对比 AgentGuard**：EMQ 支持 10+ 种认证后端，AgentGuard 目前只有 JWT + GxP Auth。差距明显。

### 4.3 数据桥接（50+ 个模块）

EMQ 的数据集成能力极强，支持 50+ 种目标：

| 类别 | 支持的目标 |
|------|-----------|
| 消息队列 | Kafka, Pulsar, RabbitMQ, RocketMQ, NATS |
| 时序数据库 | InfluxDB, Timescale, TDengine, QuasarDB, GreptimeDB |
| 关系数据库 | PostgreSQL, MySQL, Oracle, SQL Server, CockroachDB |
| 云服务 | AWS Kinesis, GCP Pub/Sub, Azure Event Hub/Grid, S3 |
| 大数据 | ClickHouse, Doris, Snowflake, BigQuery, Redshift |
| 搜索引擎 | Elasticsearch |
| 对象存储 | S3, Azure Blob, Tablestore |

**对比 AgentGuard**：AgentGuard 的审计数据只能导出到 Kafka。差距巨大。

### 4.4 协议网关（8 种协议）

| 网关 | 协议 | 场景 |
|------|------|------|
| `emqx_gateway_coap` | CoAP | 轻量级 IoT |
| `emqx_gateway_lwm2m` | LwM2M | 设备管理 |
| `emqx_gateway_mqttsn` | MQTT-SN | 传感器网络 |
| `emqx_gateway_stomp` | STOMP | 消息中间件 |
| `emqx_gateway_nats` | NATS | 微服务 |
| `emqx_gateway_ocpp` | OCPP | 充电桩 |
| `emqx_gateway_gbt32960` | GB/T 32960 | 国标车联网 |
| `emqx_gateway_jt808` | JT/T 808 | 部标车载终端 |
| `emqx_gateway_exproto` | 自定义协议 | 扩展 |

**对比 AgentGuard**：AgentGuard 只支持 HTTP/REST。EMQ 的多协议能力是护城河。

### 4.5 其他关键模块

| 模块 | 职责 | AgentGuard 对应 |
|------|------|----------------|
| `emqx_audit` | 操作审计 | ✅ data-governance |
| `emqx_rule_engine` | 规则引擎 | ⚠️ workflow-engine（弱） |
| `emqx_schema_validation` | Schema 校验 | ❌ 无 |
| `emqx_schema_registry` | Schema 注册 | ❌ 无 |
| `emqx_opentelemetry` | OTel 遥测 | ⚠️ monitor（弱） |
| `emqx_prometheus` | Prometheus 指标 | ✅ monitor |
| `emqx_license` | 许可证管理 | ❌ 无 |
| `emqx_mt` | 多租户 | ❌ 无 |
| `emqx_node_rebalance` | 节点再平衡 | ❌ 无 |
| `emqx_cluster_link` | 集群互联 | ❌ 无 |
| `emqx_durable_storage` | 持久化存储 | ✅ data-store |
| `emqx_streams` | 流处理 | ❌ 无 |
| `emqx_a2a_registry` | A2A 注册表 | ❌ 无 |
| `emqx_ai_completion` | AI 补全 | ❌ 无 |
| `emqx_uns` (插件) | UNS 治理 | ❌ 无 |

---

## 五、EMQ 客户案例深度分析

### 5.1 吉利汽车 — 车联网

| 维度 | 数据 |
|------|------|
| 行业 | 汽车制造 |
| 场景 | 车联网（V2C）、OTA、智能制造 |
| 规模 | 百万级连接 |
| 价值 | 从传统运营向智能数据驱动转型 |

**AgentGuard 机会**：车联网 Agent（自动驾驶决策、OTA 更新审批）需要行为审计和合规追溯。

### 5.2 国泰海通 — 金融交易

| 维度 | 数据 |
|------|------|
| 行业 | 金融/证券 |
| 场景 | 超低时延行情推送 |
| 规模 | 高并发 |
| 价值 | 满足金融监管合规要求 |

**AgentGuard 机会**：金融 Agent（交易决策、风控）需要不可篡改的审计链和数字签名。

### 5.3 国家电网 — 电力物联网

| 维度 | 数据 |
|------|------|
| 行业 | 能源/电力 |
| 场景 | 智慧物联体系 |
| 规模 | 百万级设备 |
| 价值 | 关键基础设施实时监控 |

**AgentGuard 机会**：电力 Agent（负荷预测、故障诊断）需要关键基础设施级别的安全审计。

### 5.4 北美 Verifone — 金融支付

| 维度 | 数据 |
|------|------|
| 行业 | 金融/支付 |
| 场景 | 新一代电子支付系统 |
| 规模 | 全球部署 |
| 价值 | PCI DSS 合规 |

**AgentGuard 机会**：支付 Agent 需要 PCI DSS 级别的审计追踪和访问控制。

### 5.5 全球半导体显示龙头 — 机器人诊断

| 维度 | 数据 |
|------|------|
| 行业 | 半导体制造 |
| 场景 | 机器人实时诊断预警 |
| 规模 | 工厂级 |
| 价值 | 预测性维护 |

**AgentGuard 机会**：工业 Agent（设备诊断、维护调度）需要操作审计和安全门禁。

### 5.6 JAGAT — 社交平台

| 维度 | 数据 |
|------|------|
| 行业 | 社交/即时通讯 |
| 场景 | 大规模实时消息 |
| 规模 | 百万级连接 |
| 价值 | 无缝社交互动 |

**AgentGuard 机会**：社交 Agent（内容审核、推荐）需要行为可追溯性。

### 5.7 FoloToy — AI 玩具

| 维度 | 数据 |
|------|------|
| 行业 | 消费电子/AI 玩具 |
| 场景 | 实时互动体验 |
| 规模 | 消费级 |
| 价值 | 低延迟交互 |

**AgentGuard 机会**：面向儿童的 AI Agent 需要内容安全和合规审计（COPPA 等）。

### 5.8 伯镭科技 — 泛在机器人

| 维度 | 数据 |
|------|------|
| 行业 | 机器人/光伏 |
| 场景 | 智能光伏解决方案 |
| 规模 | 工业级 |
| 价值 | 云边协同 |

### 5.9 力氪新能源 — 充电桩

| 维度 | 数据 |
|------|------|
| 行业 | 新能源 |
| 场景 | 充电桩运营管理 |
| 规模 | 城市级 |
| 价值 | 设备管理和运营优化 |

### 5.10 淮安港航 — 智慧航运

| 维度 | 数据 |
|------|------|
| 行业 | 智慧城市/航运 |
| 场景 | 无人值守船闸 |
| 规模 | 城市级 |
| 价值 | 安全关键系统 |

---

## 六、EMQ 竞争格局（EMQ vs 其他 MQTT Broker）

| 维度 | EMQX | HiveMQ | VerneMQ | Mosquitto |
|------|------|--------|---------|-----------|
| 语言 | Erlang | Java | Erlang | C/C++ |
| 许可证 | BSL 1.1 | 商业 | Apache 2.0 | EPL/EDL |
| 集群 | ✅ 原生 | ✅ | ✅ | ❌ 无原生集群 |
| 规模 | 100M+ 连接 | 10M+ | 1M+ | 10K |
| 数据集成 | 50+ 目标 | 中等 | 少 | 无 |
| 协议网关 | 8 种 | 3 种 | 无 | 无 |
| A2A 支持 | ✅ 6.2 新增 | ❌ | ❌ | ❌ |
| 云服务 | ✅ | ✅ | ❌ | ❌ |
| AI 集成 | ✅ AI Completion | ❌ | ❌ | ❌ |

**结论**：EMQ 在 MQTT 领域已经是绝对领导者。AgentGuard 不应与其在 MQTT 层面竞争，而应在治理层建立差异化。

---

## 七、AgentGuard vs EMQ：差异化定位

### 7.1 EMQ 做了什么（连接 + 路由 + 集成）

```
设备/Agent ──MQTT──→ EMQX Broker ──规则引擎──→ 50+ 数据目标
                      │
                      ├── 认证/授权（10+ 后端）
                      ├── 多协议网关（8 种）
                      ├── A2A 注册表（Agent 发现）
                      ├── UNS 治理（主题规范）
                      └── Schema 校验
```

### 7.2 EMQ 没做什么（治理 + 审计 + 合规）

| 能力 | EMQ | AgentGuard |
|------|-----|------------|
| Agent 行为审计 | ❌ 只管消息不管行为 | ✅ 每个 Agent 操作可追溯 |
| GxP/FDA 合规 | ❌ 无医疗合规 | ✅ ALCOA+ 审计链 |
| 自主度控制 | ❌ 无 | ✅ Suggest/Auto/Full |
| 成本归因 | ❌ 不管 Agent 成本 | ✅ 每 Agent 每任务 Token 成本 |
| 数字签名 | ❌ 无不可否认性 | ✅ PKI + 电子签名 |
| Agent 沙箱 | ❌ 无执行隔离 | ✅ seccomp + cgroup |
| 行为异常检测 | ❌ 不管行为异常 | ✅ 统计离群点检测 |
| 合规报告生成 | ❌ 无 | ✅ Annex IV / 21 CFR Part 11 |

### 7.3 战略选择：互补 vs 竞争

**方案 A：互补（推荐）**
```
EMQX（连接层）──MQTT──→ AgentGuard（治理层）
  │                        │
  ├── Agent 发现/路由       ├── 行为审计
  ├── 消息路由/过滤         ├── 合规治理
  └── 数据集成             └── 成本归因
```
- AgentGuard 作为 EMQX 的治理插件/中间件
- 复用 EMQ 的客户基础（44+ 行业客户）
- 联合销售：EMQX + AgentGuard = 完整 Agent 基础设施

**方案 B：竞争**
- 自建 A2A 注册表 + MQTT Broker
- 工作量巨大，且 EMQ 已有 10 年积累
- 不推荐

**方案 C：混合（最佳）**
- 核心治理能力自研（审计、合规、自主度控制）
- A2A/MCP 协议支持（不自建 Broker，集成 EMQX）
- 提供 EMQX 插件 + 独立部署两种形态

---

## 八、可执行的超越方案

### 8.1 立即可做（本周）

| # | 任务 | 产出 | 对标 EMQ |
|---|------|------|----------|
| 1 | A2A 协议支持（轻量版） | Agent 注册/发现 API | 对标 emqx_a2a_registry |
| 2 | MCP Server 实现 | Agent 工具调用治理 | EMQ 无此能力 |
| 3 | 审计数据 Kafka 导出 | 已有 kafka_bridge | 对标 emqx_bridge_kafka |

### 8.2 短期（1 个月）

| # | 任务 | 产出 | 对标 EMQ |
|---|------|------|----------|
| 4 | OTel Agent Span 导出 | 分布式追踪 | 对标 emqx_opentelemetry |
| 5 | Schema 校验（Agent Card） | Agent 注册时校验 | 对标 emqx_schema_validation |
| 6 | 多租户支持 | 租户隔离 | 对标 emqx_mt |
| 7 | Prometheus 指标完善 | 已有基础 | 对标 emqx_prometheus |

### 8.3 中期（3 个月）

| # | 任务 | 产出 | EMQ 做不到 |
|---|------|------|-----------|
| 8 | GxP 合规模块 | ALCOA+ 审计 + 电子签名 | EMQ 无医疗合规 |
| 9 | EU AI Act 自动合规 | 风险分类 + Annex IV 报告 | EMQ 无 AI 合规 |
| 10 | Agent 行为异常检测 | 统计离群点 + 告警 | EMQ 不管行为 |
| 11 | 成本归因引擎 | 每 Agent 每任务 Token | EMQ 不管成本 |

### 8.4 长期（6 个月）

| # | 任务 | 产出 | 商业价值 |
|---|------|------|----------|
| 12 | EMQX 治理插件 | AgentGuard-as-EMQX-Plugin | 接入 EMQ 44 客户 |
| 13 | 行业方案包 | 医疗/金融/制造 | 高客单价 |
| 14 | 云服务 MVP | 托管版 | 规模化收入 |

---

## 九、EMQ 的弱点（AgentGuard 的机会）

### 9.1 只管连接不管行为

EMQ 管的是"消息从 A 到 B"，不管"Agent 做了什么"。这是根本性的差异。

### 9.2 无合规能力

EMQ 没有任何 GxP、FDA、EU AI Act 合规能力。在医疗/制药/金融行业，这是硬性要求。

### 9.3 无自主度控制

EMQ 不区分 Agent 是在"建议"还是在"自动执行"。AgentGuard 的三模式自主度是独特卖点。

### 9.4 无成本归因

EMQ 不追踪 Agent 的 Token 消耗和成本。对 CFO 来说，这是关键需求。

### 9.5 Erlang 的劣势

- Erlang 生态小，招聘困难
- 与主流 AI/ML 生态（Python/Rust）不兼容
- 性能虽好但开发效率低

**AgentGuard 用 Rust 的优势**：性能媲美 Erlang，生态更现代，安全保证更强（编译期内存安全）。

---

## 十、关键结论

1. **EMQ 已经形成规模**：16K Stars、44+ 行业客户、117 个模块、10 年积累。正面竞争不现实。

2. **EMQ 做了连接层，缺治理层**：A2A 注册表、消息路由、数据集成 —— 这些 EMQ 都做了。但 Agent 行为审计、合规治理、自主度控制、成本归因 —— 这些 EMQ 都没做。

3. **AgentGuard 的定位应该是"Agent 的治理层"**：不造轮子（不自建 MQTT Broker），造安全带（Agent 行为治理）。

4. **最佳策略是互补**：AgentGuard 作为 EMQX 的治理层插件，接入 EMQ 的 44+ 客户基础。

5. **Rust 是差异化**：唯一用系统语言实现 Agent 治理的方案，编译期安全保证，性能媲美 Erlang。

6. **GxP/FDA 是蓝海**：EMQ 完全不碰医疗/制药行业，AgentGuard 可以独占这个市场。
