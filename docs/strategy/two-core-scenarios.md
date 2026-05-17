# KIAS 两大核心场景

## 钱学森系统工程视角

KIAS 是 Long-running Agents 的基础设施。从系统工程角度，Agent 系统的本质是：

```
输入（任务/目标）→ 处理（Agent 集群协作）→ 输出（结果/价值）
反馈（观察/度量）→ 优化（自我进化）→ 再输入（正向循环）
```

两大核心场景覆盖 Agent 生命周期的全部维度：

---

## 场景一：基础设施编排（Like K8s）

### 本质
把 Agent 当作"容器"来管理——声明式定义、自动调度、自愈恢复、弹性伸缩。

### 核心能力矩阵

| 能力 | 对标 K8s 概念 | KIAS 实现 |
|------|-------------|-----------|
| 声明式定义 | YAML Deployment | Agent YAML manifest，定义期望状态 |
| 自动调度 | kube-scheduler | AgentScheduler，基于能力/负载/亲和性分配任务 |
| 自愈恢复 | livenessProbe + restartPolicy | HealthChecker + Reconciler 循环检测 → 自动修复 |
| 弹性伸缩 | HPA / Cluster Autoscaler | 负载指标驱动 Agent 数量自动调整 |
| 滚动更新 | RollingUpdate | Agent 版本无缝切换，零停机 |
| 资源隔离 | Namespace / ResourceQuota | Sandbox（5 后端）+ RBAC + 租户隔离 |
| 服务发现 | Service / DNS | AgentRegistry + 能力索引 + 智能路由 |
| 配置管理 | ConfigMap / Secret | CredentialManager（AES-256-GCM）+ 热加载 |
| 全链路追踪 | Jaeger / OpenTelemetry | TracingCollector + Metrics + 审计日志 |
| 负载均衡 | Service LoadBalancer | Agent 能力分层 + 任务队列 + 优先级调度 |
| 熔断限流 | Circuit Breaker | Resilience 模块（熔断器 + 令牌桶限流） |
| 干预控制 | kubectl apply/delete | kias-cli + API + CronJob 定时巡检 |

### 运维自动化场景

1. **集群管理**：Agent 注册 → 能力上报 → 心跳保活 → 异常驱逐 → 自动重建
2. **任务调度**：任务入队 → 能力匹配 → 优先级排序 → 分配执行 → 结果回收
3. **故障自愈**：健康检查 → 异常检测 → 根因分析 → 自动修复 → 事后报告
4. **安全合规**：凭证轮换 → 权限审计 → 沙箱隔离 → 网络策略 → 合规报告
5. **成本管控**：Token 计量 → 成本归因 → 预算告警 → 自动降级 → 优化建议
6. **多模型路由**：模型能力评估 → 成本/质量权衡 → 智能路由 → fallback 策略

---

## 场景二：自循环开发（Self-cyclic Development）

### 本质
用 Agent 开发 Agent——自创建、自编排、自测试、自部署、自进化。

### 核心能力矩阵

| 能力 | 描述 | KIAS 实现 |
|------|------|-----------|
| Agent 自创建 | 根据需求自动生成 Agent 定义 | AgentFactory + 模板系统 |
| Workflow 自编排 | 任务自动拆解为 DAG | WorkflowEngine + DAG 引擎 |
| 代码自生成 | 需求 → 代码 → 测试 → 部署 | Skill 系统 + 代码生成管线 |
| Skill 自提取 | 从历史操作中提取可复用模式 | 自蒸馏流水线（运行→观察→模式→Skill） |
| 质量自保证 | 自动测试 + 审查 + 门禁 | TDD 流程 + cargo clippy/test + 代码审查 Agent |
| 文档自维护 | 代码变更自动同步文档 | README Guardrails + 文档生成 |
| 依赖自管理 | crate 依赖自动解析 + 更新 | workspace 依赖图 + 自动更新 |
| 版本自管理 | Skill/Agent 版本控制 + 回滚 | SkillVersionHistory + content_hash |

### 开发自动化场景

1. **需求 → Agent**：自然语言描述 → 解析为任务 → 生成 Agent 定义 → 验证 → 部署
2. **任务 → Workflow**：复杂任务自动拆解 → DAG 编排 → 依赖解析 → 并行执行
3. **代码 → 测试**：代码生成 → 自动测试 → 失败修复 → 重试 → 通过
4. **操作 → Skill**：重复操作识别 → 模式提取 → Skill 生成 → 验证 → 入库
5. **变更 → 部署**：代码变更 → 质量门禁 → Git 推送 → 滚动部署 → 健康检查
6. **问题 → 修复**：异常检测 → 根因分析 → 修复方案 → 实现 → 验证 → 部署

### 自进化闭环

```
运行 Agent → 收集观察数据 → 提取成功/失败模式
     ↑                                    ↓
  部署新 Skill ← 验证 Skill ← 生成 Skill
```

每一轮循环都让系统更强：
- **Skill 库增长**：可复用能力越来越多
- **模式识别进化**：从简单模式到复杂策略
- **故障知识积累**：每次故障都转化为预防措施
- **效率持续提升**：相同任务耗时越来越短

---

## 两大场景的关系

```
场景一（基础设施）←→ 场景二（自循环开发）

基础设施为开发提供运行环境（Agent 沙箱、调度、监控）
自循环开发为基础设施提供进化能力（新 Agent、新 Skill、新策略）
```

两者形成**正向增强循环**：
- 场景一保证系统**稳定运行**（可靠性）
- 场景二保证系统**持续进化**（适应性）
- 两者结合 = **Long-running Agents 的基石**

---

## 与现有产品对比

| 维度 | K8s | Hermes Agent | KIAS |
|------|-----|-------------|------|
| 管理对象 | 容器 | 单 Agent | Agent 集群 |
| 编排方式 | 声明式 YAML | Prompt 驱动 | 声明式 YAML + Prompt |
| 自愈能力 | 容器重启 | 无 | Agent 自愈 + 自进化 |
| 开发能力 | 无 | 代码生成 | 自循环开发 |
| Skill 系统 | 无 | 文件 Skill | 版本化 Skill + 自提取 |
| 适用场景 | 微服务 | 个人助手 | 企业级 Agent 基础设施 |

KIAS = K8s 的编排哲学 + Hermes 的 Skill 系统 + 自循环开发能力
