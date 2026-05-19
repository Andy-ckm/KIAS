# LiteLLM Router 设计分析

> 源文件: `litellm/router.py` (10,897 行, ~451KB)
> 分析日期: 2026-05-14

## 1. 整体架构

LiteLLM Router 是一个 **LLM 请求路由 + 负载均衡 + 故障转移** 的核心组件，位于 `litellm/router.py`，约 10,900 行代码。

```
请求入口 (acompletion / completion / aembedding / ...)
    │
    ▼
┌─────────────────────────────────────────┐
│  Router                                  │
│  ┌───────────────────────────────────┐  │
│  │ 1. Pre-Routing Hook               │  │  ← 可修改 model/messages
│  │ 2. Model Alias 解析               │  │  ← gpt-4 → gpt-3.5-turbo
│  │ 3. 获取候选 Deployments            │  │  ← model_name → [dep1, dep2, ...]
│  │ 4. 过滤 (access group / team)     │  │  ← 权限控制
│  │ 5. 健康检查过滤                    │  │  ← 排除不健康节点
│  │ 6. Cooldown 过滤                  │  │  ← 排除冷却中节点
│  │ 7. Pre-call Check (context window) │  │  ← 上下文窗口检查
│  │ 8. Order 排序                     │  │  ← 按 order 优先级
│  │ 9. 路由策略选择 Deployment          │  │  ← 负载均衡算法
│  │ 10. 发起 LLM 调用                  │  │
│  │ 11. 失败 → Retry → Fallback       │  │  ← 故障转移链
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## 2. 路由策略 (Routing Strategies)

### 2.1 支持的策略

| 策略 | 值 | 说明 |
|------|-----|------|
| `simple-shuffle` | 默认 | 基于 weight 的随机加权选择 |
| `least-busy` | 最少忙碌 | 基于并发请求数选择最空闲的 deployment |
| `usage-based-routing` | TPM/RPM v1 | 基于 token 使用量的最低负载选择 |
| `usage-based-routing-v2` | TPM/RPM v2 | v2 版本，支持异步获取 |
| `latency-based-routing` | 延迟优先 | 选择历史延迟最低的 deployment |
| `cost-based-routing` | 成本优先 | 选择成本最低的 deployment |

### 2.2 策略初始化

```python
# 策略通过 _build_strategy_selector 构造选择器
def _build_strategy_selector(self, strategy, routing_strategy_args):
    match strategy:
        case "least-busy":
            selector = LeastBusyLoggingHandler(router_cache=self.cache)
        case "usage-based-routing":
            selector = LowestTPMLoggingHandler(router_cache=self.cache, ...)
        case "usage-based-routing-v2":
            selector = LowestTPMLoggingHandler_v2(router_cache=self.cache, ...)
        case "latency-based-routing":
            selector = LowestLatencyLoggingHandler(router_cache=self.cache, ...)
        case "cost-based-routing":
            selector = LowestCostLoggingHandler(router_cache=self.cache, ...)
