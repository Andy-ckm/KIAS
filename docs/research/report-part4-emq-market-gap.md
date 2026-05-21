# 第八部分：EMQ/EMQX 深度分析

## 8.1 公司概况

| 指标 | 数据 |
|------|------|
| 公司 | EMQ Technologies（杭州） |
| 产品 | EMQX MQTT Broker |
| GitHub Stars | 16,296 |
| 语言 | Erlang/OTP |
| 许可证 | BSL 1.1 |
| 创建时间 | 2012-12 |
| 最新版本 | 6.2.0 (2026-04-28) |
| 模块数 | 124 个 apps |
| 定位 | 最可扩展的 MQTT Broker |

## 8.2 产品线

| 产品 | 定位 | 价格 |
|------|------|------|
| EMQX Open Source | 开源核心 | 免费 |
| EMQX Enterprise | 企业版 | 付费 |
| EMQX Cloud | 托管云服务 | 按用量 |
| EMQX Platform | 平台级 | 定制 |

## 8.3 核心能力（124 个模块）

### 协议支持
- MQTT 5.0 / 3.1.1 / 3.1
- MQTT over QUIC
- MQTT-SN / CoAP / LwM2M / STOMP / NATS
- OCPP（充电桩）
- JT808 / GBT32960（车联网国标）

### 认证授权（11 种）
- 内置数据库（Mnesia）
- MySQL / PostgreSQL / MongoDB / Redis
- HTTP / LDAP / JWT / Kerberos
- 客户端信息认证

### 数据桥接（50+ 连接器）
**消息队列：** Kafka, RabbitMQ, Pulsar, RocketMQ
**数据库：** PostgreSQL, MySQL, MongoDB, Redis, ClickHouse, InfluxDB, TDengine, TimescaleDB, Cassandra, DynamoDB, Oracle, SQL Server, Couchbase, Doris, GreptimeDB, QuasarDB, Redshift, Snowflake, BigQuery, AWS Timestream, Azure Blob, S3
**云服务：** AWS Kinesis, GCP Pub/Sub, Azure Event Hub, Confluent Cloud
**其他：** HTTP Webhook, MQTT Bridge, Disk Log

### 网关（10 种协议）
- CoAP / LwM2M / MQTT-SN / STOMP / NATS
- ExProto（自定义协议）
- OCPP（充电桩）
- JT808 / GBT32960（车联网国标）

### AI 集成（EMQX 6.2 新特性）
- **A2A Registry** — Agent-to-Agent 智能体发现与协作
- **A2A over MQTT** — 基于 MQTT 的 A2A 协议
- **Agent Card** — 结构化智能体描述
- **事件驱动发现** — 实时推送，无需轮询

### 可观测性
- Prometheus / Grafana / Datadog / OpenTelemetry
- 审计日志 / 实时追踪 / 慢订阅追踪
- Dashboard 管理控制台

### 安全
- TLS/SSL / WSS / PSK / mTLS
- RBAC / ACL / IP 白名单
- 客户端 ID 限制

### 部署
- Docker / Kubernetes (Helm Chart)
- 集群自动发现（DNS/K8s/etcd）
- Core + Replicant 部署模式
- 无主集群，高可用容错

## 8.4 EMQX 6.2 新特性分析

### A2A over MQTT
```
核心特性：
1. 智能体发布 Agent Card 到 $a2a/v1/discovery/{org_id}/{unit_id}/{agent_id}
2. 订阅者连接后立即获取全量已注册节点
3. 智能体上下线实时推送
4. 内置在线状态感知（online/offline/lwt）
5. Schema 校验（不合规 Agent Card 被拒绝）
6. Dashboard + CLI 管理
```

**与 AgentGuard 的关系：**
- EMQ 做 A2A 数据传输
- AgentGuard 做 A2A 行为治理
- 互补：EMQ 管数据流，AgentGuard 管合规

### 订阅层面的消息过滤
```
sensor/+/temperature?location=roomA&value>25
```
- Broker 侧过滤，节省带宽
- 降低客户端负载
- 高吞吐场景增益明显

### UNS 治理插件
- 统一命名空间治理
- ACL 检查阶段强制规范主题结构
- Payload Schema 校验
- fail-fast 策略

**与 AgentGuard 的对比：**
| 能力 | EMQ UNS | AgentGuard |
|------|---------|-----------|
| 治理对象 | MQTT Topic | Agent 行为 |
| 校验方式 | Schema | 规则引擎 |
| 合规 | 无 | GxP/FDA |
| 审计 | 基础 | 完整 |

## 8.5 客户案例（44 个）

