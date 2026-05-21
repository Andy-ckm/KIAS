# 第五部分：可观测性层竞品分析

## 5.1 LangSmith（LangChain 官方）

### 基本信息
| 指标 | 数据 |
|------|------|
| 公司 | LangChain Inc. |
| 定位 | LLM 应用可观测平台 |
| 部署 | 云服务 |
| 集成 | LangChain 原生 |

### 核心功能
1. **调用链追踪** — 完整的 LLM 调用链
2. **评估框架** — 自动化评估
3. **数据集管理** — 训练/测试数据
4. **Prompt 管理** — 版本控制
5. **成本追踪** — Token 使用统计
6. **调试工具** — 交互式调试

### 定价
| 版本 | 价格 | 功能 |
|------|------|------|
| Developer | 免费 | 5K traces/月 |
| Plus | $39/月 | 100K traces/月 |
| Enterprise | 定制 | 无限、SLA |

### 优势
- ✅ LangChain 原生集成
- ✅ 调用链完整
- ✅ 评估框架强大

### 劣势
- ❌ 绑定 LangChain
- ❌ 无治理功能
- ❌ 无合规追踪
- ❌ 闭源

## 5.2 LangFuse（8K+ Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | langfuse/langfuse |
| Stars | 8,000+ |
| 语言 | TypeScript |
| 许可证 | MIT |
| 定位 | 开源 LLM 可观测 |

### 核心功能
1. **追踪** — LLM 调用追踪
2. **评估** — 自动化评估
3. **Prompt 管理** — 版本控制
4. **数据集** — 测试数据
5. **用户分析** — 用户行为分析

### 定价
| 版本 | 价格 | 功能 |
|------|------|------|
| Cloud Free | 免费 | 50K events/月 |
| Cloud Pro | $59/月 | 500K events/月 |
| Self-Host | 免费 | 自行部署 |

### 优势
- ✅ 开源
- ✅ 框架无关
- ✅ 自部署选项

### 劣势
- ❌ 无治理功能
- ❌ 无合规追踪
- ❌ 功能较浅

## 5.3 Arize Phoenix（5K+ Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | Arize-ai/phoenix |
| Stars | 5,000+ |
| 语言 | Python |
| 许可证 | Apache-2.0 |
| 定位 | LLM 可观测性 |

### 核心功能
1. **追踪** — OpenTelemetry 集成
2. **评估** — LLM 评估
3. **嵌入分析** — 向量空间可视化
4. **漂移检测** — 数据漂移

### 优势
- ✅ OpenTelemetry 原生
- ✅ 嵌入分析创新
- ✅ 开源

### 劣势
- ❌ 无治理功能
- ❌ 无合规追踪

## 5.4 AgentOps（2K+ Stars）

### 核心功能
1. **会话回放** — Agent 会话录制
2. **成本追踪** — Token 使用
3. **错误追踪** — 异常监控
4. **LLM 调用分析** — 调用统计

### 优势
- ✅ 会话回放创新
- ✅ Agent 专用

### 劣势
- ❌ 功能较浅
- ❌ 无治理功能

## 5.5 其他可观测工具

### Weights & Biases Weave
- **定位：** Agent 追踪 + 实验管理
- **优势：** W&B 生态集成
- **劣势：** 不专注 Agent 治理

### Braintrust
- **定位：** AI 评估平台
- **优势：** 评估框架强大
- **劣势：** 只做评估

### Datadog LLM Observability
- **定位：** LLM 监控
- **优势：** Datadog 生态
- **劣势：** 通用，不专注 Agent

### New Relic AI Monitoring
- **定位：** AI 监控
- **优势：** New Relic 生态
- **劣势：** 通用

### OpenLIT（2K+ Stars）
- **定位：** OpenTelemetry AI
- **优势：** 原生 OTel
- **劣势：** 只做遥测

## 5.6 可观测性层总结

| 能力 | LangSmith | LangFuse | Phoenix | AgentOps | AgentGuard |
|------|-----------|----------|---------|----------|-----------|
| 追踪 | ✅ | ✅ | ✅ | ✅ | ✅ |
| 评估 | ✅ | ✅ | ✅ | ❌ | ✅ |
| 治理 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 合规 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 成本 | ⚠️ | ❌ | ❌ | ✅ | ✅ |
| 开源 | ❌ | ✅ | ✅ | ❌ | ✅ |

**关键洞察：** 可观测工具只做"看"，不做"控"。AgentGuard = 看 + 控 + 审 + 合规。

---

