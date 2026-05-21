# EMQX 竞品深度分析报告
> 生成时间: 2026-05-21
> 数据来源: emqx.com 全站 + GitHub API

## 一、公司概况

| 指标 | 数据 |
|------|------|
| GitHub Stars | 16,296 |
| Forks | 2,504 |
| 主语言 | Erlang |
| 许可证 | BSL 1.1 (v5.9.0 起统一开源+企业版) |
| 创建时间 | 2012-12-17 |
| 最近更新 | 2026-05-21 |
| 模块数 | 124 个 apps |
| 定位 | "The most scalable and reliable MQTT broker for AI, IoT, IIoT and connected vehicles" |

## 二、产品线

1. **EMQX Open Source** — 开源核心，社区版
2. **EMQX Enterprise** — 企业版（v5.9.0 起合并到统一版本）
3. **EMQX Cloud** — 托管云服务
4. **EMQX Platform** — 平台级产品

## 三、核心能力（124 个模块分析）

### 3.1 协议支持
- MQTT 5.0 / 3.1.1 / 3.1
- MQTT over QUIC
- MQTT-SN / CoAP / LwM2M / STOMP / NATS / OCPP
- JT808 / GBT32960（车联网国标）

### 3.2 认证授权（11 种）
- emqx_auth_mnesia（内置数据库）
- emqx_auth_mysql / postgresql / mongodb / redis
- emqx_auth_http / ldap / jwt / kerberos
- emqx_auth_cinfo（客户端信息）

### 3.3 数据桥接（50+ 连接器）
**消息队列**: Kafka, RabbitMQ, Pulsar, RocketMQ
**数据库**: PostgreSQL, MySQL, MongoDB, Redis, ClickHouse, InfluxDB, TDengine, TimescaleDB, Cassandra, DynamoDB, Oracle, SQL Server, Couchbase, Doris, GreptimeDB, QuasarDB, Redshift, Snowflake, BigQuery, AWS Timestream, Azure Blob, S3
**云服务**: AWS Kinesis, GCP Pub/Sub, Azure Event Hub, Confluent Cloud
**其他**: HTTP Webhook, MQTT Bridge, Disk Log

### 3.4 网关（10 种协议）
- CoAP / LwM2M / MQTT-SN / STOMP / NATS
- ExProto（自定义协议）
- OCPP（充电桩）
- JT808 / GBT32960（车联网国标）

### 3.5 AI 集成
- emqx_ai_completion — 原生 AI 处理
- emqx_a2a_registry — A2A 智能体发现与协作（v6.2 新特性）
- AI-driven decision making at edge/cloud

### 3.6 可观测性
- Prometheus / Grafana / Datadog / OpenTelemetry
- 审计日志 / 实时追踪 / 慢订阅追踪
- Dashboard 管理控制台

### 3.7 安全
- TLS/SSL / WSS / PSK / mTLS
- RBAC / ACL / IP 白名单
- 客户端 ID 限制

### 3.8 部署
- Docker / Kubernetes (Helm Chart)
- 集群自动发现（DNS/K8s/etcd）
- Core + Replicant 部署模式
- 无主集群，高可用容错

## 四、客户故事（44 个）

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

### 客户故事完整列表
1. 吉利汽车：车联网，百万级连接，安全认证
2. 路特斯：全球智能网联汽车平台
3. 上汽大众：智能制造
4. 台铃科技：电动车智能化
5. 国泰海通：超低时延行情推送，4000万用户
6. 建信金科：金融科技
7. Verifone：金融电子支付
8. 国家电网信通院：电力物联网
9. 力氪新能源：充电桩运营
10. 尚唯斯：光伏运维
11. 华北油田：石油物联网
12. 半导体显示龙头：机器人实时诊断预警
13. 钢铁行业：数字化平台
14. 全球食品巨头：预测性维护
15. 智能制造平台：工业物联网
16. 淮安港航：无人值守船闸
17. 深城交：智慧城市
18. 中国电信：电信物联网
19. 中国移动：移动物联网
20. 智慧餐饮：实时点餐同步
21. 农业物联网：种业育繁
22. FoloToy：AI 玩具实时互动
23. JAGAT：社交互动
24. 伯镭科技：智能光伏
25. 黑马高科：光伏电站运维
26. 常州皓鸣：钢铁数字化
27. 上海研博：水务物联网
28. 力氪新能源：充电桩
29. 尚唯斯：光伏
30. 台铃：电动车
31. 路特斯：跑车
32. 更多...

## 五、解决方案（35+ 场景）

