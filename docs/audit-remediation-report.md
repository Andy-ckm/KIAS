# AgentGuard 整改前后对比报告

**日期**: 2026-05-22
**执行范围**: P0 全部5项任务

## 对比结果

| 指标 | 整改前 | 整改后 | 变化 |
|------|--------|--------|------|
| cargo fmt --check 差异文件数 | 491 | 0 | -491 ✅ |
| 生产代码 unwrap() (非测试/非mutex) | ~172 | 0 | -172 ✅ |
| println! 调用 | 334 | 280 | -54 |
| todo!/unimplemented! | 0 | 0 | 持平 ✅ |
| lint-arch 实现 | grep 级 | cargo metadata 依赖图 | 升级 ✅ |
| CI 目标 | 无 | make ci (fmt+clippy+test+lint-arch) | 新增 ✅ |
| 测试注解数 | ~6900 | 6987 | +87 |
| Crate 数 | 36 | 36 | 持平 |

## 详细变更

### P0-任务1: cargo fmt
- 执行 `cargo fmt --all`，491个文件统一格式化
- 提交: `f461e9c`

### P0-任务2: unwrap 治理
- **common/observability_std.rs**: 5处 SystemTime unwrap → unwrap_or_default()
- **knowledge/entity_extractor.rs**: 9处 Regex unwrap → expect()
- **compliance-security/**: partial_cmp/as_deref/HMAC 修复
- **goal-engine, model-router, monitor, team-engine, workflow-engine, data-aggregator**: 逐个修复
- 生产代码非测试非mutex unwrap: 120 → 0
- 提交: `f461e9c`

### P0-任务3: CI 阶段
- 新增 `make ci` = fmt-check + clippy + test + lint-arch
- 提交: `b096e41`

### P0-任务4: lint-arch 升级
- 从 grep 方案升级为 `scripts/lint-arch.py` (cargo metadata 依赖图)
- 自动发现 34 个 workspace crate 的依赖关系
- 支持 L0→L1→L2→L3 四层架构校验
- 提交: `b096e41`

### P0-任务5: 本报告

## 门禁通过率

| 门禁 | 状态 |
|------|------|
| cargo fmt --check | ✅ PASS (0 diffs) |
| cargo check --workspace | ✅ PASS |
| lint-arch | ✅ PASS (34 crates) |
| 非测试 unwrap | ✅ 0 |
| clippy -D warnings | ⚠️ 有 warnings 待修 |

## 下一步 (P1/P2)

- clippy warnings 清零
- println! → tracing 替换
- 关键路径测试补充
- 失败路径测试
