# WebSocket 实时推送 — 生产级增强设计文档

> Sprint 11 | 2026-05-14

## 1. 概述

KIAS WebSocket 系统从基础的 EventBus + 订阅过滤升级为生产级实时推送系统，
新增连接注册表、事件回放缓冲、心跳保活、统计端点四大核心能力。

## 2. 架构

```text
┌──────────────────────────────────────────────────────────────┐
│                      API Server                               │
│                                                                │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐  │
│  │ Agent    │   │ Workflow │   │ Scheduler│   │ A2A      │  │
│  │ Handlers │   │ Handlers │   │ Handler  │   │ Handler  │  │
│  └────┬─────┘   └────┬─────┘   └────┬─────┘   └────┬─────┘  │
│       │              │              │              │          │
│       └──────────────┴──────────────┴──────────────┘          │
│                          │                                     │
│                    EventBus.publish()                           │
│                          │                                     │
│              ┌───────────┴───────────┐                         │
│              ▼                       ▼                         │
│    EventReplayBuffer       broadcast::Sender                  │
│    (ring buffer N=100)           │                             │
│              │              ┌────┴────┐                        │
│              │              ▼         ▼                        │
│              │          WS Client  WS Client                   │
│              │                                                │
│              └──── New clients get replay on connect          │
│                                                                │
│  ┌──────────────────┐    ┌──────────────────────────┐         │
│  │ ConnectionRegistry│    │ Heartbeat (30s ping)     │         │
│  │ - active conns    │    │ - stale detection (90s)  │         │
│  │ - total accepted  │    │ - auto-close stale       │         │
│  │ - messages sent   │    └──────────────────────────┘         │
│  │ - lagged events   │                                         │
│  └──────────────────┘                                         │
│                                                                │
│  GET /api/v1/ws/stats → WsStats JSON                          │
└──────────────────────────────────────────────────────────────┘
```

## 3. 新增组件

### 3.1 ConnectionRegistry

线程安全的连接注册表，使用 `Arc<ConnectionRegistryInner>` 包装：

- `register(addr) -> ConnectionId`: 注册新连接，返回唯一 ID
- `unregister(id)`: 注销连接
- `set_subscriptions(id, subs)`: 更新订阅过滤器
- `inc_messages_sent(id)`: 全局消息计数
- `inc_lagged()`: 滞后事件计数
- `stats() -> WsStats`: 快照当前统计

### 3.2 EventReplayBuffer

固定容量环形缓冲区（默认 100 事件）：

- `push(event)`: 添加事件，满时淘汰最旧的
- `snapshot() -> Vec<WsEvent>`: 克隆所有缓冲事件
- 新 WebSocket 客户端连接时自动回放缓冲事件

### 3.3 心跳保活

- 服务端每 30 秒发送 Ping 帧
- 客户端 90 秒内未响应 Pong 则断开
- 使用 `tokio::select!` 实现非阻塞心跳检测

### 3.4 WsStats 端点

`GET /api/v1/ws/stats` 返回 JSON：

```json
{
  "active_connections": 3,
  "total_connections": 10,
  "total_messages_sent": 42,
  "total_lagged": 1,
  "replay_buffer_size": 50,
  "replay_buffer_capacity": 100,
  "connections": [
    {
      "id": 1,
      "remote_addr": "127.0.0.1:8080",
      "connected_at": "2026-05-14T14:00:00Z",
      "subscriptions": ["agent_created", "task_completed"],
      "events_sent": 10
    }
  ]
}
```

## 4. Handler EventBus 集成

### 4.1 Agent Handlers

| 操作 | 发布事件 |
|------|---------|
| `POST /api/v1/agents` | `agent_created` |
| `DELETE /api/v1/agents/:id` | `agent_deleted` |
| `PATCH /api/v1/agents/:id/status` | `agent_status_changed` |

### 4.2 Workflow Handlers

| 操作 | 发布事件 |
|------|---------|
| `POST /api/v1/workflows` | `workflow_update` (action=created) |
| `DELETE /api/v1/workflows/:id` | `workflow_update` (action=deleted) |

每个事件同时写入 EventBus（实时推送）和 EventReplayBuffer（新客户端回放）。

## 5. Wire Protocol

### 5.1 服务端 → 客户端

```json
// 连接确认
{"type": "connected", "connection_id": 1, "message": "...", "timestamp": "..."}

// 业务事件
{"type": "agent_created", "data": {"agent_id": "...", "name": "..."}, "timestamp": "..."}

// 滞后告警
{"type": "system_alert", "data": {"alert_type": "lagged", "message": "Missed 5 events..."}, "timestamp": "..."}
```

### 5.2 客户端 → 服务端

```json
// 订阅过滤
{"subscribe": ["agent_created", "task_completed"]}

// 清除过滤（接收所有事件）
{"subscribe": []}
```

## 6. 测试覆盖

| 测试类别 | 测试数 | 覆盖范围 |
|---------|--------|---------|
| EventBus | 8 | 发布/订阅/多订阅者/序列化/便捷方法 |
| ConnectionRegistry | 4 | 注册/注销/订阅/指标 |
| EventReplayBuffer | 5 | 推入/快照/淘汰/容量/集成 |
| WsStats | 2 | 序列化/ConnectionInfo |
| 心跳 | 2 | 间隔合理性/超时 > 间隔 |
| 集成 | 1 | 发布 → 回放缓冲 |

## 7. 文件变更

| 文件 | 变更 |
|------|------|
| `crates/api-server/src/websocket.rs` | 新增 ConnectionRegistry, EventReplayBuffer, Heartbeat, ws_stats_handler |
| `crates/api-server/src/lib.rs` | AppState 新增 connection_registry, event_replay_buffer 字段 |
| `crates/api-server/src/handlers/agents.rs` | create/delete/status_update 发布 WS 事件 |
| `crates/api-server/src/handlers/workflows.rs` | create/delete 发布 WS 事件 |
| `crates/api-server/src/routes/api.rs` | 新增 /api/v1/ws/stats 路由 |
| `crates/api-server/src/handlers/scheduler.rs` | 测试代码适配新字段 |
| `crates/api-server/src/handlers/tokens.rs` | 测试代码适配新字段 |
| `docs/sprint-plan.md` | 更新进度 |
| `docs/design-docs/websocket-realtime-push.md` | 本文档 |