### 使用场景
- 互联机器人、数据中心、车联网、车队管理
- 车队遥测、车队管理、智能制造
- 软件定义汽车、工业物联网、边缘计算
- 统一命名空间

### 技术方向
- 实时 AI（MQTT 数据流驱动 AI）
- 物联网、软件定义汽车、工业物联网

### 云集成
- MQTT for AWS / Azure / Google Cloud / Oracle
- MQTT on Kubernetes / MQTT Sparkplug

### 数据集成
- MQTT → Kafka / ClickHouse / Snowflake / 时序数据库

### 行业
- 汽车、金融、医疗、零售、能源、电信、交通、游戏

## 六、商业模式分析

### 定价策略
- **开源版**: 免费，核心功能
- **企业版**: 付费，高级功能（v5.9.0 起合并）
- **云服务**: 按用量付费

### 收入来源
1. 企业版许可证
2. 云服务订阅
3. 技术支持
4. 培训和咨询

### 市场策略
1. 开源获客 → 社区建设 → 企业转化
2. 行业解决方案 → 垂直市场深耕
3. 生态合作 → 云厂商集成

## 七、技术架构分析

### 核心架构
- **语言**: Erlang/OTP（高并发、容错、热更新）
- **协议**: MQTT 5.0 为核心
- **集群**: 无主集群，Masterless
- **存储**: Mnesia（内置）、外部数据库
- **扩展**: 插件架构 + Hooks

### 关键技术指标
- 100M+ 并发连接
- 百万级消息/秒
- 亚毫秒级延迟
- 99.99% 可用性

### 模块分类
```
核心: emqx, emqx_conf, emqx_machine, emqx_utils
认证: emqx_auth_* (11种)
桥接: emqx_bridge_* (50+种)
网关: emqx_gateway_* (10种)
监控: emqx_prometheus, emqx_opentelemetry, emqx_telemetry
安全: emqx_psk, emqx_license
管理: emqx_dashboard, emqx_management, emqx_ctl
AI: emqx_ai_completion, emqx_a2a_registry
```

## 八、AgentGuard 超越路线

### 8.1 互补定位
```
EMQX 做的:                AgentGuard 做的:
─────────────────────────────────────────────────
设备→MQTT→数据管道          Agent→治理层→合规审计
百万级连接                  百万级 Agent 动作
数据路由/集成               合规门禁/审计追踪
QoS 0/1/2                  三模式自主度
A2A over MQTT              A2A 合规治理
```

### 8.2 共享客户池
EMQ 的 44 个客户 = AgentGuard 的 44 个潜在客户
- 他们已有 MQTT 数据管道
- 他们理解 IoT/Agent 的重要性
- 他们有预算买基础设施软件
- 他们缺的是 Agent 动作的合规治理层

### 8.3 差异化优势
1. **GxP/FDA 合规** — EMQ 没有的
2. **审计追踪** — AgentGuard 的核心能力
3. **自主度控制** — 三模式（Suggest/Auto/Full）
4. **Rust 实现** — 内存安全、高性能
5. **机器可读门禁** — 自动化合规检查

### 8.4 学习 EMQ 的地方
1. **开源+企业+云** 三级商业模式
2. **行业解决方案** 垂直市场深耕
3. **124 个模块** 的插件化架构
4. **50+ 数据桥接** 的生态集成
5. **A2A 协议** 的智能体协作

## 九、开发计划

### Phase 1: 核心能力对标（2 周）
- [ ] 多认证提供者（LDAP/JWT/OAuth2.0/SCRAM）
- [ ] 实时监控 Dashboard（Prometheus/Grafana）
- [ ] 审计数据集成（Kafka/ClickHouse/PostgreSQL）
- [ ] 集群模式（多节点部署、自动发现）

### Phase 2: 行业解决方案（4 周）
- [ ] 车联网 Agent 合规治理
- [ ] 金融 Agent 合规治理
- [ ] 能源 Agent 合规治理
- [ ] 医疗 Agent 合规治理
- [ ] 制造 Agent 合规治理

### Phase 3: 生态集成（4 周）
- [ ] A2A 协议支持
- [ ] 数据桥接（Kafka/RabbitMQ/Pulsar）
- [ ] 云厂商集成（AWS/Azure/GCP）
- [ ] 插件架构

### Phase 4: 商业化（2 周）
- [ ] 定价模型设计
- [ ] 企业版差异化功能
- [ ] 云服务部署方案
- [ ] 行业案例包装

### Phase 5: 市场推广（持续）
- [ ] 开源社区建设
- [ ] 行业会议参与
- [ ] 技术博客输出
- [ ] 客户案例收集
