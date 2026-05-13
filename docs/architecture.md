# KIAS 架构设计

## 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                        L3: Handlers                         │
│              (API 路由、请求处理、响应序列化)                    │
├─────────────────────────────────────────────────────────────┤
│                        L2: Services                         │
│           (业务逻辑、调度算法、缓存策略、监控分析)                │
├─────────────────────────────────────────────────────────────┤
│                        L1: Models                           │
│              (数据模型、配置结构、事件定义)                      │
├─────────────────────────────────────────────────────────────┤
│                        L0: Common                           │
│              (工具函数、错误类型、日志、配置)                    │
└─────────────────────────────────────────────────────────────┘
```

## 依赖规则

**允许的依赖方向**：
- L3 → L2, L1, L0
- L2 → L1, L0
- L1 → L0

**禁止的依赖**：
- L0 → L1, L2, L3
- L1 → L2, L3
- L2 → L3

**自动检查**：`make lint-arch`

## 核心组件

### API Server
- **职责**：接收请求，认证授权，路由分发
- **技术**：axum (HTTP) + tonic (gRPC)
- **依赖**：scheduler, controller

### Scheduler
- **职责**：资源调度，Agent 分配
- **算法**：
  - Round Robin（轮询）
  - Least Loaded（最少负载）
  - Resource Aware（资源感知）
  - Cache Aware（缓存感知，借鉴 DeepSeek）
- **依赖**：controller, cache-hub

### Controller
- **职责**：Agent 生命周期管理
- **功能**：
  - 创建/删除 Agent
  - 自动扩缩容
  - 故障恢复
- **依赖**：agentsight

### AgentSight
- **职责**：可观测性，Token 追踪
- **借鉴**：ANOLISA 的 AgentSight 组件
- **功能**：
  - Token 逐笔拆账
  - Agent 健康监控
  - eBPF 零侵入探针
  - 可视化 Dashboard
- **依赖**：无（独立组件）

### Cache Hub
- **职责**：KV Cache 优化
- **借鉴**：DeepSeek 的 Prefix Caching
- **功能**：
  - Prefix Caching：相同前缀复用
  - 语义缓存：相似请求命中
  - 分布式缓存：跨节点共享
- **依赖**：无（独立组件）

## 数据流

```
用户请求
    │
    ▼
API Server (认证、路由)
    │
    ▼
Scheduler (资源调度)
    │
    ├──→ Cache Hub (缓存命中检查)
    │
    ▼
Controller (Agent 生命周期)
    │
    ▼
Agent Pod (执行任务)
    │
    ▼
AgentSight (监控、Token 追踪)
```

## Agent Pod 结构

```
Agent Pod
├── AGENTS.md          # Agent 上下文（给 AI 看）
├── agent-config.yaml  # 配置
├── workspace/         # 工作目录
├── logs/              # 日志
└── eBPF Probe         # 监控探针
```

## 资源管理

### 资源类型
- CPU：核心数
- Memory：GiB
- GPU：卡数
- Token：配额

### 资源请求
```yaml
resources:
  requests:
    cpu: "0.5"
    memory: "512Mi"
  limits:
    cpu: "2"
    memory: "2Gi"
```

## 调度策略

### 亲和性
```yaml
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: gpu
              operator: In
              values:
                - "true"
```

### 优先级
```yaml
priorityClassName: high  # high, medium, low
```

## 参考

- [Kubernetes 架构](https://kubernetes.io/docs/concepts/overview/components/)
- [ANOLISA AgentSight](https://github.com/alibaba/anolisa)
- [DeepSeek Cache](https://arxiv.org/abs/2405.04532)