# KIAS 全面审计报告
> 日期: 2026-05-16 | 提交: 5d356b5 | 测试: 2139 全绿

## 一、项目规模

| 指标 | 数值 |
|------|------|
| Crate 数量 | 27 |
| .rs 文件 | 272 |
| 总 LOC | 107,643 |
| 函数数 | 5,466 |
| 测试数 | 2,139 |
| 测试/函数比 | 39% |
| TODO/FIXME | 13 (12 in skills/builtin.rs + 1 in data-store) |
| todo!()/unimplemented!() | 0 |

## 二、关键架构缺陷修复状态

| # | 缺陷 | 状态 | 关键文件:行 |
|---|------|------|------------|
| 1 | HNSW O(N) 暴力搜索 | ✅ 已修复 | `common/src/vector.rs:226-446` (多层 beam search, ef=100) |
| 2 | Redis 虚假声明 | ✅ 已清理 | `common/src/config.rs:136`, `cache_persist/mod.rs` (SQLite/内存) |
| 3 | Scheduler 无多租户 | ✅ 已实现 | `scheduler/src/scheduler.rs:14-359` (TenantContext/Namespace/Quota) |
| 4 | team-engine 重试未执行 | ✅ 已修复 | `team-engine/src/engine.rs:305-340` (execute_with_retry 循环) |
| 5 | autonomy-controller 未集成 | ✅ 已接入 | `controller/src/reconciler.rs:81-103` (AutonomyGate spawn 前检查) |
| 6 | data-store→knowledge 跨层 | ✅ 已修复 | `common/src/vector.rs:4-5` (vector types 移到 L0 common) |

## 三、各 Crate 状态

| Crate | LOC | 函数 | 测试 | 状态 |
|-------|-----|------|------|------|
| controller | 4,266 | 229 | 113 | ✅ Real + AutonomyGate |
| scheduler | 6,745 | 308 | 114 | ✅ Real (6种调度算法) |
| team-engine | 8,429 | 434 | 179 | ✅ Real (含重试循环) |
| autonomy-controller | 1,042 | 82 | 46 | ✅ Real (已接入reconciler) |
| data-store | 5,246 | 244 | 91 | ✅ Real |
| knowledge | 8,339 | 453 | 179 | ✅ Real (RAG+GraphRAG) |
| workflow-engine | 6,562 | 306 | 131 | ✅ Real (DAG+Checkpoint) |
| langgraph-engine | 2,872 | 114 | 77 | ✅ Real |
| model-router | 3,669 | 188 | 71 | ✅ Real |
| skills | 3,364 | 284 | 64 | ⚠️ Mixed (12个外部集成TODO) |
| api-server | 10,349 | 396 | 248 | ✅ Real |
| auto-loop | 8,817 | 364 | 181 | ✅ Real |
| mcp-protocol | 11,430 | 539 | 201 | ✅ Real |
| 其他 14 crate | ~25K | ~1500 | ~500 | ✅ Real |

## 四、剩余 TODO

1. **skills/builtin.rs**: 12 个外部系统集成占位符（LLM调用、ERP、WMS等返回硬编码数据）
2. **HNSW**: 缺 SIMD 距离计算和磁盘持久化图（TODO#real-hnsw）
3. **benchmarks crate**: 被注释出 workspace members

## 五、质量门禁

- ✅ 0 clippy warnings
- ✅ 0 test failures
- ✅ 0 todo!()/unimplemented!() macros
- ✅ 所有 panic! 仅在 #[test] 块中
- ✅ 生产代码使用 thiserror/anyhow 错误处理
