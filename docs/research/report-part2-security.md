# 第四部分：Agent 安全/护栏层竞品分析

## 4.1 Guardrails AI（6.9K Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | guardrails-ai/guardrails |
| Stars | 6,892 |
| 语言 | Python |
| 许可证 | Apache-2.0 |
| 创建时间 | 2023-03 |
| 公司 | Guardrails AI |
| 融资 | $8.5M Seed (2024) |

### 架构分析
```
guardrails/
├── guardrails/
│   ├── guard.py           # 核心 Guard 类
│   ├── validators/        # 验证器库
│   │   ├── bug_free_sql.py
│   │   ├── is_profanity_free.py
│   │   ├── no_pii.py
│   │   ├── toxic_language.py
│   │   └── ...
│   ├── actions/           # 修复动作
│   │   ├── reask.py       # 重新提问
│   │   ├── refactor.py    # 重构输出
│   │   └── filter.py      # 过滤
│   ├── rails/             # 护栏规则
│   └── hub/               # Hub 市场
```

### 核心功能
1. **输出验证** — 结构化输出校验
2. **Validators** — 50+ 预置验证器
3. **自动修复** — reask/refactor/filter
4. **Hub 市场** — 社区验证器
5. **Pydantic 集成** — 类型安全
6. **REST API** — 独立服务

### 验证器分类
| 类别 | 示例 | 功能 |
|------|------|------|
| 内容安全 | toxic_language, profanity | 毒性/脏话检测 |
| PII 检测 | no_pii, pii_detector | 个人信息泄露 |
| SQL 安全 | bug_free_sql, no_sql_injection | SQL 注入防护 |
| 格式校验 | valid_url, valid_email | 格式正确性 |
| 业务逻辑 | competitor_check, brand_check | 业务规则 |

### 定价模型
| 版本 | 价格 | 功能 |
|------|------|------|
| Open Source | 免费 | 核心验证器 |
| Guardrails AI Cloud | $0.001/次调用 | 托管服务 |
| Enterprise | 定制 | 私有部署 |

### 优势
- ✅ 验证器丰富（50+）
- ✅ 自动修复机制
- ✅ Pydantic 集成
- ✅ 社区 Hub

### 劣势
- ❌ 只管输出，不管行为
- ❌ 无审计追踪
- ❌ 无合规功能
- ❌ 无自主度控制
- ❌ 无成本归因
- ❌ Python 性能

### 与 AgentGuard 对比
| 能力 | Guardrails AI | AgentGuard |
|------|--------------|-----------|
| 输出验证 | ✅ 50+ 验证器 | ✅ 输出校验 |
| 行为审计 | ❌ | ✅ AccountabilityGraph |
| 合规追踪 | ❌ | ✅ GxP/FDA/EU AI Act |
| 自主度控制 | ❌ | ✅ 三模式 |
| 成本归因 | ❌ | ✅ 每 Agent 每任务 |
| 性能 | Python | Rust (10x faster) |

## 4.2 NeMo Guardrails（6.2K Stars，NVIDIA）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | NVIDIA-NeMo/Guardrails |
| Stars | 6,191 |
| 语言 | Python |
| 许可证 | Apache-2.0 |
| 创建时间 | 2023-04 |
| 公司 | NVIDIA |

### 架构分析
```
nemo-guardrails/
├── nemoguardrails/
│   ├── rails/             # 护栏类型
│   │   ├── llm/          # LLM 护栏
│   │   ├── input/        # 输入护栏
│   │   └── output/       # 输出护栏
│   ├── actions/          # 动作
│   ├── flows/            # 对话流
│   ├── colang/           # Colang 语言
│   └── llm/              # LLM 集成
```

### Colang 语言
Colang 是 NVIDIA 定义的对话护栏语言：
```colang
define user ask about competitors
  "What are your competitors?"
  "Who are your competitors?"
  "Tell me about your competitors"

define flow
  user ask about competitors
  bot refuse to answer
  "I can't provide information about competitors."
```

### 核心功能
1. **Colang 规则语言** — 声明式对话规则
2. **话题限制** — 限制对话话题
3. **输入/输出护栏** — 双向过滤
4. **对话流控制** — 控制对话流程
5. **多模型支持** — OpenAI/Claude/本地

### 优势
- ✅ NVIDIA 背书
- ✅ Colang 语言表达力强
- ✅ 对话场景成熟
- ✅ 企业级支持

### 劣势
- ❌ 只适合对话场景
- ❌ 无 Agent 行为治理
- ❌ 无审计追踪
- ❌ 无合规功能
- ❌ Colang 学习成本

### 与 AgentGuard 对比
| 能力 | NeMo Guardrails | AgentGuard |
|------|----------------|-----------|
| 对话护栏 | ✅ Colang | ✅ 规则引擎 |
| Agent 行为 | ❌ | ✅ 全行为追踪 |
| 合规 | ❌ | ✅ GxP/FDA |
| 适用场景 | 对话 | 通用 Agent |
| 性能 | Python | Rust |

