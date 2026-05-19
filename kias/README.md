# AgentGuard Dogfooding — 用 AgentGuard 开发 AgentGuard

## 概述

AgentGuard 系统采用 dogfooding 模式进行开发——使用 AgentGuard 自身来管理和驱动 AgentGuard 的开发工作。

## Agent 定义

| Agent | 角色 | 用途 |
|-------|------|------|
| `code-reviewer` | 代码审查 | 需求分析、代码审查、架构合规检查 |
| `dogfood-dev` | 开发实现 | 功能开发、Bug 修复、测试编写 |
| `doc-writer` | 文档编写 | README、API 文档、技术展示 |

## 工作流

### kias-dogfooding

标准开发循环工作流：

```
plan → implement → test → review → docs → deploy
```

1. **plan**: 需求分析，研究 Codex/CloudDM 文档，制定实施计划
2. **implement**: 按计划实现功能，确保零 clippy 警告
3. **test**: 运行 `cargo test`、`cargo clippy`，确保全部通过
4. **review**: 代码质量、安全性、架构合规审查
5. **docs**: 更新 README、API 文档、technical-showcase.md
6. **deploy**: `git add`、`commit`、`push` 到 main 分支

## 使用方法

```bash
# 注册 Agent
kias-cli agent apply -f kias/agents/kias-agent-code-reviewer.yaml
kias-cli agent apply -f kias/agents/kias-agent-dogfood-dev.yaml
kias-cli agent apply -f kias/agents/kias-agent-doc-writer.yaml

# 创建工作流
kias-cli workflow apply -f kias/workflows/kias-dogfooding-workflow.yaml

# 查看状态
kias-cli agent list
kias-cli workflow list
```

## 设计理念

- **声明式定义**: 所有 Agent 和 Workflow 使用 YAML 声明式定义，版本可控
- **K8S 风格**: 遵循 apiVersion/kind/metadata/spec 结构
- **可观测性**: 每个步骤的输入输出、执行状态都可追踪
- **质量门禁**: 测试通过、clippy 零警告、代码审查通过才能进入下一步
