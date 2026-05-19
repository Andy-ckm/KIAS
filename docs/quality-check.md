# AgentGuard 质量验证报告

> 自动生成于 2026-05-16

## 1. 测试结果 (`cargo test --workspace`)

| 指标 | 数值 |
|------|------|
| 测试总数 | 1,076 |
| 通过 | 1,076 |
| 失败 | 0 |
| 忽略 | 2 (doc-tests) |
| **通过率** | **100%** |

### 各 crate 测试分布

| Crate | 测试数 |
|-------|--------|
| kias-workflow-engine | 122 |
| kias-langgraph-engine | 112 |
| kias-team-engine | 99 |
| kias-mcp-protocol | 82 |
| kias-skills | 71 |
| kias-goal-engine | 62 |
| kias-scheduler | 57 |
| kias-cache | 52 |
| kias-knowledge | 52 |
| kias-common | 49 |
| kias-model-router | 48 |
| kias-data-store | 47 |
| kias-autonomy-controller | 46 |
| kias-controller | 44 |
| kias-logger | 38 |
| kias-orchestrator | 35 |
| kias-monitor | 33 |
| kias-agent-view | 27 |
| 其余 crate | 0 (lib-only / doc-tests) |

## 2. Clippy 静态分析 (`cargo clippy --workspace`)

| 指标 | 数值 |
|------|------|
| **警告总数** | **4** |

### 警告明细

| 类型 | 数量 | 说明 |
|------|------|------|
| unused import | 3 | `ErrorHandlerConfig`, `KiasError`, `ErrorHandler` |
| derivable impl | 1 | `kias-workflow-engine` 中可由 `#[derive]` 替代 |

**结论**：无 error 级别问题，警告均为低风险。

## 3. unwrap() 使用情况（非测试代码）

| 指标 | 数值 |
|------|------|
| **非测试代码中 `.unwrap()` 调用** | **~1,131** |

> 统计范围：`crates/` 下 `.rs` 文件，排除 `/tests/` 目录。
>
> **风险提示**：项目规范（AGENTS.md）要求统一使用 `KiasError` 错误处理，禁止直接 `unwrap()`。
> 当前存在大量 unwrap，生产环境中可能导致 panic。建议逐步替换为 `?` 运算符或 `.expect()` 带上下文信息。

## 4. 总结

| 维度 | 状态 | 说明 |
|------|------|------|
| 单元/集成测试 | ✅ 优秀 | 1,076 测试全部通过，覆盖率广泛 |
| Clippy 合规 | ✅ 良好 | 仅 4 个低风险警告 |
| unwrap() 治理 | ⚠️ 需改进 | ~1,131 处，需逐步清理 |
