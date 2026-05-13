# KIAS 用户使用说明

> Kubernetes-like Intelligent Agent Scheduling System
> 专业 AI Agent 集群调度系统

## 1. 快速开始

### 1.1 安装

#### Docker 安装（推荐）
```bash
# 拉取镜像
docker pull kias/control-plane:latest
docker pull kias/node-agent:latest

# 启动控制平面
docker run -d \
  --name kias-control-plane \
  -p 8080:8080 \
  -p 9090:9090 \
  -v /var/lib/kias:/data \
  kias/control-plane:latest

# 启动节点代理
docker run -d \
  --name kias-node-1 \
  -e KIAS_NODE_ID=node-1 \
  -e KIAS_CONTROL_PLANE=http://control-plane:8080 \
  kias/node-agent:latest
```

#### 二进制安装
```bash
# 下载最新版本
curl -LO https://github.com/your-org/kias/releases/latest/download/kias-linux-amd64.tar.gz

# 解压
tar xzf kias-linux-amd64.tar.gz

# 安装到系统路径
sudo mv kias /usr/local/bin/

# 验证安装
kias version
```

#### 源码编译
```bash
# 克隆仓库
git clone https://github.com/your-org/kias.git
cd kias

# 编译
make build

# 安装
sudo make install
```

### 1.2 首次配置

```bash
# 生成默认配置
kias init

# 编辑配置
vim ~/.kias/config.toml
```

**配置示例**：
```toml
[api_server]
host = "0.0.0.0"
port = 8080

[scheduler]
algorithm = "cache_aware"

[cache_hub]
redis_url = "redis://localhost:6379"
```

### 1.3 启动服务

```bash
# 启动控制平面
kias control-plane start

# 启动节点代理（在其他节点执行）
kias node-agent start --node-id node-1
```

---

## 2. 核心功能

### 2.1 Agent 管理

#### 创建 Agent
```bash
# 基本创建
kias agent create \
  --name my-agent \
  --image python:3.11 \
  --command "python app.py"

# 指定资源
kias agent create \
  --name my-agent \
  --image python:3.11 \
  --cpu 0.5 \
  --memory 512Mi

# 使用 AGENTS.md
kias agent create \
  --name my-agent \
  --image python:3.11 \
  --agents-md ./AGENTS.md
```

#### 查看 Agent
```bash
# 列出所有 Agent
kias agent list

# 查看特定 Agent
kias agent describe my-agent

# 查看 Agent 日志
kias agent logs my-agent

# 实时日志
kias agent logs my-agent -f
```

#### 管理 Agent
```bash
# 停止 Agent
kias agent stop my-agent

# 启动 Agent
kias agent start my-agent

# 重启 Agent
kias agent restart my-agent

# 删除 Agent
kias agent delete my-agent
```

### 2.2 节点管理

#### 查看节点
```bash
# 列出所有节点
kias node list

# 查看节点详情
kias node describe node-1

# 查看节点资源
kias node resources node-1
```

#### 管理节点
```bash
# 添加节点（在新节点执行）
kias node join --control-plane http://control-plane:8080

# 移除节点
kias node drain node-1
kias node remove node-1
```

### 2.3 调度管理

#### 查看调度状态
```bash
# 查看调度器状态
kias scheduler status

# 查看调度历史
kias scheduler history

# 查看待调度 Agent
kias scheduler queue
```

#### 配置调度策略
```bash
# 设置调度算法
kias scheduler set-algorithm cache_aware

# 配置亲和性规则
kias scheduler affinity add \
  --agent my-agent \
  --node-label gpu=true
```

### 2.4 监控与追踪

#### Token 消耗查询
```bash
# 查看总消耗
kias token summary

# 查看特定 Agent 消耗
kias token report my-agent

# 查看历史趋势
kias token trend --period 7d
```

#### 健康监控
```bash
# 查看系统健康状态
kias health status

# 查看 Agent 健康状态
kias health agents

# 查看告警
kias alerts list
```

#### 可视化 Dashboard
```bash
# 启动 Dashboard
kias dashboard start --port 3000

# 访问 http://localhost:3000
```

### 2.5 知识管理

#### 摄入知识
```bash
# 摄入单个文件
kias knowledge ingest --source article.md --type article

# 摄入目录
kias knowledge ingest --source ./docs/ --type project

# 监控目录变化
kias knowledge watch --path ./knowledge/
```

#### 查询知识
```bash
# 查询知识
kias knowledge query "KIAS 的调度算法有哪些？"

# 查询并显示来源
kias knowledge query "KIAS 的调度算法有哪些？" --show-sources

# 查询实体关系
kias knowledge graph query --entity "kias-scheduler"
```

#### 维护知识
```bash
# 健康检查
kias knowledge health

# 清理过时内容
kias knowledge cleanup --older-than 30d

# 重建索引
kias knowledge reindex
```

### 2.6 缓存管理

#### 查看缓存状态
```bash
# 查看缓存统计
kias cache stats

# 查看缓存命中率
kias cache hit-rate

# 查看缓存大小
kias cache size
```

#### 管理缓存
```bash
# 清空缓存
kias cache clear

# 预热缓存
kias cache warmup --agents my-agent

# 配置缓存策略
kias cache config set prefix_size 2GB
```

---

## 3. 高级功能

### 3.1 自动扩缩容

```bash
# 启用自动扩缩容
kias autoscaler enable \
  --agent my-agent \
  --min 1 \
  --max 10 \
  --cpu-threshold 70

# 查看扩缩容状态
kias autoscaler status my-agent

# 禁用自动扩缩容
kias autoscaler disable my-agent
```

### 3.2 故障恢复

```bash
# 查看故障恢复配置
kias recovery config

# 手动触发恢复
kias recovery trigger my-agent

# 查看恢复历史
kias recovery history my-agent
```

