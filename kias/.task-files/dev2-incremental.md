# dev-2 渐进任务（NPO：每次 2-3 文件）

## 当前批次

linux-automation 测试补全

### 本批次文件

1. `src/linux_automation/package_manager.rs` — 包管理器
2. `src/linux_automation/network_diagnostics.rs` — 网络诊断

### 执行步骤

1. 读共享状态：`cat /mnt/workspace/kias/.matrix-shared.md`
2. 读目标文件，找 pub fn / pub async fn
3. 文件末尾加 `#[cfg(test)] mod tests { ... }`
4. 每个函数至少 3 个测试：正常、边界、错误
5. 运行：`cd /mnt/workspace/kias && cargo test -p agentguard-linux-automation --lib 2>&1 | tail -5`
6. 写入共享状态：
```bash
echo -e "\n### [$(date '+%H:%M')] dev-2 | package_manager + network_diagnostics\n- 做了什么：...\n- 学到的：...\n- 产出文件：..." >> /mnt/workspace/kias/.matrix-shared.md
```
7. 等调度器分配下一批
