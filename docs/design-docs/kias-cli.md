# AgentGuard CLI 设计文档

> 参考：阿里云 AgentRun CLI + Anthropic Managed Agents

## 设计原则

**Agent 是生产资源，不是聊天助手。**
- 能进 Git，能审查，能部署，能回滚，能审计，能限权，能算成本

## 命令结构

```bash
kias <resource> <action> [flags]

# 资源类型
kias agent      # Agent 管理
kias workflow    # 工作流管理
kias tool        # 工具管理
kias skill       # 技能管理
kias sandbox     # 沙箱管理
kias model       # 模型管理
kias config      # 配置管理

# 全局 flags
--output json|table|yaml|quiet
--dry-run
--namespace <ns>
--context <ctx>
```

## 核心命令

### Agent 管理
```bash
# 声明式管理
kias agent apply -f agent.yaml
kias agent delete <name>
kias agent get <name>
kias agent list

# 运行
kias agent run --name <name> --prompt "..."
kias agent run -f agent.yaml --prompt "..."

# 非交互调用（CI 友好）
kias agent invoke --name <name> --text "分析这个错误" --text-only
kias agent invoke --name <name> --text "..." --output json
```

### 工作流管理
```bash
kias workflow apply -f workflow.yaml
kias workflow run <name> --input '{"key": "value"}'
kias workflow status <run-id>
kias workflow logs <run-id>
```

### 沙箱管理
```bash
kias sandbox create --template python-data
kias sandbox exec <id> -- python script.py
kias sandbox destroy <id>
```

## YAML 定义格式

### Agent 定义
```yaml
apiVersion: kias/v1
kind: Agent
metadata:
  name: customer-support
  namespace: production
  labels:
    team: support
    tier: critical
spec:
  prompt: |
    你是客服助手，负责处理用户工单。
    分析工单内容，给出分类和建议。
  model:
    name: qwen-max
    service: svc-qwen-prod
  tools:
    - mcp-ticket-system
    - mcp-knowledge-base
  skills:
    - skill-ticket-policy
    - skill-escalation
  sandboxes:
    - sb-python-analysis
  resources:
    memory: 512Mi
    cpu: 0.5
  permissions:
    read:
      - tickets/*
      - knowledge/*
    write:
      - tickets/status
    deny:
      - tickets/delete
  cost:
    maxTokensPerRun: 10000
    maxCostPerDay: 100.00
  audit:
    logLevel: detailed
    retention: 90d
```

### Workflow 定义
```yaml
apiVersion: kias/v1
kind: Workflow
metadata:
  name: ticket-triage
spec:
  entry: classify
  nodes:
    - name: classify
      agent: customer-support
      prompt: "分类这个工单"
    - name: route
      condition: "state.priority == 'high'"
      agent: senior-support
    - name: auto-resolve
      condition: "state.priority == 'low'"
      agent: auto-responder
  edges:
    - from: classify
      to: route
      condition: "state.priority == 'high'"
    - from: classify
      to: auto-resolve
      condition: "state.priority == 'low'"
```

## 退出码

| 退出码 | 含义 |
|--------|------|
| 0 | 成功 |
| 1 | 参数错误 |
| 2 | 认证失败 |
| 3 | 资源不存在 |
| 4 | 权限不足 |
| 5 | 服务端错误 |
| 6 | 超时 |
| 7 | 成本超限 |

## 输出格式

### JSON（默认）
```json
{
  "status": "success",
  "data": {
    "agent_id": "agent-123",
    "output": "工单已分类为：技术支持"
  },
  "metadata": {
    "tokens_used": 150,
    "cost": 0.003,
    "duration_ms": 1200
  }
}
```

### Table
```
AGENT_ID     STATUS    TOKENS   COST     DURATION
agent-123    success   150      $0.003   1.2s
```

### Quiet
```
agent-123
```

## 实现计划

### Phase 1: 基础 CLI
- [ ] CLI 框架（clap）
- [ ] Agent CRUD 命令
- [ ] YAML 解析
- [ ] JSON 输出

### Phase 2: 运行时
- [ ] Agent 执行引擎
- [ ] 非交互调用
- [ ] 退出码语义化

### Phase 3: 企业特性
- [ ] 沙箱隔离
- [ ] 凭证管理
- [ ] RBAC 权限
- [ ] 审计日志
- [ ] 成本归因

### Phase 4: CI/CD 集成
- [ ] GitHub Actions 集成
- [ ] GitLab CI 集成
- [ ] Jenkins 插件
