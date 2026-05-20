# dev-1 渐进任务（NPO：每次 2-3 文件，完成后立即写共享状态）

## 当前批次

目标：api-server handler 测试补全，每次 2-3 个 handler

### 本批次文件（按依赖顺序）

1. `src/api/handlers/users.rs` — 用户 CRUD
2. `src/api/handlers/roles.rs` — 角色管理

### 执行步骤

1. **读取共享状态**：`cat /mnt/workspace/kias/.matrix-shared.md`，了解其他人做了什么
2. **读取目标文件**，找到所有 pub async fn
3. **在文件末尾添加 `#[cfg(test)] mod tests { ... }`**
4. **每个 handler 至少 3 个测试**：正常路径、边界条件、错误路径
5. **运行测试**：`cd /mnt/workspace/kias && cargo test -p agentguard-api-server --lib 2>&1 | tail -5`
6. **写入共享状态**（CoPD 互为师生）：

```bash
cat >> /mnt/workspace/kias/.matrix-shared.md << 'EOF'

### [$(date '+%H:%M')] dev-1 | users + roles handler 测试
- 做了什么：users.rs + roles.rs 的 handler 测试
- 学到的：（写你发现的有用信息）
- 产出文件：（写了哪些测试）
EOF
```

7. **完成后等待调度器给下一批**

### 关键铁律
- 用 `$1`/`$2` 测试——**禁止 `.clone()` / `.to_string()` / `.unwrap()`**
- 标准 mock：`TestMocks::standard()` / `TestMocks::with_policy()`
- **完成一批后不要自行找任务，等调度器分配**（NPO：调度器决定 S 最大的任务）
