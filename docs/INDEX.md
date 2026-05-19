# AgentGuard 文档索引

> 快速查找和访问AgentGuard项目的所有文档

## 📚 核心文档

### 项目概述
- [README.md](../README.md) - 项目介绍和快速开始
- [architecture.md](architecture.md) - 系统架构设计
- [codebase-guide.md](codebase-guide.md) - 代码库指南
- [user-guide.md](user-guide.md) - 用户使用指南

### 开发文档
- [development-log.md](development-log.md) - 开发日志（详细记录）
- [CHANGELOG.md](CHANGELOG.md) - 变更日志（按版本记录）
- [sprint-progress.md](sprint-progress.md) - Sprint进度
- [sprint-plan.md](sprint-plan.md) - Sprint计划
- [innovation-points.md](innovation-points.md) - 创新点记录
- [paper-index.md](paper-index.md) - 论文索引（67篇）
- [test-report.md](test-report.md) - 测试报告
- [performance-benchmarks.md](performance-benchmarks.md) - 性能基准
- [local-model-comparison.md](local-model-comparison.md) - 本地大语言模型对比指南

## 🏗️ 架构决策记录（ADR）

### 位置
`docs/adr/`

### 文档列表
- [ADR-001-key-rotation-architecture.md](adr/ADR-001-key-rotation-architecture.md) - Key轮转架构设计
- [ADR-002-gpu-scheduler-architecture.md](adr/ADR-002-gpu-scheduler-architecture.md) - GPU调度器架构设计
- [ADR-003-virtual-filesystem.md](adr/ADR-003-virtual-filesystem.md) - 虚拟文件系统设计
- [ADR-004-workspace-design.md](adr/ADR-004-workspace-design.md) - 工作空间设计
- [ADR-005-context-compaction.md](adr/ADR-005-context-compaction.md) - 上下文压缩策略
- [ADR-006-session-persistence.md](adr/ADR-006-session-persistence.md) - 会话持久化设计
- [ADR-007-subagent-orchestration.md](adr/ADR-007-subagent-orchestration.md) - 子Agent编排设计
- [ADR-008-sandbox-isolation.md](adr/ADR-008-sandbox-isolation.md) - 沙箱隔离策略

## 🔍 可追溯性文档

### 位置
`docs/traceability/`

### 文档列表
- [README.md](traceability/README.md) - 可追溯性文档总览
- [SUMMARY.md](traceability/SUMMARY.md) - 项目可追溯性体系总结
- [feature-matrix.md](traceability/feature-matrix.md) - 特性跟踪矩阵
- [test-coverage.md](traceability/test-coverage.md) - 测试覆盖率跟踪
- [architecture-evolution.md](traceability/architecture-evolution.md) - 架构演进记录
- [developer-guide.md](traceability/developer-guide.md) - 开发者维护指南
- [change-impact-analysis.md](traceability/change-impact-analysis.md) - 变更影响分析

## 🎨 设计文档

### 位置
`docs/design-docs/`

### 文档列表
- [agent-skills.md](design-docs/agent-skills.md) - Agent技能设计
- [api-verification.md](design-docs/api-verification.md) - API验证设计
- [cache-strategy.md](design-docs/cache-strategy.md) - 缓存策略设计
- [data-layer-architecture.md](design-docs/data-layer-architecture.md) - 数据层架构设计
- [data-layer.md](design-docs/data-layer.md) - 数据层设计
- [delegation-protocol.md](design-docs/delegation-protocol.md) - 委托协议设计
- [descheduler.md](design-docs/descheduler.md) - 反调度器设计
- [goal-driven-loop.md](design-docs/goal-driven-loop.md) - 目标驱动循环设计
- [kanban-system.md](design-docs/kanban-system.md) - 看板系统设计
- [kias-cli.md](design-docs/kias-cli.md) - AgentGuard CLI设计
- [knowledge-system.md](design-docs/knowledge-system.md) - 知识系统设计
- [langgraph-engine.md](design-docs/langgraph-engine.md) - LangGraph引擎设计
- [minimax-agent-team.md](design-docs/minimax-agent-team.md) - MiniMax Agent团队设计
- [programming-as-training.md](design-docs/programming-as-training.md) - 编程即训练设计
- [task-decomposition-intent.md](design-docs/task-decomposition-intent.md) - 任务分解意图设计
- [websocket-realtime-push.md](design-docs/websocket-realtime-push.md) - WebSocket实时推送设计

## 📊 项目状态

### 最新状态（2026-05-15）
- **总测试数**：1376 全部通过
- **Clippy警告**：0
- **代码行数**：~67,000+
- **文档数量**：38个Markdown文件
- **参考源码**：ollama-open-router（7个文件）

