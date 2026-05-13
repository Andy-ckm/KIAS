# KIAS 缓存策略

> 借鉴 DeepSeek 的 KV Cache 优化，降低 Agent 运行成本

## 核心思想

**Prefix Caching**：相同的前缀复用 KV Cache，避免重复计算

```
请求1: "你是一个专业的..." + "任务A"
请求2: "你是一个专业的..." + "任务B"
         ↑
    共享前缀，复用 KV Cache
```

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│                    Cache Hub                            │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │   Prefix    │  │  Semantic   │  │ Distributed │   │
│  │   Cache     │  │   Cache     │  │   Cache     │   │
│  └─────────────┘  └─────────────┘  └─────────────┘   │
│         │                │                │            │
│         └────────────────┴────────────────┘            │
│                          │                              │
│                    ┌─────┴─────┐                       │
│                    │  Router   │                       │
│                    └───────────┘                       │
└─────────────────────────────────────────────────────────┘
```

## 缓存类型

### 1. Prefix Cache（前缀缓存）

**原理**：相同前缀的请求复用 KV Cache

**适用场景**：
- 相同 System Prompt 的多个请求
- 相同 AGENTS.md 的多个 Agent
- 多轮对话中的历史消息

**实现**：
```rust
struct PrefixCache {
    // 前缀哈希 -> KV Cache
    cache: HashMap<PrefixHash, KvCache>,
    // LRU 淘汰策略
    lru: LruCache<PrefixHash, Instant>,
}

impl PrefixCache {
    fn get(&self, prefix: &str) -> Option<&KvCache> {
        let hash = hash_prefix(prefix);
        self.cache.get(&hash)
    }
    
    fn put(&mut self, prefix: &str, kv: KvCache) {
        let hash = hash_prefix(prefix);
        self.cache.insert(hash, kv);
        self.lru.put(hash, Instant::now());
    }
}
```

**配置**：
```yaml
cache:
  prefix:
    enabled: true
    max_size: 10GB
    ttl: 3600s  # 1小时
```

### 2. Semantic Cache（语义缓存）

**原理**：相似语义的请求命中缓存

**适用场景**：
- 意思相同但表述不同的请求
- 常见问题的标准回答

**实现**：
```rust
struct SemanticCache {
    // 向量索引
    index: HnswIndex,
    // 语义哈希 -> 响应
    cache: HashMap<SemanticHash, Response>,
}

impl SemanticCache {
    fn get(&self, query: &str) -> Option<&Response> {
        let embedding = embed(query);
        let neighbors = self.index.search(embedding, k=5);
        
        for neighbor in neighbors {
            if neighbor.similarity > 0.95 {
                return self.cache.get(&neighbor.hash);
            }
        }
        None
    }
}
```

**配置**：
```yaml
cache:
  semantic:
    enabled: true
    similarity_threshold: 0.95
    embedding_model: text-embedding-3-small
```

### 3. Distributed Cache（分布式缓存）

**原理**：跨节点共享缓存

**适用场景**：
- 多节点集群
- Agent 迁移后继续使用缓存

**实现**：
```rust
struct DistributedCache {
    // 本地缓存
    local: PrefixCache,
    // Redis 连接
    redis: RedisClient,
    // 一致性哈希
    ring: ConsistentHashRing,
}

impl DistributedCache {
    async fn get(&self, key: &str) -> Option<KvCache> {
        // 先查本地
        if let Some(kv) = self.local.get(key) {
            return Some(kv);
        }
        
        // 再查 Redis
        let node = self.ring.get_node(key);
        let kv = self.redis.get(key).await?;
        
        // 写入本地缓存
        self.local.put(key, kv.clone());
        Some(kv)
    }
}
```

**配置**：
```yaml
cache:
  distributed:
    enabled: true
    redis_url: redis://localhost:6379
    consistency: eventual  # eventual | strong
```

## 缓存命中策略

### 命中流程
```
1. 接收请求
2. 提取前缀
3. 计算哈希
4. 查询缓存
   ├── 命中 → 直接返回
   └── 未命中 → 调用模型 → 存入缓存
```

### 淘汰策略
- **LRU**：最近最少使用
- **LFU**：最不经常使用
- **TTL**：过期时间

### 预热策略
```rust
async fn warmup_cache(&self, agents: &[Agent]) {
    for agent in agents {
        let prefix = agent.system_prompt();
        if !self.cache.contains(prefix) {
            // 预计算 KV Cache
            let kv = compute_kv_cache(prefix).await;
            self.cache.put(prefix, kv);
        }
    }
}
```

## 成本优化

### 成本计算
```rust
struct CostCalculator {
    // 每 Token 成本
    input_cost_per_token: f64,
    output_cost_per_token: f64,
}

impl CostCalculator {
    fn calculate(&self, input_tokens: u64, output_tokens: u64, cache_hit: bool) -> f64 {
        if cache_hit {
            // 缓存命中，成本降低 90%
            self.input_cost_per_token * input_tokens as f64 * 0.1
        } else {
            self.input_cost_per_token * input_tokens as f64
        }
        + self.output_cost_per_token * output_tokens as f64
    }
}
```

### 成本报告
```bash
# 查看成本报告
kias agent cost my-agent --period 7d

# 输出
# Input tokens: 1,000,000
# Output tokens: 500,000
# Cache hit rate: 85%
# Total cost: $12.50
# Savings: $45.00 (78%)
```

## 监控指标

### Prometheus 指标
```yaml
# 缓存命中率
kias_cache_hit_rate{type="prefix"} 0.85
kias_cache_hit_rate{type="semantic"} 0.45

# 缓存大小
kias_cache_size_bytes{type="prefix"} 1073741824

# 缓存延迟
kias_cache_lookup_duration_seconds{type="prefix"} 0.001
```

### Dashboard 可视化
- 缓存命中率趋势
- 缓存大小变化
- 成本节省统计

## 最佳实践

### 1. 优化 System Prompt
```yaml
# 好的实践：统一 System Prompt
system_prompt: |
  你是一个专业的 AI 助手。
  请用中文回答问题。
  保持简洁明了。

# 不好的实践：每个请求不同的 System Prompt
system_prompt: "你是... (每次不同)"
```

### 2. 合理设置 TTL
```yaml
cache:
  prefix:
    ttl: 3600s  # 1小时，根据使用频率调整
  semantic:
    ttl: 86400s  # 24小时
```

### 3. 监控缓存命中率
```bash
# 实时监控
watch -n 1 'curl -s localhost:9090/metrics | grep cache_hit_rate'
```

## 参考

- [DeepSeek-V2](https://arxiv.org/abs/2405.04532)
- [Prefix Caching](https://arxiv.org/abs/2310.03991)