# 第六部分：LLM 网关层竞品分析

## 6.1 LiteLLM（48K Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | BerriAI/litellm |
| Stars | 47,755 |
| 语言 | Python |
| 许可证 | MIT |
| 定位 | LLM 统一代理 |

### 核心功能
1. **统一 API** — 100+ LLM 提供商统一接口
2. **负载均衡** — 多 key 轮换
3. **限流** — 速率限制
4. **缓存** — 响应缓存
5. **Fallback** — 故障转移
6. **预算管理** — 预算控制

### 定价
| 版本 | 价格 | 功能 |
|------|------|------|
| Open Source | 免费 | 核心功能 |
| Cloud | $50/月 | 托管、监控 |
| Enterprise | 定制 | SLA、定制 |

### 优势
- ✅ 100+ 提供商支持
- ✅ 负载均衡成熟
- ✅ 社区活跃

### 劣势
- ❌ 无 Agent 治理
- ❌ 无审计追踪
- ❌ 无合规功能

## 6.2 Portkey Gateway（12K Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | Portkey-AI/gateway |
| Stars | 11,807 |
| 语言 | TypeScript |
| 许可证 | MIT |
| 定位 | AI 网关 + 护栏 |

### 核心功能
1. **Guardrails** — 内置护栏
2. **Caching** — 智能缓存
3. **Fallback** — 故障转移
4. **Load Balancing** — 负载均衡
5. **Analytics** — 分析
6. **Observability** — 可观测

### 优势
- ✅ 集成护栏
- ✅ 高性能（50x faster than LiteLLM）
- ✅ TypeScript 实现

### 劣势
- ❌ 只做网关
- ❌ 无 Agent 生命周期管理
- ❌ 无合规功能

## 6.3 Bifrost（5K+ Stars）

### 核心功能
1. **最快 AI 网关** — 声称 50x faster
2. **多提供商** — 统一接口
3. **缓存** — 智能缓存
4. **容错** — 故障转移

### 优势
- ✅ 极致性能
- ✅ Go 实现

### 劣势
- ❌ 功能较浅
- ❌ 无治理功能

---

# 第七部分：企业 AI 平台分析

## 7.1 Anthropic Claude Console

### Agent 能力
- **治理 API** — 审计日志、使用统计
- **安全** — 内置安全护栏
- **合规** — SOC 2、HIPAA

### 缺什么
- ❌ 只管自家模型
- ❌ 无跨框架治理
- ❌ 无 GxP 合规

## 7.2 OpenAI Platform

### Agent 能力
- **Assistants API** — Agent 构建
- **函数调用** — 工具集成
- **Tracing** — 调用追踪

### 缺什么
- ❌ 绑定 OpenAI
- ❌ 无跨框架治理
- ❌ 无合规功能

## 7.3 Google Vertex AI Agent Builder

### Agent 能力
- **Agent 构建** — 可视化构建
- **Grounding** — 事实性保证
- **搜索** — 搜索集成

### 缺什么
- ❌ GCP 锁定
- ❌ 无跨框架治理
- ❌ 无 GxP 合规

## 7.4 AWS Bedrock Agents

### Agent 能力
- **Agent 构建** — 可视化构建
- **知识库** — RAG 集成
- **Action Groups** — 工具组

### 缺什么
- ❌ AWS 锁定
- ❌ 无跨框架治理
- ❌ 无 GxP 合规

## 7.5 Azure AI Agent Service

### Agent 能力
- **Agent 构建** — 代码优先
- **集成** — Azure 生态
- **安全** — Azure AD

### 缺什么
- ❌ Azure 锁定
- ❌ 无跨框架治理
- ❌ 无 GxP 合规

## 7.6 企业平台总结

| 平台 | 厂商 | Agent 能力 | 跨框架 | GxP | 开源 |
|------|------|-----------|--------|-----|------|
| Claude Console | Anthropic | 治理 API | ❌ | ❌ | ❌ |
| OpenAI Platform | OpenAI | Assistants | ❌ | ❌ | ❌ |
| Vertex AI | Google | Agent Builder | ❌ | ❌ | ❌ |
| Bedrock Agents | AWS | Agent Builder | ❌ | ❌ | ❌ |
| Azure AI Agent | Azure | Agent Service | ❌ | ❌ | ❌ |
| **AgentGuard** | **开源** | **全栈治理** | **✅** | **✅** | **✅** |

**关键洞察：** 云厂商只管自己生态里的 Agent。AgentGuard 是**跨模型、跨云、跨框架**的治理层。