### 行业分布
| 行业 | 客户数 | 代表客户 |
|------|--------|---------|
| 汽车/车联网 | 8+ | 吉利、路特斯、上汽大众、台铃 |
| 能源/电力 | 6+ | 国家电网、力氪新能源、尚唯斯、华北油田 |
| 金融/支付 | 3+ | 国泰海通、建信金科、Verifone |
| 工业制造 | 5+ | 半导体龙头、钢铁、食品饮料 |
| 智慧城市 | 3+ | 淮安港航、深城交、中国电信 |
| 零售/餐饮 | 2+ | 智慧餐饮 |
| 农业 | 1 | 种业育繁 |
| 消费电子 | 1 | FoloToy AI 玩具 |
| 社交 | 1 | JAGAT |
| 机器人 | 2+ | 伯镭科技、半导体龙头 |
| 物流 | 1 | 车轮运输 |
| 电信 | 2+ | 中国移动、中国电信 |
| 游戏 | 1 | Tech Sport |

### 关键客户故事
1. **吉利汽车** — 车联网，百万级连接，安全认证
2. **路特斯** — 全球智能网联汽车平台
3. **国泰海通** — 超低时延行情推送，4000 万用户
4. **国家电网** — 电力物联网
5. **FoloToy** — AI 玩具实时互动

## 8.6 商业模式分析

### 收入来源
1. **企业版许可证** — 年费
2. **云服务订阅** — 按用量
3. **技术支持** — SLA
4. **培训和咨询** — 专业服务

### 市场策略
1. **开源获客** → 社区建设 → 企业转化
2. **行业解决方案** → 垂直市场深耕
3. **生态合作** → 云厂商集成

## 8.7 EMQ 与 AgentGuard 的关系

### 互补定位
```
EMQ 做的：                    AgentGuard 做的：
─────────────────────────────────────────────────
设备→MQTT→数据管道            Agent→治理层→合规审计
百万级连接                    百万级 Agent 动作
数据路由/集成                 合规门禁/审计追踪
QoS 0/1/2                    三模式自主度
A2A over MQTT                A2A 合规治理
```

### 共享客户池
EMQ 的 44 个客户 = AgentGuard 的 44 个潜在客户
- 他们已有 MQTT 数据管道
- 他们理解 IoT/Agent 的重要性
- 他们有预算买基础设施软件
- 他们缺的是 Agent 动作的合规治理层

### 集成方案
```
Agent 动作 → EMQX 路由 → AgentGuard 审计 → 合规报告
```

---

# 第九部分：市场空白分析

## 9.1 现有竞品的 5 大盲区

| 盲区 | 说明 | AgentGuard 机会 |
|------|------|----------------|
| **1. 行为审计** | 没人管 Agent "做了什么" | AgentGuard 核心能力 |
| **2. GxP/FDA 合规** | 没有专注医疗的 Agent 治理 | 蓝海市场 |
| **3. 跨框架治理** | 每个框架只管自己 | AgentGuard 跨一切 |
| **4. 自主度控制** | 没人做 Suggest/Auto/Full 三模式 | 独特卖点 |
| **5. 成本归因** | 没人做每 Agent 每任务成本 | CFO 最爱 |

## 9.2 竞品能力对比矩阵

| 能力 | Guardrails | NeMo | LangSmith | Datadog | LiteLLM | AgentGuard |
|------|-----------|------|-----------|---------|---------|-----------|
| 输入过滤 | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ |
| 输出校验 | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ |
| 行为审计 | ❌ | ❌ | ⚠️ | ⚠️ | ❌ | ✅ |
| 合规追踪 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 自主度控制 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 成本归因 | ❌ | ❌ | ⚠️ | ⚠️ | ⚠️ | ✅ |
| 沙箱隔离 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 数字签名 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 跨框架 | ✅ | ❌ | ❌ | ✅ | ✅ | ✅ |
| GxP 合规 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 性能 | Python | Python | API | API | Python | Rust |

## 9.3 AgentGuard 的 10 个差异化能力

| # | 能力 | 竞品状态 | AgentGuard 方案 | 差异化 |
|---|------|---------|----------------|--------|
| 1 | 行为审计图 | ❌ 没人有 | AccountabilityGraph | 论文可发 |
| 2 | 三模式自主度 | ❌ 没人有 | Suggest/Auto/Full | 独特卖点 |
| 3 | GxP 合规 | ❌ 没人做 | ALCOA+ 审计 | 蓝海市场 |
| 4 | 成本归因 | ⚠️ 浅层 | 每 Agent 每任务 | CFO 最爱 |
| 5 | Agent 沙箱 | ❌ 没人做 | seccomp + cgroup | 安全壁垒 |
| 6 | Prompt 防御 | ⚠️ 静态 | 运行时多层检测 | 更强 |
| 7 | 数字签名 | ❌ 没人做 | PKI + 不可否认性 | 合规必备 |
| 8 | 能力图谱 | ❌ 没人有 | Agent 技能依赖映射 | 运维利器 |
| 9 | 异常检测 | ⚠️ 通用 | Agent 行为统计离群点 | 专用 |
| 10 | 跨框架治理 | ❌ 没人做 | 统一治理 | 平台级 |

## 9.4 论文支撑（294 篇论文）

### 关键论文
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
| TrustAgent | 动态信誉评分 | 信誉系统 |
| AgentSafetyBench | 安全基准 | 测试框架 |

### 论文驱动创新
294 篇论文 → 提取 actionable insights → 实现为 Rust 代码 → 测试验证
