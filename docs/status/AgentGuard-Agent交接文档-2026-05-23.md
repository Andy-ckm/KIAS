# AgentGuard 项目交接文档

> 供后续Agent接手继续开发使用
> 最后更新：2026-05-23
> 当前分支：main（唯一分支）

---

## 一、项目基本信息

### 1.1 项目是什么
**AgentGuard**（原KIAS）：让AI Agent可追溯、透明、可控的企业级AI Agent治理与合规系统。

**核心技术栈**：Rust 1.95 / 36 crates / 143K+ LOC / 5241+ 测试

**定位**：面向医疗/制药/器械行业（GxP合规），对标国际大厂（J&J/Pfizer/Roche），做垂直深度而非平台广度。

**GitHub**：`https://github.com/Andy-ckm/KIAS`

### 1.2 核心模块（按重要性排序）

| Crate | 用途 | 成熟度 |
|-------|------|--------|
| `agent-monitor` | 运行时监控、异常检测 | ✅ 成熟 |
| `goal-engine` | 目标驱动引擎 | ✅ 成熟 |
| `auto-loop` | 自修复/自进化循环 | ✅ 成熟 |
| `im-client` | 变更影响分析 | ✅ 成熟 |
| `it-change-management` | IT变更管理 | ⚠️ 待完善 |
| `compliance-gxp` | GxP合规45项检查 | ⚠️ 部分实现 |
| `llm-engine` | LLM调用引擎（支持MiniMax） | ⚠️ 需集成mimo |
| `cache` | 三层缓存（内存/LRU/持久化） | ✅ 成熟 |
| `gxp-compliance` | ALCOA+/21CFR Part11 | ⚠️ 部分实现 |
| `agent-shell` | Shell任务执行 | ⚠️ 概念阶段 |

### 1.3 依赖环境
- **Rust**：1.95.0（必须）
- **MiniMax API**：用于LLM调用（key在 `.env`）
- **TMUX**：guardian.sh 守护进程需要
- **PostgreSQL**（部分模块需要）
- **Docker**（可选，测试用）

---

## 二、快速启动

### 2.1 本地开发环境配置

```bash
# 1. 克隆项目
git clone https://github.com/Andy-ckm/KIAS.git
cd KIAS

# 2. 配置API Key（必须）
cp .env.example .env  # 或手动创建
# 编辑 .env，填入真实的 MINIMAX_API_KEY=sk-cp-...
# 注意：.env 已在 .gitignore，不会被提交

# 3. 构建项目
cargo build --workspace --release

# 4. 运行测试
cargo test --workspace

# 5. 运行特定模块测试
cargo test -p agent-monitor
cargo test -p goal-engine
cargo test -p auto-loop
```

### 2.2 常用开发命令

```bash
# 全量测试（并行，CI质量）
cargo test --workspace -j 8

# 单crate测试并显示输出
cargo test -p <crate-name> -- --nocapture

# 检查代码格式
cargo fmt --check

# 格式化代码
cargo fmt

# 检查编译错误（不触发全量编译）
cargo check --workspace

# 运行clippy
cargo clippy --workspace -- -D warnings

# 运行文档测试
cargo test --doc --workspace
```

---

## 三、当前工作状态

### 3.1 已完成（可参考）

✅ **核心功能**：
- Agent运行时监控（182测试）
- 目标引擎（46测试）
- 自修复循环（403测试）
- 变更影响分析（115测试）
- 三层缓存系统
- GxP合规45项（45%满足）

✅ **流程规范**：
- 四步开发法（评估→审视→方案→开发）
- 铁律：不经调研不写代码、代码即负债、死代码=欺骗
- 统一使用 `main` 分支

✅ **文档**：
- 80+ 份文档在 `docs/`
- 4份status文档在 `docs/status/`
- 方法论在 `docs/METHODOLOGY.md`

### 3.2 未完成（按优先级）

#### P0（核心功能，阻断）
**无**

#### P1（重要，强烈建议优先）

| 任务 | 模块 | 说明 |
|------|------|------|
| MiniMax/mimo集成 | `llm-engine` | 4个API key在 `kias/.task-queue/api_keys.json`，需实现真正的MiniMax调用 |
| IT变更管理web层 | `it-change-management/src/web.rs` | 351行只有1个pub函数，需要完整HTTP handler |
| agentsight接入 | 外部服务 | token跟踪未运行，配置位置不明 |

#### P2（功能完善）
| 任务 | 模块 | 说明 |
|------|------|------|
| 代码生成自动化 | `auto-loop/src/code_gen_automation.rs` | 2个 `todo!()` 桩代码 |
| 自修复循环完善 | `auto-loop` | R019 AIOps智能运维（14/15调研完成）|
| IM集成测试覆盖率 | `im-client` | 2738 LOC / 115测试（2.2%密度）|