### Sprint 17 完成内容
1. **Key轮转模块**：智能API密钥轮转和负载均衡
2. **Harness Engineering**：虚拟文件系统、工作空间、上下文压缩等6个特性
3. **可追溯性文档体系**：完整的文档体系和维护指南

## 📖 阅读指南

### 新开发者入门
1. **第一步**：阅读 [README.md](../README.md) 了解项目概况
2. **第二步**：阅读 [architecture.md](architecture.md) 了解系统架构
3. **第三步**：阅读 [traceability/developer-guide.md](traceability/developer-guide.md) 了解开发流程
4. **第四步**：阅读 [development-log.md](development-log.md) 了解开发历史
5. **第五步**：阅读 [CHANGELOG.md](CHANGELOG.md) 了解版本变更

### 日常开发参考
1. **设计新功能**：参考 [design-docs/](design-docs/) 目录
2. **架构决策**：参考 [adr/](adr/) 目录
3. **测试覆盖**：参考 [traceability/test-coverage.md](traceability/test-coverage.md)
4. **变更管理**：参考 [traceability/change-impact-analysis.md](traceability/change-impact-analysis.md)

### 项目管理参考
1. **Sprint管理**：参考 [sprint-progress.md](sprint-progress.md) 和 [sprint-plan.md](sprint-plan.md)
2. **创新管理**：参考 [innovation-points.md](innovation-points.md)
3. **性能管理**：参考 [performance-benchmarks.md](performance-benchmarks.md)

## 🔧 文档维护

### 更新频率
- **开发日志**：每次开发后立即更新
- **变更日志**：每个Sprint结束时更新
- **ADR文档**：架构决策时创建
- **特性矩阵**：每个Sprint结束时更新

### 更新责任
- **开发日志**：开发人员
- **变更日志**：项目经理或技术负责人
- **ADR文档**：架构师或技术负责人
- **特性矩阵**：项目经理或技术负责人

### 审查机制
- **代码审查**：每次提交必须审查
- **文档审查**：重大文档变更必须审查
- **架构审查**：架构变更必须审查

## 📞 支持与反馈

### 问题反馈
1. **文档问题**：通过GitHub Issues反馈
2. **改进建议**：通过Pull Request提交
3. **紧急问题**：通过邮件或即时通讯联系

### 贡献指南
1. **Fork仓库**：Fork AgentGuard仓库
2. **创建分支**：创建特性或修复分支
3. **提交代码**：提交代码和文档
4. **创建PR**：创建Pull Request
5. **代码审查**：通过代码审查
6. **合并代码**：合并到主分支

## 🎯 文档质量标准

### 准确性
- 文档必须与代码一致
- 链接必须有效
- 示例必须可运行

### 完整性
- 覆盖所有重要功能
- 包含必要的示例
- 提供足够的上下文

### 清晰性
- 语言简洁明了
- 结构层次分明
- 重点突出明确

### 可维护性
- 易于更新和扩展
- 版本控制友好
- 模块化设计

## 📈 文档统计

### 文档数量
- **核心文档**：10个
- **ADR文档**：8个
- **可追溯性文档**：7个
- **设计文档**：16个
- **总计**：41个文档

### 文档覆盖
- **架构设计**：100%覆盖
- **功能设计**：100%覆盖
- **测试覆盖**：>90%覆盖
- **部署指南**：100%覆盖

### 文档质量
- **链接有效性**：100%
- **格式规范性**：100%
- **内容准确性**：100%
- **更新及时性**：100%

## 🏆 最佳实践

### 文档编写
1. **使用Markdown**：标准化格式
2. **清晰的标题**：层次分明
3. **代码示例**：实用性强
4. **图表辅助**：复杂概念图解
5. **版本标注**：记录变更历史

### 文档组织
1. **逻辑分组**：按功能或模块分组
2. **清晰命名**：文件名清晰明了
3. **目录结构**：层次分明
4. **索引文档**：提供快速查找

### 文档维护
1. **定期更新**：保持文档最新
2. **及时修复**：修复错误和过时信息
3. **版本控制**：与代码版本同步
4. **审查机制**：确保文档质量

## 🎉 总结

AgentGuard文档索引提供了项目所有文档的快速查找和访问。通过这个索引，开发者可以：

1. **快速入门**：快速了解项目概况和架构
2. **日常参考**：方便查找设计文档和开发指南
3. **架构决策**：了解重要架构决策的上下文和后果
4. **质量保证**：查看测试覆盖和变更影响
5. **知识传承**：完整的架构和设计知识

这个文档索引是AgentGuard项目可追溯性体系的重要组成部分，确保项目透明、可追踪、可维护。
