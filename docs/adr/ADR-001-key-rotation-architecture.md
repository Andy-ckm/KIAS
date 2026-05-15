# ADR-001: Key轮转架构设计

## 状态
已接受 ✅

## 上下文
KIAS需要支持多API密钥的负载均衡和故障转移。现有模型路由器只能处理单个API密钥，在生产环境中存在以下问题：
1. 单个API密钥容易触发速率限制（429错误）
2. 密钥失效会导致服务中断
3. 无法平衡多个密钥的成本和使用量
4. 缺乏智能选择和降权机制

## 决策
实现智能Key轮转模块（key_rotation），参考ollama-open-router的源码设计：

### 核心设计
1. **多策略支持**：RoundRobin（轮询）、LRU（最近最少使用）、Random（智能随机）
2. **Smart Pick算法**：Fisher-Yates shuffle + 失败密钥降权
3. **Cooldown机制**：429错误后自动冷却，到期后自动恢复
4. **预算追踪**：每个密钥的使用量/预算限制
5. **状态持久化**：JSON文件存储轮转状态，支持崩溃恢复
6. **失败追踪**：记录last_failed_key用于下次选择降权

### 架构组件
- `MultiKeyProvider`：管理多个API密钥的提供者
- `KeyRotator`：核心轮转逻辑，支持策略切换
- `KeyInfo`：单个密钥的元数据（状态、冷却、预算）
- `RotationStats`：统计信息（总请求数、冷却数等）

## 后果

### 积极影响
1. **高可用性**：单个密钥失效不影响整体服务
2. **智能负载均衡**：根据密钥健康状况动态调整
3. **成本控制**：预算追踪防止超支
4. **可追溯性**：完整的状态记录和统计信息
5. **可维护性**：模块化设计，易于扩展新策略

### 风险和缓解
1. **复杂性增加**：通过详细的单元测试（11个测试用例）确保可靠性
2. **状态同步**：使用Mutex确保线程安全
3. **配置错误**：提供清晰的API和错误处理

## 参考资料
- ollama-open-router源码：`reference-projects/ollama-open-router/`
- 测试文件：`crates/model-router/tests/key_rotation.rs`
- 实现文件：`crates/model-router/src/key_rotation.rs`

## 相关决策
- ADR-002: 调度器架构设计（GPU调度器）
- ADR-003: 沙箱隔离策略

## 变更历史
- 2026-05-15: 初始版本，基于Sprint 17实现