#### P3（长期）
- AgentShell调度系统
- MCP协议完整实现
- 21CFR Part11全合规
- 国际化（J&J/Pfizer英文文档）

---

## 四、已知陷阱（接手必读）

### 4.1 铁律
1. **不动网关**：gateway 相关代码不在本项目范围，不碰
2. **四步法**：任何功能开发前必须调研竞品+下载源码+搜论文+找空白，违反=返工
3. **代码即负债**：没有业务价值的代码是毒药，不要写
4. **死代码=欺骗**：写了没接入=没写，必须追踪调用链
5. **灵魂>骨架**：先定义问题再写代码，拒绝骨架代码
6. **不用bash heredoc追加**：Rust模块写独立文件，注册用 `sed`，禁止 `cat >>`

### 4.2 技术陷阱

| 陷阱 | 位置 | 解决方案 |
|------|------|---------|
| E0592/E0432编译错误 | 常见 | 检查 `mod tests` 是否在文件末尾多余一个 `}` |
| `repeat:"once"` 不触发 | cronjob | 用 `"every Nm"` + `"forever"` |
| 编译死代码panic | 意外 | 生产代码永远不 `.unwrap()`，已清零 |
| API key轮换 | MiniMax | 30秒内更新新key |
| TMUX session断开 | guardian.sh | 守护进程需在TMUX内运行 |

### 4.3 API Key 说明
- **真实key**：`~/.env` 和 `~/.env.local`（gitignore隔离，不推送）
- **已推送key**：`kias/.task-queue/api_keys.json` — 全是 `tp-<PLACEHOLDER_NN>` 占位符
- **MiniMax端点**：`api.minimaxi.com`，sk-cp- 开头
- **其他key类型**：tp-s- 开头是其他服务（对MiniMax返回401）

---

## 五、MiniMax/mimo 集成（未完成，P1）

### 5.1 当前状态
`llm-engine` crate 存在骨架代码，但未真正接入 MiniMax API。

### 5.2 目标
实现 `crates/llm-engine/src/providers/minimax.rs`，真正调用 MiniMax M2.7 模型。

### 5.3 参考
- 竞品：参考 `reference-projects/DeepSeek-TUI` 的实现方式
- API格式：`api.minimaxi.com/v1` 端点
- prompt传参：长prompt用文件传JSON（`d @file`）
- 模型：M2.7 有 `reasoning_content` 字段，max_tokens需6000+
- 计费：按请求计费，每请求最大化token

### 5.4 现有参考
```bash
# 查看mimo配置（用户本地有，未同步到workspace）
# 路径：未知，用户会告知
```

---

## 六、测试指南

### 6.1 测试运行
```bash
# 所有测试
cargo test --workspace

# 带覆盖率
cargo tarpaulin --workspace -o Html

# 特定模块
cargo test -p agent-monitor -- --test-threads=4
```

### 6.2 测试文件位置
每个crate的 `src/` 下，文件名 `xxx_test.rs` 或 `#[cfg(test)]` 模块。

### 6.3 端到端测试
```bash
# 在 Docker 内运行完整测试套件
docker compose up test
```

---

## 七、文档索引

| 文档 | 用途 |
|------|------|
| `docs/METHODOLOGY.md` | 方法论（四步法、灵魂>骨架） |
| `docs/status/成果清单-2026-05-23.md` | 当前交付物清单 |
| `docs/status/未完成任务清单-2026-05-23.md` | 待办任务 |
| `docs/status/AgentGuard-项目说明书.md` | 项目完整说明 |
| `docs/architecture.md` | 架构文档 |
| `docs/competitive-analysis-and-surpass-plan.md` | 竞品分析 |
| `docs/research/harness-engineering-analysis.md` | 论文支撑 |
| `docs/design-docs/gxp-compliance-architecture.md` | GxP合规设计 |

---

## 八、沟通规范

- **用户称呼**：零（不要叫"你"）
- **用户说"可以"** = 去做别废话
- **用户发队列ID**（q010等）= 查详情+执行
- **重复请求**：同一指令连续发送=第一次执行后续拒绝
- **用户极度厌恶**：死代码、骨架代码、不调研就写代码、被动等待
- **用户期望**：自主推进，CPU高负载，不要问"要不要做"

---

## 九、重置后的第一步

1. **恢复API key**：将备份的 `.env` / `.env.local` 放回项目根目录
2. **验证构建**：`cargo build --workspace`
3. **验证测试**：`cargo test --workspace -j 8`
4. **确定mimo位置**：询问用户mimo配置脚本的实际路径
5. **选择P1任务开始**：建议从 `it-change-management/src/web.rs` 入手（边界清晰）

---

> 本文档由Agent编写，供后续Agent接手使用。
> 如有疑问，基于 `docs/METHODOLOGY.md` 的方法论推进。
> 核心原则：**做减法不做加法，学底层通用思路，做工程不做玩具。**
