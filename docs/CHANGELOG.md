# KIAS 变更日志

> 记录所有重要变更，按时间倒序排列

## [未发布] - 2026-05-15

### Sprint 17: Harness Engineering

#### 新增特性
- **F-003 Key轮转** (`model-router/src/key_rotation.rs`)
  - 参考ollama-open-router源码实现
  - Smart Pick算法：Fisher-Yates shuffle + 失败密钥降权
  - 三种策略：RoundRobin、LRU、Random
  - Cooldown机制：429错误自动冷却和恢复
  - 预算追踪：per-key spend/budget
  - 状态持久化：JSON文件存储
  - 测试：11个新测试用例

- **F-004 虚拟文件系统** (`common/src/vfs.rs`)
  - VirtualFs trait抽象文件操作
  - LocalFs：本地文件系统实现
  - MemoryFs：内存文件系统实现（测试用）
  - 测试：11个新测试用例

- **F-005 工作空间** (`team-engine/src/workspace.rs`)
  - Workspace结构：AGENTS.md、MEMORY.md、skills/、knowledge/
  - 工作空间投影：沙箱中的工作空间视图
  - 测试：10个新测试用例

- **F-006 上下文压缩** (`team-engine/src/compaction.rs`)
  - 上下文压缩策略
  - Token预算管理
  - 事实提取和整合
  - 测试：8个新测试用例

- **F-007 会话持久化** (`team-engine/src/session.rs`)
  - 会话状态JSONL序列化
  - 上下文快照和恢复
  - 会话元数据管理
  - 测试：10个新测试用例

- **F-008 子Agent编排** (`team-engine/src/subagent.rs`)
  - 声明式YAML定义
  - 同步/异步执行模式
  - 任务依赖和状态管理
  - 测试：16个新测试用例

- **F-009 沙箱状态恢复** (`mcp-protocol/src/sandbox.rs`)
  - 工作空间投影到沙箱
  - 三级隔离级别（进程/容器/虚拟机）
  - 状态恢复机制
  - 测试：13个新测试用例

#### 文档更新
- 新增ADR-001：Key轮转架构设计
- 新增特性跟踪矩阵
- 新增变更日志模板
- 更新开发日志

#### 测试结果
- 总测试数：1365 → 1376（+11）
- Clippy警告：0
- 代码覆盖率：待测量

#### 参考资料
- ollama-open-router源码：`reference-projects/ollama-open-router/`
- AgentScope Python：`reference-projects/agentscope-python/`
- rig（Rust）：`reference-projects/rig/`

---

## [1.0.0] - 2026-05-14

### Sprint 16: 模型路由器优化

#### 新增特性
- **模型路由器测试扩展**：18 → 55个测试（+37）
- **DashMap死锁修复**：解决并发访问的死锁问题

#### 测试结果
- 总测试数：1234 → 1309（+75）
- Clippy警告：0

---

## [0.9.0] - 2026-05-13

### Sprint 15: GPU调度器和JWT安全

#### 新增特性
- **GPU调度器**：支持NVIDIA/AMD/Intel + MIG
- **JWT安全增强**：密钥验证和签名检查
- **Controller jitter**：避免惊群效应
- **Sandbox修复**：配置错误修复
- **Workflow修复**：持久化检查点

#### 测试结果
- 总测试数：1198 → 1234（+36）
- Clippy警告：0

---

## [0.8.0] - 2026-05-12

### Sprint 14: MCP协议完善

#### 新增特性
- **认证系统**：JWT/OAuth/API Key/RBAC
- **韧性机制**：熔断、限流、降级
- **Prometheus指标**：性能监控
- **凭证管理**：AES-256-GCM加密
- **热重载**：配置动态更新
- **沙箱后端**：5个后端stub

#### 测试结果
- 总测试数：1165 → 1198（+33）
- Clippy警告：0

---

## 维护指南

### 版本号规则
- 主版本号：重大架构变更或不兼容修改
- 次版本号：新特性添加，向后兼容
- 修订号：bug修复和小改进

### 变更记录要求
1. 每个Sprint必须有独立章节
2. 新特性必须关联特性ID（F-XXX）
3. 必须记录测试变化（新增/修改/删除）
4. 必须记录文档更新
5. 必须记录参考来源

### 审查流程
1. 变更后立即更新此日志
2. Sprint结束时审查完整性
3. 定期清理过期条目
4. 与Git提交历史交叉验证