### 3.3 多租户

```bash
# 创建租户
kias tenant create --name team-a

# 为租户分配资源配额
kias tenant quota set \
  --tenant team-a \
  --cpu 10 \
  --memory 20Gi

# 为租户创建 Agent
kias agent create \
  --name team-a-agent \
  --tenant team-a \
  --image python:3.11
```

### 3.4 审计日志

```bash
# 查看审计日志
kias audit log

# 按时间范围查询
kias audit log --start 2026-05-01 --end 2026-05-13

# 按操作类型查询
kias audit log --action create_agent

# 导出审计日志
kias audit export --format json --output audit.json
```

---

## 4. 配置参考

### 4.1 完整配置示例

```toml
# ~/.kias/config.toml

[api_server]
host = "0.0.0.0"
port = 8080
workers = 4
max_connections = 10000
request_timeout = 30
tls_cert = "/path/to/cert.pem"
tls_key = "/path/to/key.pem"

[scheduler]
algorithm = "cache_aware"  # round_robin, least_loaded, resource_aware, cache_aware
interval = 10
batch_size = 100

[controller]
heartbeat_interval = 5
failure_threshold = 3
recovery_timeout = 60

[agentsight]
enabled = true
ebpf_enabled = true
metrics_port = 9090

[cache_hub]
prefix_cache_size = 1073741824  # 1GB
semantic_cache_enabled = true
redis_url = "redis://localhost:6379"
redis_pool_size = 10

[knowledge]
storage_path = "~/.kias/knowledge"
vector_db_path = "~/.kias/data/knowledge_vectors.db"
graph_db_path = "~/.kias/data/knowledge_graph.db"

[logging]
level = "info"
format = "json"
output = "stdout"  # stdout, file
file_path = "/var/log/kias/kias.log"

[metrics]
enabled = true
port = 9090
path = "/metrics"
```

### 4.2 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `KIAS_CONFIG` | 配置文件路径 | `~/.kias/config.toml` |
| `KIAS_API_HOST` | API 监听地址 | `0.0.0.0` |
| `KIAS_API_PORT` | API 监听端口 | `8080` |
| `KIAS_LOG_LEVEL` | 日志级别 | `info` |
| `KIAS_REDIS_URL` | Redis 地址 | `redis://localhost:6379` |
| `KIAS_ETCD_ENDPOINTS` | etcd 地址 | `http://localhost:2379` |

---

## 5. 故障排查

### 5.1 常见问题

#### 服务启动失败
```bash
# 检查日志
kias logs --tail 100

# 检查配置
kias config validate

# 检查端口占用
lsof -i :8080
```

#### Agent 创建失败
```bash
# 检查节点资源
kias node resources

# 检查调度器状态
kias scheduler status

# 查看详细错误
kias agent create --name my-agent --image python:3.11 --verbose
```

#### 缓存命中率低
```bash
# 查看缓存统计
kias cache stats

# 检查前缀一致性
kias cache prefix list

# 调整缓存大小
kias cache config set prefix_size 2GB
```

### 5.2 日志查看

```bash
# 查看控制平面日志
kias logs control-plane --tail 100

# 查看节点代理日志
kias logs node-agent --node-id node-1 --tail 100

# 实时日志
kias logs control-plane -f

# 按级别过滤
kias logs control-plane --level error
```

### 5.3 性能诊断

```bash
# 查看系统资源使用
kias system resources

# 查看 API 响应时间
kias api latency

# 查看调度延迟
kias scheduler latency

# 生成性能报告
kias diagnostics report --output report.html
```

---

## 6. 最佳实践

### 6.1 资源规划

| Agent 类型 | CPU | Memory | GPU |
|------------|-----|--------|-----|
| 轻量级 | 0.25 | 256Mi | - |
| 标准 | 0.5 | 512Mi | - |
| 计算密集型 | 2 | 4Gi | - |
| GPU 加速型 | 4 | 8Gi | 1 |

### 6.2 调度策略

- **开发环境**：使用 `round_robin` 简单调度
- **生产环境**：使用 `cache_aware` 优化成本
- **高性能场景**：使用 `resource_aware` 确保资源充足

### 6.3 缓存优化

- 保持 System Prompt 一致，提高缓存命中率
- 合理设置 TTL，避免缓存过期
- 监控缓存命中率，及时调整配置

### 6.4 安全建议

- 启用 TLS 加密
- 配置 RBAC 权限
- 定期轮换密钥
- 启用审计日志

---

## 7. API 参考

### 7.1 Agent API

```
POST   /api/v1/agents              # 创建 Agent
GET    /api/v1/agents              # 列出 Agent
GET    /api/v1/agents/{name}       # 获取 Agent
PUT    /api/v1/agents/{name}       # 更新 Agent
DELETE /api/v1/agents/{name}       # 删除 Agent
GET    /api/v1/agents/{name}/logs  # 获取日志
GET    /api/v1/agents/{name}/tokens # 获取 Token 消耗
```

### 7.2 Node API

```
GET    /api/v1/nodes               # 列出节点
GET    /api/v1/nodes/{id}          # 获取节点
DELETE /api/v1/nodes/{id}          # 移除节点
```

### 7.3 Knowledge API

```
POST   /api/v1/knowledge/ingest    # 摄入知识
GET    /api/v1/knowledge/query     # 查询知识
GET    /api/v1/knowledge/graph     # 查询图谱
```

详细 API 文档：`kias docs api`

---

## 8. 支持与反馈

- **文档**：https://docs.kias.dev
- **Issues**：https://github.com/your-org/kias/issues
- **讨论**：https://github.com/your-org/kias/discussions
- **邮件**：support@kias.dev