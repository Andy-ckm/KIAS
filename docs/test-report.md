# KIAS 测试报告

> 自动生成于 2026-05-14

## 各 Crate 测试数量

| # | Crate | 单元测试 (lib) | 单元测试 (main) | 集成测试 | 合计 |
|---|-------|---------------|----------------|---------|------|
| 1 | kias-agent-view | 49 | 0 | - | **49** |
| 2 | kias-api-server | 115 | 0 | 57 | **172** |
| 3 | kias-autonomy-controller | 19 | 0 | - | **19** |
| 4 | kias-benchmarks | 0 | - | - | **0** |
| 5 | kias-cache | 35 | 0 | - | **35** |
| 6 | kias-cli | 48 | 0 | - | **48** |
| 7 | kias-common | 82 | - | - | **82** |
| 8 | kias-controller | 91 | 0 | - | **91** |
| 9 | kias-data-store | 41 | 0 | - | **41** |
| 10 | kias-executor | 27 | 0 | - | **27** |
| 11 | kias-goal-engine | 25 | 0 | - | **25** |
| 12 | kias-knowledge | 82 | 0 | - | **82** |
| 13 | kias-langgraph-engine | 6 | - | 33 | **39** |
| 14 | kias (main binary) | - | 47 | - | **47** |
| 15 | kias-mcp-protocol | 62 | - | - | **62** |
| 16 | kias-model-router | 10 | - | - | **10** |
| 17 | kias-monitor | 52 | 0 | - | **52** |
| 18 | kias-scheduler | 87 | 0 | - | **87** |
| 19 | kias-skills | 47 | 0 | - | **47** |
| 20 | kias-team-engine | 97 | 0 | - | **97** |
| 21 | kias-workflow-engine | 74 | 0 | - | **74** |
| | **总计** | | | | **1,166** |

## 测试结果汇总

- **全部通过** ✅ — 所有 1,166 个测试均为 `ok` 状态
- **失败**: 0
- **忽略**: 1 (kias 相关 doc test)
- **总 crate 数**: 21

## 测试覆盖排名 (Top 5)

1. **kias-api-server** — 172 个测试 (含 57 集成测试)
2. **kias-team-engine** — 97 个测试
3. **kias-controller** — 91 个测试
4. **kias-scheduler** — 87 个测试
5. **kias-common** / **kias-knowledge** — 各 82 个测试

## 注意事项

- `kias-benchmarks` 为性能基准 crate，无单元测试（使用 Criterion 框架）
- Doc tests 贡献了少量额外测试（多数为 0 passed）
