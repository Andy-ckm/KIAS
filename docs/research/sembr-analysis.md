# Sembr: 意图驱动的自部署雷达
> 来源: Peakstone Labs, 2026-05
> GitHub: github.com/Peakstone-Labs/sembr
> 状态: 已入队深度研究（优先级2）

## 核心架构
六步闭环：Collector → Embedder(BGE-M3) → Qdrant → Matcher(反向匹配) → Summarizer(LLM) → Notifier

## Agent-first 三层设计
1. **INSTALL.md** — 写给 Agent 看的 6 阶段安装指南，Agent 自主部署
2. **Skills 包** — 5 文件标准格式（SKILL.md + endpoints + schemas + recipes + errors）
3. **/fire API** — 同步端点，Agent 一次 HTTP 调用获得匹配+摘要

## 竞品对比
| 维度 | Feedly | Inoreader | Bloomberg | Perplexity | Sembr |
|------|--------|-----------|-----------|------------|-------|
| 语义匹配 | ✅ | ❌ | ✅ | ✅ | ✅ |
| 双语中英 | ⚠️ | ⚠️ | ✅ | ⚠️ | ✅ |
| 自定义源 | ⚠️ | ✅ | ❌ | ❌ | ✅ |
| 自部署 | ❌ | ❌ | ❌ | ❌ | ✅ |
| 独立分析 | ⚠️ | ⚠️ | ❌ | ⚠️ | ✅ |

## KIAS 映射
| Sembr | KIAS | 行动 |
|-------|------|------|
| 意图语义匹配 | SkillMatcher | 升级为向量匹配 |
| BGE-M3 免费嵌入 | knowledge 层 | 接入免费嵌入模型 |
| /fire 同步端点 | A2A handler | 统一 fire API |
| INSTALL.md 自部署 | kias-cli | 补 Agent 自部署文档 |
| Skills 包格式 | SkillRegistry | 对齐 Sembr 5 文件标准 |

## 成本结构启示
- 嵌入层免费（SiliconFlow BGE-M3）
- LLM 只在命中后触发（DeepSeek-V4-Flash）
- 10 意图 × 24 轮 × 365 天 ≈ 8.8 万次，但嵌入免费+LLM按需=极低成本
- 对比 Perplexity 每次查询都收费

## KIAS 待落地
1. SkillMatcher 向量化 — 用 HNSW 做意图→技能语义匹配
2. A2A /fire 端点 — Agent 标准化调用接口
3. Agent 自部署文档 — INSTALL.md for KIAS
4. 免费嵌入层接入 — SiliconFlow/Ollama BGE-M3
