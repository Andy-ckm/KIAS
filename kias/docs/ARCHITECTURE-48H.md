# 48 小时开发矩阵架构方案

> 基于 A 卷（RLSD/NPO/CoPD）+ team-engine 现有代码
> 四步法：评估→审视→方案→开发

## 1. 核心映射

### A卷 → team-engine → tmux 矩阵

| A卷原理 | team-engine 组件 | tmux 矩阵角色 | 具体实现 |
|---------|-----------------|---------------|---------|
| RLSD：方向+幅度解耦 | Verifier (方向) + Worker (幅度) | dev-qa (Verifier) + dev-1/2/3 (Worker) | QA 跑 cargo test/clippy → 方向；Worker 写测试/功能 → 幅度 |
| NPO：S=Q/V | Task 状态机 (Pending→Verified) | 调度器渐进分发 | 每批 2-3 文件，完成立即给下一批（保持 Q 高 V 低） |
| CoPD：互为师生 | Swarm MapReduce + Memory | 共享状态 + 持续合并 | .matrix-shared.md + dev-merge 持续蒸馏 |
| 三层记忆 | Memory (STM/LTM/Entity) | .matrix-shared.md + .dev-log + .dev-tasks.yaml | STM=当前批次, LTM=共享状态, Entity=任务追踪 |

### 2. 架构图

```
┌─────────────────────────────────────────────────┐
│                Owner（cron 调度器）               │
│  每 3 分钟：分析状态 → 动态生成任务 → 分发        │
│  复用：TeamEngine.create_task() + assign_task()   │
└──────────┬──────────┬──────────┬────────────────┘
           │          │          │
    ┌──────▼──┐ ┌─────▼───┐ ┌───▼────┐
    │ Worker-1│ │ Worker-2│ │Worker-3│   ← dev-1/2/3
    │ api-    │ │ linux-  │ │论文驱动│   ← CodeWorker.execute()
    │ server  │ │ auto    │ │功能    │
    └────┬────┘ └────┬────┘ └───┬────┘
         │           │          │
    ┌────▼───────────▼──────────▼────┐
    │         共享记忆层              │   ← Memory (三层)
    │  .matrix-shared.md (STM/LTM)   │
    │  .dev-log (Entity)             │
    │  .dev-tasks.yaml (任务追踪)    │
    └────────────┬───────────────────┘
                 │
    ┌────────────▼───────────────────┐
    │      Aggregator（dev-merge）    │   ← Swarm MapReduce
    │  有 commit 就合并，冲突回滚     │
    └────────────┬───────────────────┘
                 │
    ┌────────────▼───────────────────┐
    │      Verifier（dev-qa）         │   ← RuleBasedVerifier
    │  commit 驱动验证                │   ← VerificationRule::ShellCheck
    │  cargo test + clippy + 回滚    │
    └────────────────────────────────┘
```

### 3. 48 小时运行保障

| 瓶颈 | 解法 | team-engine 对应 |
|------|------|-----------------|
| 任务耗尽 | 调度器动态分析项目状态生成任务 | TeamEngine.create_task() 动态调用 |
| tmux 死亡 | 调度器检查活性，死了重启 | AgentStatus::Failed → 恢复 |
| 磁盘爆满 | 每 10 轮 cargo clean，监控 df | 资源管理 |
| 共享状态膨胀 | 滚动保留最近 20 条 | Memory LRU 淘汰 |
| 代码冲突 | dev-merge 持续合并 + 冲突回滚 | Swarm MapReduce |

### 4. 调度器逻辑（Owner）

```
每 3 分钟：
1. 健康检查：tmux 存活？最后输出？
2. 死了 → 重启 + 恢复上下文
3. 空闲 → 分析项目状态：
   a. 哪些文件没有测试？→ 生成测试任务
   b. 哪些论文 insight 未实现？→ 生成功能任务
   c. 有新 commit？→ 触发 QA
4. 渐进分发：每次 2-3 文件
5. 磁盘检查：>80% → cargo clean
6. 共享状态检查：>50 条 → 截断保留最近 20
```

### 5. 验证标准（Verifier）

```bash
# 每个 Worker 完成后必须通过：
cargo test --workspace 2>&1 | tail -5   # 测试全绿
cargo clippy --workspace 2>&1 | grep "warning" | wc -l  # 0 warnings
git diff --stat  # 有实际变更

# 不通过 → 回滚，重新分配
git stash || git checkout -- .
```

### 6. 实施步骤

1. **紧急**：清理磁盘（系统盘 90%）
2. 更新调度器 cron prompt（完整的 Owner 逻辑）
3. 更新 QA cron（Verifier 逻辑，commit 驱动）
4. 更新合并者逻辑（Aggregator，持续蒸馏）
5. 验证第一轮运行
6. 监控 1 小时，确认不中断
