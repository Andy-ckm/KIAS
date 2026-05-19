# AgentGuard API 验证规范

> 验证不止于编译通过，跑通接口才算完

## 验证闭环

```
改代码 → 构建 → 启动 → curl 验证 → 解析响应 → 确认正确
```

## curl 验证规范

### 原则
1. **每个 curl 独立执行** - 禁止串联多个 curl
2. **用临时文件传递数据** - curl 输出写入 /tmp/
3. **Token 获取模板化** - 登录 → 写文件 → 提取 token
4. **排查路径明确** - 日志文件位置、数据库连接方式

### 为什么这么严格？

AI Agent 在 shell 中执行命令时，经常遇到兼容性问题：
- zsh 下管道 + 方括号的 glob 问题
- `curl | python3 -c "print(data['key'])"` 会报错

用临时文件中转虽然多了一步，但稳定性高得多。

## 验证模板

### 健康检查
```bash
# Step 1: 健康检查
curl -s http://localhost:8080/health > /tmp/health.json

# Step 2: 解析响应
python3 -c "import json; print(json.load(open('/tmp/health.json')))"

# Step 3: 验证状态
python3 -c "
import json
data = json.load(open('/tmp/health.json'))
assert data['status'] == 'healthy', f'Expected healthy, got {data[\"status\"]}'
print('✓ Health check passed')
"
```

### 创建 Agent
```bash
# Step 1: 创建 Agent
curl -s -X POST http://localhost:8080/api/v1/agents \
  -H 'Content-Type: application/json' \
  -d '{"name":"test-agent","image":"python:3.11"}' > /tmp/agent.json

# Step 2: 解析响应
python3 -c "
import json
data = json.load(open('/tmp/agent.json'))
print(f'Agent ID: {data[\"id\"]}')
print(f'Status: {data[\"status\"]}')
"

# Step 3: 验证创建成功
python3 -c "
import json
data = json.load(open('/tmp/agent.json'))
assert data['name'] == 'test-agent', f'Expected test-agent, got {data[\"name\"]}'
assert data['status'] in ['Pending', 'Running'], f'Unexpected status: {data[\"status\"]}'
print('✓ Agent created successfully')
"
```

### 查询 Agent 列表
```bash
# Step 1: 查询列表
curl -s http://localhost:8080/api/v1/agents > /tmp/agents.json

# Step 2: 解析响应
python3 -c "
import json
data = json.load(open('/tmp/agents.json'))
print(f'Total agents: {len(data[\"agents\"])}')
for agent in data['agents']:
    print(f'  - {agent[\"name\"]}: {agent[\"status\"]}')
"
```

### Token 消耗查询
```bash
# Step 1: 查询 Token 消耗
curl -s http://localhost:8080/api/v1/agents/test-agent/tokens > /tmp/tokens.json

# Step 2: 解析响应
python3 -c "
import json
data = json.load(open('/tmp/tokens.json'))
print(f'Input tokens: {data[\"input_tokens\"]}')
print(f'Output tokens: {data[\"output_tokens\"]}')
print(f'Total cost: \${data[\"cost\"]:.4f}')
"
```

## 验证脚本

### scripts/verify.sh
```bash
#!/bin/bash
set -e

BASE_URL="http://localhost:8080"
PASS=0
FAIL=0

check() {
    local name=$1
    local result=$2
    
    if [ "$result" = "0" ]; then
        echo "✓ $name"
        ((PASS++))
    else
        echo "✗ $name"
        ((FAIL++))
    fi
}

# 健康检查
curl -s $BASE_URL/health > /tmp/health.json
python3 -c "import json; assert json.load(open('/tmp/health.json'))['status'] == 'healthy'"
check "Health check" $?

# 创建 Agent
curl -s -X POST $BASE_URL/api/v1/agents \
  -H 'Content-Type: application/json' \
  -d '{"name":"verify-test","image":"python:3.11"}' > /tmp/agent.json
python3 -c "import json; assert json.load(open('/tmp/agent.json'))['name'] == 'verify-test'"
check "Create agent" $?

# 查询 Agent
curl -s $BASE_URL/api/v1/agents > /tmp/agents.json
python3 -c "import json; assert len(json.load(open('/tmp/agents.json'))['agents']) > 0"
check "List agents" $?

# 清理
curl -s -X DELETE $BASE_URL/api/v1/agents/verify-test > /dev/null
check "Delete agent" $?

# 汇总
echo ""
echo "Results: $PASS passed, $FAIL failed"
[ "$FAIL" = "0" ] && exit 0 || exit 1
```

## 前端验证

### Agent Browser 验证
对于前端页面，使用 Agent Browser 能力：

```bash
# 启动浏览器验证
kias agent browser --url http://localhost:3000 --screenshot /tmp/screenshot.png
```

### 视觉验证
```python
# 使用 vision 工具验证截图
from hermes_tools import vision_analyze

result = vision_analyze(
    image_url="/tmp/screenshot.png",
    question="页面是否正常渲染？是否有错误信息？"
)
print(result)
```

## CI/CD 集成

### GitHub Actions
```yaml
- name: Verify API
  run: |
    ./scripts/start-control-plane.sh
    sleep 5
    ./scripts/verify.sh
```

## 故障排查

### 日志位置
- 控制平面：`/var/log/kias/control-plane.log`
- 节点代理：`/var/log/kias/node-agent.log`
- Agent：`kias agent logs <agent-name>`

### 常见问题
1. **连接拒绝** - 检查服务是否启动
2. **认证失败** - 检查 Token 是否有效
3. **资源不足** - 检查节点资源使用情况

## 参考

- [curl 文档](https://curl.se/docs/)
- [Python json 模块](https://docs.python.org/3/library/json.html)