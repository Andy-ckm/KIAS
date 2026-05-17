# Agent Shell 调度设计

## 概述

Agent Shell 调度是一种模板+参数的 Agent 快速创建机制，参考 K8S Pod 调度和 Dify Agent 工作流。

## 核心概念

### Shell（模板）

Shell 是 Agent 的模板，定义了：
- 能力列表（capabilities）
- 约束列表（constraints）
- 参数模板（param_templates）
- 调度策略（scheduling_strategy）

### Params（参数）

Params 是 Shell 的具体值，填充了：
- 参数值（values）
- 元数据（metadata）

### Intent（意图）

Intent 是用户的需求，包含：
- 意图描述（description）
- 意图类型（intent_type）
- 需求列表（requirements）
- 优先级（priority）

### Scheduler（调度器）

Scheduler 根据 Intent 选择 Shell + Params，支持：
- 轮询（RoundRobin）
- 最少负载（LeastLoaded）
- 亲和性（Affinity）
- 缓存感知（CacheAware）
- GPU 感知（GpuAware）
- 优先级（Priority）
- 资源感知（ResourceAware）

## 调度流程

```
用户 Intent
    ↓
Scheduler 过滤候选 Shell
    ↓
Scheduler 根据策略选择 Shell
    ↓
Scheduler 填充 Params
    ↓
返回 ScheduleResult
```

## 参考来源

1. K8S Pod 调度
2. Dify Agent 工作流
3. Coze Studio Bot 配置

---

*基于第一性原则：Agent Shell 的本质是模板+参数，快速创建 Agent。*