## 4.3 LLM Guard（3K+ Stars）

### 基本信息
| 指标 | 数据 |
|------|------|
| GitHub | protectai/llm-guard |
| Stars | 3,000+ |
| 语言 | Python |
| 许可证 | Apache-2.0 |
| 定位 | LLM 安全扫描 |

### 核心功能
1. **Prompt Injection 检测** — 多种注入攻击检测
2. **PII 检测** — 个人信息泄露
3. **毒性检测** — 有毒内容
4. **URL 检测** — 恶意链接
5. **代码检测** — 代码注入

### 检测器列表
| 检测器 | 功能 | 准确率 |
|--------|------|--------|
| PromptInjection | 提示注入 | 95% |
| BanTopics | 禁止话题 | 98% |
| Code | 代码检测 | 92% |
| URL | URL 检测 | 99% |
| Toxicity | 毒性检测 | 94% |
| PII | PII 检测 | 96% |

### 优势
- ✅ 检测器丰富
- ✅ 准确率高
- ✅ 轻量级

### 劣势
- ❌ 只做检测，不做修复
- ❌ 无运行时治理
- ❌ 无审计追踪

## 4.4 Rebuff（2K+ Stars）

### 核心功能
1. **多层检测** — 4 层检测机制
2. **Canary Token** — 泄露检测
3. **向量相似度** — 语义检测
4. **启发式规则** — 规则检测

### 检测层次
```
Layer 1: 启发式规则 — 快速过滤明显注入
Layer 2: 向量相似度 — 语义级别检测
Layer 3: LLM 分类器 — 深度检测
Layer 4: Canary Token — 泄露验证
```

### 优势
- ✅ 多层检测
- ✅ Canary Token 创新
- ✅ 低误报率

### 劣势
- ❌ 只防注入
- ❌ 无运行时治理
- ❌ 无合规功能

## 4.5 商业安全平台

### Lakera Guard
| 指标 | 数据 |
|------|------|
| 公司 | Lakera（瑞士） |
| 融资 | $20M Series A (2024) |
| 定价 | $0.001/次调用 |
| 核心 | 实时 Prompt Injection 防护 |

**优势：** 实时检测、低延迟、企业级
**劣势：** 闭源、按调用收费、只防注入

### Prompt Armor
| 指标 | 数据 |
|------|------|
| 公司 | Prompt Armor |
| 定位 | 企业级 Prompt 安全 |
| 核心 | 多层防护、合规报告 |

**优势：** 企业级、合规报告
**劣势：** 闭源、价格高

### Robust Intelligence（被 Cisco 收购）
| 指标 | 数据 |
|------|------|
| 公司 | Robust Intelligence |
| 收购 | Cisco 2024 |
| 定位 | AI 安全平台 |
| 核心 | 模型验证、运行时防护 |

**优势：** Cisco 背书、全栈安全
**劣势：** 通用 AI、不专注 Agent

### Arthur AI
| 指标 | 数据 |
|------|------|
| 公司 | Arthur AI |
| 融资 | $60M+ |
| 定位 | AI 可观测性 |
| 核心 | 模型监控、护栏 |

**优势：** 可观测性强
**劣势：** 不专注 Agent 行为

### Galileo
| 指标 | 数据 |
|------|------|
| 公司 | Galileo |
| 融资 | $45M+ |
| 定位 | LLM 可观测性 |
| 核心 | 幻觉检测、质量评估 |

**优势：** 幻觉检测创新
**劣势：** 不管合规

### WhyLabs
| 指标 | 数据 |
|------|------|
| 公司 | WhyLabs |
| 融资 | $30M+ |
| 定位 | AI 可观测性 |
| 核心 | 数据漂移、模型监控 |

**优势：** 数据漂移检测
**劣势：** 不专注 Agent

## 4.6 安全层竞品总结

### 对比矩阵

| 能力 | Guardrails | NeMo | LLM Guard | Rebuff | Lakera | AgentGuard |
|------|-----------|------|-----------|--------|--------|-----------|
| 输入过滤 | ⚠️ | ✅ | ✅ | ✅ | ✅ | ✅ |
| 输出校验 | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ |
| 行为审计 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 合规追踪 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 自主度控制 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 成本归因 | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ |
| 性能 | Python | Python | Python | Python | API | Rust |
| 开源 | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |

### 关键洞察

1. **安全工具只做"过滤"** — Guardrails/NeMo/LLM Guard 都是输入/输出过滤，不管 Agent 行为
2. **没有合规功能** — 没有一个安全工具做 GxP/FDA/EU AI Act 合规
3. **没有自主度控制** — 没有工具做 Suggest/Auto/Full 三模式
4. **没有成本归因** — 没有工具做每 Agent 每任务成本追踪
5. **Python 性能瓶颈** — 所有开源工具都是 Python 实现

**AgentGuard 差异化：** 唯一一个"过滤+审计+合规+自主度+成本"一体化方案，用 Rust 实现高性能。