```

**设计特点**: 策略选择器作为 callback 注册到 LiteLLM 全局回调系统，通过请求成功/失败的回调自动更新统计信息。

### 2.3 Routing Groups (路由组)

Router 支持将 model 分组，每组可使用不同的路由策略：

```python
routing_groups = [
    RoutingGroup(
        group_name="premium",
        models=["gpt-4", "claude-3"],
        routing_strategy="latency-based-routing",
        routing_strategy_args={"window_size": 600}
    ),
    RoutingGroup(
        group_name="budget",
        models=["gpt-3.5-turbo"],
        routing_strategy="cost-based-routing",
    )
]
```

- 每个 model 最多属于一个显式组
- 不属于任何组的 model 使用隐式 "default" 组
- 通过 `_get_routing_context(model)` 解析路由上下文

## 3. Deployment 选择流程

### 3.1 核心入口: `get_available_deployment()` / `async_get_available_deployment()`

```
get_available_deployment(model)
    │
    ├─ 1. _common_checks_available_deployment()
    │     ├─ specific_deployment=True → 直接返回指定 deployment
    │     ├─ model_id 精确匹配
    │     ├─ model_alias 解析 (gpt-4 → gpt-3.5-turbo)
    │     ├─ team-specific deployment 查找
    │     ├─ pattern matching (通配符路由: openai/*)
    │     ├─ default_deployment 降级
    │     └─ _get_all_deployments() + access_group 过滤
    │
    ├─ 2. _filter_health_check_unhealthy_deployments()
    │     └─ 排除健康检查标记为不健康的节点
    │
    ├─ 3. _filter_cooldown_deployments()
    │     └─ 排除处于冷却期的节点
    │
    ├─ 4. _pre_call_checks() (可选)
    │     ├─ context window 检查
    │     ├─ rate limit 检查
    │     └─ prompt caching deployment 检查
    │
    ├─ 5. order 排序过滤
    │     └─ 选择 order 值最小的 deployment
    │
    └─ 6. 策略选择
          ├─ simple-shuffle → simple_shuffle() (加权随机)
          └─ 其他策略 → _select_deployment_sync/async()
```

### 3.2 Pattern Match Router (通配符路由)

支持模型名的通配符匹配，如 `openai/*` 匹配所有 OpenAI 模型：

```python
self.pattern_router = PatternMatchRouter()
self.team_pattern_routers: Dict[str, PatternMatchRouter] = {}
```

### 3.3 索引优化

使用多个索引实现 O(1) 查找：

```python
self.model_id_to_deployment_index_map: Dict[str, int] = {}
self.model_name_to_deployment_indices: Dict[str, List[int]] = {}
self.team_model_to_deployment_indices: Dict[Tuple[str, str], List[int]] = {}
```

## 4. 故障转移 (Fallback) 设计

### 4.1 Fallback 层次

```
请求失败
    │
    ├─ 1. Retry (重试同一 deployment)
    │     ├─ num_retries 次重试
    │     ├─ 支持 RetryPolicy 按错误类型设置不同重试次数
    │     └─ _time_to_sleep_before_retry() 计算退避时间
    │
    ├─ 2. 同组内切换 (其他 healthy deployment)
    │     └─ 有其他 healthy deployment 时立即重试(不等待)
    │
    ├─ 3. Order-based Fallback (按优先级降级)
    │     └─ 同 model_group 内，从 order=1 降级到 order=2
    │
    ├─ 4. Context Window Fallback
    │     └─ ContextWindowExceededError → 切换到更大上下文窗口的模型
    │
    ├─ 5. Content Policy Fallback
    │     └─ ContentPolicyViolationError → 切换到其他模型
    │
    └─ 6. Generic Fallback (通用降级)
          ├─ 指定 fallback: [{"gpt-4": ["gpt-3.5-turbo"]}]
          ├─ 通配符 fallback: [{"*": ["gpt-3.5-turbo"]}]
          └─ max_fallbacks 限制最大降级次数 (默认 5)
```

### 4.2 Fallback 执行流程

```python
async def async_function_with_fallbacks(self, *args, **kwargs):
    try:
        response = await self.async_function_with_retries(*args, **kwargs)
        return response
    except Exception as e:
        return await self.async_function_with_fallbacks_common_utils(
            e, disable_fallbacks, fallbacks,
            context_window_fallbacks, content_policy_fallbacks,
            model_group, args, kwargs
        )
```

**关键设计**:
- 先尝试 Order-based Fallback (同组内按优先级降级)
- 再尝试外部 Fallback (切换到其他 model group)
- `max_fallbacks` 限制最大降级深度，防止无限降级

### 4.3 错误分类处理

```python
def should_retry_this_error(self, error, healthy_deployments, ...):
    # ContextWindowExceededError + 有 context_window_fallbacks → 降级
    # ContentPolicyViolationError + 有 content_policy_fallbacks → 降级
    # RateLimitError + 无 healthy deployments + 有 fallbacks → 降级
    # AuthenticationError + 只有 1 个 deployment → 不重试
    # NotFoundError → 不重试
```

## 5. 冷却机制 (Cooldown)

### 5.1 冷却触发条件

```python
def deployment_callback_on_failure(self, kwargs, ...):
    # 1. 部署级别配置的 cooldown_time
    # 2. 响应头中的 Retry-After
    # 3. Router 默认 cooldown_time
    # 优先级: deployment config > response header > router default
```

### 5.2 冷却数据结构

```python
self.cooldown_cache = CooldownCache(
    cache=self.cache,  # DualCache (Redis + InMemory)
    default_cooldown_time=self.cooldown_time
)
self.failed_calls = InMemoryCache()  # 跟踪每分钟失败次数
```

### 5.3 冷却过滤

```python
def _filter_cooldown_deployments(self, healthy_deployments, cooldown_deployments):
    cooldown_set = set(cooldown_deployments)  # O(1) 查找
    return [
        d for d in healthy_deployments
        if d["model_info"]["id"] not in cooldown_set
    ]
```

**安全网**: 如果所有 deployment 都在冷却中，会绕过冷却过滤，避免完全不可用。

## 6. 健康检查 (Health Check)

### 6.1 健康状态缓存

```python
self.health_state_cache = DeploymentHealthCache(
    cache=self.cache,
    staleness_threshold=float(_staleness)  # 默认 DEFAULT_HEALTH_CHECK_INTERVAL * MULTIPLIER
)
```

### 6.2 健康检查过滤

```python
async def _async_filter_health_check_unhealthy_deployments(self, ...):
    if not self.enable_health_check_routing:
        return healthy_deployments  # 默认关闭

    if self.allowed_fails_policy is not None:
        return healthy_deployments  # 有策略时用 cooldown 机制

    unhealthy_ids = await self.health_state_cache.async_get_unhealthy_deployment_ids()
    filtered = [d for d in healthy_deployments if d["model_info"]["id"] not in unhealthy_ids]

    if not filtered:  # 安全网: 全部不健康时返回全部
        return healthy_deployments
    return filtered
```

## 7. 缓存系统

### 7.1 DualCache 架构

```python
self.cache = DualCache(
    redis_cache=redis_cache,      # Redis/RedisCluster (分布式)
    in_memory_cache=InMemoryCache()  # 本地内存 (快速)
)
```

### 7.2 缓存用途

| 缓存键模式 | 用途 |
|-----------|------|
| `TPM:{id}:{minute}:{model}` | Token/分钟 使用量追踪 |
| `RPM:{id}:{minute}:{model}` | 请求/分钟 使用量追踪 |
| Cooldown cache | Deployment 冷却状态 |
| Health state cache | Deployment 健康状态 |
| Response cache | LLM 响应缓存 (可选) |

### 7.3 使用量追踪

```python
async def deployment_callback_on_success(self, kwargs, ...):
    # 通过 Redis Pipeline 批量更新 TPM/RPM
    pipeline_operations = [
        RedisPipelineIncrementOperation(key=tpm_key, increment_value=total_tokens, ttl=60),
        RedisPipelineIncrementOperation(key=rpm_key, increment_value=1, ttl=60),
    ]
    await self.cache.async_increment_cache_pipeline(increment_list=pipeline_operations)
```

## 8. 重试机制

### 8.1 重试退避策略

```python
def _time_to_sleep_before_retry(self, e, remaining_retries, num_retries, ...):
    # 有其他 healthy deployment → 立即重试 (sleep=0)
    # 只有 1 个 deployment → 根据 Retry-After 头或指数退避
```

### 8.2 RetryPolicy 支持

```python
retry_policy = RetryPolicy(
    BadRequestErrorRetries=0,
    AuthenticationErrorRetries=3,
    TimeoutErrorRetries=2,
    RateLimitErrorRetries=5,
    ContentPolicyViolationErrorRetries=0,
)
# 还支持 per-model-group 的 retry policy
model_group_retry_policy = {"gpt-4": RetryPolicy(...)}
```

## 9. Pre-Routing Hook

### 9.1 自动路由 (AutoRouter)

```python
async def async_pre_routing_hook(self, model, request_kwargs, messages, ...):
    # 支持在路由决策前修改 model 和 messages
    # 用于 AutoRouter / ComplexityRouter / AdaptiveRouter / QualityRouter
```

### 9.2 支持的高级路由

| Router | 说明 |
|--------|------|
| AutoRouter | 自动选择最佳模型 |
| ComplexityRouter | 基于任务复杂度选择模型 |
| AdaptiveRouter | 自适应路由 |
| QualityRouter | 基于质量评分路由 |

## 10. Scheduler (优先级调度)

```python
self.scheduler = Scheduler(
    polling_interval=polling_interval,
    redis_cache=redis_cache
)
self.default_priority = default_priority
```

支持带优先级的请求调度，通过 `scheduler_acompletion()` 入口使用。

## 11. Factory Function 模式

Router 使用 `factory_function()` 为所有 API 端点创建包装函数：

```python
self.acompletion = self.factory_function(litellm.acompletion)
self.aembedding = self.factory_function(litellm.aembedding)
self.amoderation = self.factory_function(litellm.amoderation, call_type="moderation")
# ... 50+ 个 API 端点
```

每个包装函数自动获得:
- 路由选择
- 重试逻辑
- Fallback 逻辑
- 使用量追踪
- 错误处理

## 12. 关键设计模式总结

### 12.1 分层过滤器模式
请求经过一系列过滤器逐步缩小候选范围:
```
全部 deployments → access_group 过滤 → 健康检查过滤 → cooldown 过滤 → pre_call 过滤 → order 过滤 → 策略选择
```

### 12.2 回调驱动的统计更新
策略选择器作为 callback 注册，通过请求成功/失败的回调自动更新统计信息，实现解耦。

### 12.3 安全网模式
多个地方有 "安全网" 逻辑:
- 所有 deployment 在冷却中 → 绕过冷却
- 所有 deployment 不健康 → 绕过健康检查
- 无可用 deployment → 尝试 default fallback

### 12.4 双缓存架构
`DualCache` = Redis (分布式一致性) + InMemory (本地速度)，兼顾性能和分布式场景。

### 12.5 渐进式降级
从同一 deployment 重试 → 同组其他 deployment → order 降级 → 跨组 fallback，层层递进。

## 13. 路由策略实现细节

### 13.1 simple-shuffle (默认)

基于 `weight` 参数的加权随机选择。每个 deployment 可设置权重，权重越高被选中概率越大。如果 deployment 配置了 `tpm`/`rpm` 限制，则基于这些值做加权。

### 13.2 least-busy (最少忙碌)

```python
class LeastBusyLoggingHandler(CustomLogger):
    # 通过 callback 机制追踪每个 deployment 的并发请求数
    # log_pre_api_call: 请求开始 → 计数 +1
    # log_success_event: 请求成功 → 计数 -1
    # log_failure_event: 请求失败 → 计数 -1
    # get_available_deployments: 选择并发数最低的 deployment
```

**实现方式**: 使用 DualCache 存储 `{model_group}_request_count` 键，值为 `{deployment_id: count}` 字典。选择时取 count 最小的 deployment。

### 13.3 usage-based-routing (TPM/RPM v1)

```python
class LowestTPMLoggingHandler:
    # 基于 Token Per Minute 和 Request Per Minute 的使用量路由
    # 通过 callback 追踪每次请求的 token 消耗
    # 选择当前分钟内 TPM/RPM 最低的 deployment
```

### 13.4 usage-based-routing-v2 (TPM/RPM v2)

v2 版本改进：
- 支持异步获取 (`async_get_available_deployments`)
- 使用 Redis Pipeline 批量更新 TPM/RPM (性能提升)
- 更精确的使用量追踪

### 13.5 latency-based-routing (延迟优先)

```python
class LowestLatencyLoggingHandler:
    # 维护每个 deployment 的滑动窗口延迟统计
    # 选择历史平均延迟最低的 deployment
    # routing_strategy_args: {"window_size": 600}  # 窗口大小(秒)
```

### 13.6 cost-based-routing (成本优先)

```python
class LowestCostLoggingHandler:
    # 基于每个 deployment 配置的成本信息
    # 选择每 token 成本最低的 deployment
    # 仅支持异步模式
```

### 13.7 策略选择器注册机制

所有策略选择器都作为 LiteLLM 全局 callback 注册：

```python
# 注册到回调系统
litellm.logging_callback_manager.add_litellm_callback(selector)

# 回调自动触发:
# 1. 请求开始 → log_pre_api_call (更新并发计数)
# 2. 请求成功 → log_success_event (更新 TPM/RPM/延迟统计)
# 3. 请求失败 → log_failure_event (更新失败计数)
```

这种设计实现了 **策略与路由逻辑的解耦**：路由选择器不需要直接调用 LLM，而是通过回调被动收集统计信息。

## 14. 与 AgentGuard Scheduler 的对比参考

| 维度 | LiteLLM Router | AgentGuard Scheduler |
|------|---------------|----------------|
| 语言 | Python | Rust |
| 路由策略 | 6 种 (simple-shuffle/least-busy/usage/latency/cost) | 4 种 (RR/LL/RA/CA) |
| 故障转移 | Retry + Fallback 链 (深度=5) | 指数退避恢复 |
| 健康检查 | 可选后台检查 + cooldown | 心跳监控 |
| 缓存 | DualCache (Redis+InMemory) | etcd + Redis |
| 优先级调度 | Scheduler (polling) | 优先级队列 |
| 状态追踪 | TPM/RPM per-deployment | AgentStatus |

---

## 附录: 核心方法索引

| 方法 | 行号 | 说明 |
|------|------|------|
| `Router.__init__()` | 234 | 构造函数，初始化所有子系统 |
| `routing_strategy_init()` | 926 | 路由策略初始化 |
| `_init_routing_groups()` | 959 | 路由组初始化 |
| `_get_routing_context()` | 1037 | 解析 model 的路由上下文 |
| `_select_deployment_async()` | 1068 | 异步策略选择 |
| `_select_deployment_sync()` | 1121 | 同步策略选择 |
| `acompletion()` | 1978 | 异步 completion 入口 |
| `async_function_with_fallbacks()` | 5895 | 异步 fallback 执行 |
| `async_function_with_retries()` | 5994 | 异步重试执行 |
| `should_retry_this_error()` | 6243 | 错误重试判断 |
| `_time_to_sleep_before_retry()` | 6361 | 重试退避计算 |
| `deployment_callback_on_success()` | 6412 | 成功回调 (更新 TPM/RPM) |
| `deployment_callback_on_failure()` | 6577 | 失败回调 (触发 cooldown) |
| `_common_checks_available_deployment()` | 9736 | 通用 deployment 检查 |
| `get_available_deployment()` | 10385 | 同步 deployment 选择入口 |
| `async_get_available_deployment()` | 10072 | 异步 deployment 选择入口 |
| `_filter_cooldown_deployments()` | 10651 | cooldown 过滤 |
| `_filter_health_check_unhealthy_deployments()` | 10714 | 健康检查过滤 |
