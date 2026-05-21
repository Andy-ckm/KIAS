# AgentGuard 开发计划（基于 EMQ 竞品分析）

> 日期：2026-05-21
> 目标：补齐与 EMQ 的差距，建立 Agent 治理层护城河

---

## Phase 1: A2A 协议支持（本周）

### 1.1 Agent 注册表 (a2a-registry)
- [ ] Agent Card 数据结构（JSON Schema）
- [ ] 注册/发现/注销 API
- [ ] 在线状态感知（heartbeat → online/offline/lwt）
- [ ] Agent Card Schema 校验
- 模块：新建 `crates/a2a-registry/`

### 1.2 Agent 通信治理
- [ ] 请求/响应追踪（每次 Agent 间调用可审计）
- [ ] 通信链路因果归因（AccountabilityGraph）
- 模块：扩展 `crates/data-governance/`

---

## Phase 2: OTel + 可观测性（本周-下周）

### 2.1 OTel Agent Span 导出
- [ ] OpenTelemetry Span 格式（Agent 操作 → OTel Span）
- [ ] OTLP gRPC/HTTP 导出
- [ ] 与 Jaeger/Zipkin/Grafana 集成
- 模块：扩展 `crates/monitor/`

### 2.2 Prometheus 指标完善
- [ ] Agent 操作延迟 histogram
- [ ] Token 消耗 counter
- [ ] 合规事件 gauge
- 模块：已有 `crates/monitor/src/prometheus.rs`

---

## Phase 3: 合规护城河（下周-下月）

### 3.1 GxP 合规模块
- [ ] ALCOA+ 审计链（Attributable, Legible, Contemporaneous, Original, Accurate）
- [ ] 21 CFR Part 11 电子签名
- [ ] Annex IV 报告生成
- 模块：扩展 `crates/compliance-security/`

### 3.2 EU AI Act 自动合规
- [ ] 风险分类（不可接受/高/有限/最小）
- [ ] Annex IV 自动生成
- [ ] 透明度义务检查
- 模块：已有 `crates/compliance-security/src/eu_ai_act.rs`

---

## Phase 4: 成本归因 + 异常检测（下月）

### 4.1 成本归因引擎
- [ ] 每 Agent 每任务 Token 追踪
- [ ] 预算告警
- [ ] 成本异常检测
- 模块：已有 `crates/data-governance/src/cost_attribution.rs`

### 4.2 Agent 行为异常检测
- [ ] 操作频率统计基线
- [ ] 离群点检测（Z-score / IQR）
- [ ] 实时告警
- 模块：新建 `crates/anomaly-detection/`

---

## 开发顺序（严格按优先级）

1. **a2a-registry**（Agent 注册表）— 对标 EMQ 最核心功能
2. **OTel Span 导出**（可观测性）— 企业必备
3. **GxP 合规**（护城河）— EMQ 做不到
4. **成本归因**（商业价值）— CFO 最爱
5. **异常检测**（差异化）— EMQ 做不到
