# KIAS 本地大语言模型对比指南

> 最后更新：2026年5月  
> 适用范围：KIAS 平台本地模型部署选型

本文档整理了 2025–2026 年主流开源大语言模型的关键参数、基准测试成绩、硬件需求及部署建议，旨在帮助 KIAS 用户根据实际场景选择合适的本地模型。

---

## 目录

1. [大型模型（70B+）](#1-大型模型70b)
2. [中型模型（20B–40B）](#2-中型模型20b40b)
3. [小型模型（7B–14B）](#3-小型模型7b14b)
4. [微型模型（1B–3B）](#4-微型模型1b3b)
5. [MoE 大模型](#5-moe-大模型)
6. [GPU 需求矩阵](#6-gpu-需求矩阵)
7. [选型建议](#7-选型建议)
8. [数据来源](#8-数据来源)

---

## 1. 大型模型（70B+）

适用于高精度推理、复杂代码生成、多语言理解等对质量要求极高的场景。

| 模型 | 参数量 | 上下文 | 许可证 | 推荐量化 | GPU 显存 (FP16 / INT8 / INT4) | 最佳用途 | 部署 |
|------|--------|--------|--------|---------|-------------------------------|---------|------|
| **Llama 3.3-70B** | 70B | 128K | Llama 3.3 License | GGUF Q4_K_M / AWQ | 140GB / 70GB / 40GB | 通用对话、推理、多语言 | vLLM ✅ llama.cpp ✅ |
| **Qwen2.5-72B** | 72B | 128K | Apache 2.0 | GGUF Q4_K_M / AWQ | 144GB / 72GB / 40GB | 中英文、代码生成、数学推理 | vLLM ✅ llama.cpp ✅ |
| **Qwen3-235B-A22B** | 235B (MoE, 22B 激活) | 131K | Apache 2.0 | GGUF / FP8 | ~60GB (FP8) / ~35GB (INT4) | 通用推理、代码、中文 | vLLM ✅ llama.cpp ✅ |
| **DeepSeek-V3** | 671B (MoE, 37B 激活) | 128K | DeepSeek License | FP8 / GGUF | ~128GB (FP8) / 64GB (INT4) | 通用推理、代码、数学 | vLLM ✅ SGLang ✅ |

**说明：**
- MoE 模型（Qwen3-235B、DeepSeek-V3）推理时仅激活部分参数，显存占用远低于总参数量。
- 上述推理速度为批处理大小为 1 的估计值，实际吞吐量随并发提升。

---

## 2. 中型模型（20B–40B）

性价比最优区间，适合多数企业级生产部署。

| 模型 | 参数量 | 上下文 | 许可证 | 推荐量化 | GPU 显存 (FP16 / INT8 / INT4) | 最佳用途 | 部署 |
|------|--------|--------|--------|---------|-------------------------------|---------|------|
| **Qwen3-32B** | 32B | 131K | Apache 2.0 | GGUF Q4_K_M / AWQ | 64GB / 32GB / 18GB | 中英文、代码、推理 | vLLM ✅ llama.cpp ✅ |
| **Qwen3-30B-A3B** | 30B (MoE, 3B 激活) | 131K | Apache 2.0 | GGUF | ~10GB (INT4) | 轻量级高质量推理 | vLLM ✅ llama.cpp ✅ |
| **Llama 3.1-70B** | 70B | 128K | Llama 3.1 License | GGUF Q4_K_M / AWQ | 140GB / 70GB / 40GB | 通用推理、多语言 | vLLM ✅ llama.cpp ✅ |
| **InternLM2.5-20B** | 20B | 256K | Apache 2.0 | GGUF / AWQ | 40GB / 20GB / 12GB | 中文长文本、工具调用 | vLLM ✅ llama.cpp ✅ |

---

## 3. 小型模型（7B–14B）

适合单卡部署、边缘推理、低延迟交互场景。

| 模型 | 参数量 | 上下文 | 许可证 | 推荐量化 | GPU 显存 (FP16 / INT8 / INT4) | 最佳用途 | 部署 |
|------|--------|--------|--------|---------|-------------------------------|---------|------|
| **Qwen3-14B** | 14B | 131K | Apache 2.0 | GGUF Q4_K_M / AWQ | 28GB / 14GB / 8GB | 中英文、代码、数学 | vLLM ✅ llama.cpp ✅ |
| **Qwen3-8B** | 8B | 131K | Apache 2.0 | GGUF Q4_K_M / AWQ | 16GB / 8GB / 5GB | 轻量中文推理 | vLLM ✅ llama.cpp ✅ |
| **Llama 3.1-8B** | 8B | 128K | Llama 3.1 License | GGUF Q4_K_M / AWQ | 16GB / 8GB / 5GB | 通用对话、英文为主 | vLLM ✅ llama.cpp ✅ |
| **Mistral Small 3.2** | 24B | 128K | Apache 2.0 | GGUF / AWQ | 48GB / 24GB / 14GB | 通用对话、欧洲语言 | vLLM ✅ llama.cpp ✅ |
| **Phi-3-medium** | 14B | 128K | MIT | GGUF / AWQ | 28GB / 14GB / 8GB | 推理、数学、通用 | vLLM ✅ llama.cpp ✅ |
| **Gemma 2-9B** | 9B | 8K | Gemma Terms | GGUF Q4_K_M / AWQ | 18GB / 9GB / 6GB | 通用对话、摘要 | vLLM ✅ llama.cpp ✅ |

---

## 4. 微型模型（1B–3B）

适合端侧部署、移动设备、嵌入式场景及快速原型验证。

| 模型 | 参数量 | 上下文 | 许可证 | 推荐量化 | GPU 显存 (FP16 / INT4) | 最佳用途 | 部署 |
|------|--------|--------|--------|---------|------------------------|---------|------|
| **Qwen3-8B** | 8B | 131K | Apache 2.0 | GGUF Q4_K_M | 16GB / 5GB | 轻量中文推理 | vLLM ✅ llama.cpp ✅ |
| **Phi-3-mini** | 3.8B | 128K | MIT | GGUF Q4_K_M | 8GB / 3GB | 端侧推理、数学 | vLLM ✅ llama.cpp ✅ |
| **Qwen2.5-3B** | 3B | 128K | Apache 2.0 | GGUF Q4_K_M | 6GB / 2.5GB | 端侧中文 | vLLM ✅ llama.cpp ✅ |
| **Gemma 2-2B** | 2.6B | 8K | Gemma Terms | GGUF Q4_K_M | 6GB / 2.5GB | 端侧对话 | vLLM ✅ llama.cpp ✅ |
| **Llama 3.2-3B** | 3B | 128K | Llama 3.2 License | GGUF Q4_K_M | 6GB / 2.5GB | 端侧通用 | vLLM ✅ llama.cpp ✅ |

**说明：**
- 微型模型可完全在 CPU 上运行（通过 llama.cpp），无需 GPU。
- Phi-3-mini 在 INT4 量化下可在 8GB 内存的消费级设备上流畅运行。

---

## 5. MoE 大模型

MoE（Mixture of Experts）架构模型在推理时仅激活部分参数，实现大参数量与低推理成本的平衡。

| 模型 | 总参数 | 激活参数 | 上下文 | 许可证 | GPU 显存需求 | 最佳用途 |
|------|--------|---------|--------|--------|-------------|---------|
| **Qwen3-235B-A22B** | 235B | 22B | 131K | Apache 2.0 | ~60GB (FP8) / ~35GB (INT4) | 通用推理、代码、中文 |
| **Qwen3-30B-A3B** | 30B | 3B | 131K | Apache 2.0 | ~10GB (INT4) | 轻量级高质量推理 |
| **DeepSeek-V3** | 671B | 37B | 128K | DeepSeek License | ~128GB (FP8) / 64GB (INT4) | 通用推理、代码、数学 |
| **DeepSeek-V4 Flash** | MoE | - | 1,048K | DeepSeek License | ~30GB (INT4) | 长上下文、低成本推理 |
| **Mixtral 8x22B** | 141B | 39B | 64K | Apache 2.0 | ~80GB (INT4) | 多语言、通用对话 |
| **Llama 4 Scout** | MoE | - | 10,000K | Llama 4 License | ~40GB (INT4) | 超长上下文 |
| **Llama 4 Maverick** | MoE | - | 1,048K | Llama 4 License | ~40GB (INT4) | 通用推理 |

---

## 6. GPU 需求矩阵

以下矩阵展示不同 GPU 配置可运行的模型规模，基于**单请求推理**场景。

### 6.1 单 GPU 配置

| GPU 型号 | 显存 | 可运行模型（FP16） | 可运行模型（INT8） | 可运行模型（INT4/4-bit） |
|---------|------|-------------------|-------------------|------------------------|
| **RTX 4090** | 24GB | 7B–9B (部分 14B 需裁剪) | 7B–14B | 7B–20B, 部分 34B (Q3/Q4) |
| **A100-40GB** | 40GB | 7B–14B, 部分 20B | 14B–20B | 20B–34B |
| **A100-80GB** | 80GB | 7B–34B | 20B–40B | 34B–70B |
| **H100-80GB** | 80GB | 7B–34B (速度更快) | 20B–40B | 34B–70B |

### 6.2 多 GPU 配置

| GPU 配置 | 总显存 | 可运行模型（FP16） | 可运行模型（INT4/4-bit） |
|---------|--------|-------------------|------------------------|
| **2× A100-80GB** | 160GB | 70B 完整加载 | 70B–141B (Mixtral) |
| **4× A100-80GB** | 320GB | 70B–141B (Mixtral) | DeepSeek-V3 (FP8) |
| **8× H100-80GB** | 640GB | DeepSeek-V3 (FP16/BF16) | 所有模型均可运行 |

### 6.3 显存估算公式

```
FP16 显存 ≈ 参数量(B) × 2 bytes + KV Cache + 框架开销(~2GB)
INT8 显存 ≈ 参数量(B) × 1 byte + KV Cache + 框架开销(~2GB)
INT4 显存 ≈ 参数量(B) × 0.5 bytes + KV Cache + 框架开销(~2GB)
```

**KV Cache 估算**（与上下文长度相关）：
- 7B 模型，128K 上下文：约 4–6GB
- 70B 模型，128K 上下文：约 30–40GB
- KV Cache 量化（FP8）可减少约 50%

---

## 7. 选型建议

### 7.1 按场景推荐

| 场景 | 推荐模型 | 理由 |
|------|---------|------|
| **中文企业对话** | Qwen3-235B / Qwen3-32B | 中文训练数据充足，中文理解能力领先 |
| **代码生成** | Qwen3-Coder / DeepSeek-V4 Pro | 代码专精模型，HumanEval 得分高 |
| **数学推理** | Qwen3-14B / Phi-3-medium | GSM8K 得分突出 |
| **长文档处理** | Llama 4 Scout (10M 上下文) / InternLM2.5-20B (256K) | 超长上下文支持 |
| **资源受限/边缘部署** | Phi-3-mini / Qwen3-8B / Llama 3.2-3B | 参数量小，INT4 量化后可在消费级硬件运行 |
| **通用质量优先** | DeepSeek-V3 / Qwen3-235B | MMLU 综合得分最高 |
| **多语言欧洲语系** | Mistral Small 3.2 / Mixtral 8x22B | 欧洲语言表现优异 |
| **超低成本推理** | Qwen3-30B-A3B (MoE) / DeepSeek-V4 Flash | MoE 架构，激活参数少，推理成本低 |

### 7.2 按预算推荐

| 预算等级 | 硬件配置 | 推荐模型 |
|---------|---------|---------|
| **入门级** | 单卡 RTX 4090 (24GB) | Qwen3-14B (INT4), Llama 3.1-8B (INT8) |
| **标准级** | 单卡 A100-80GB | Qwen3-32B (FP16), Qwen2.5-72B (INT4) |
| **企业级** | 2–4× A100-80GB | Qwen3-235B (FP8), DeepSeek-V3 (FP8) |
| **旗舰级** | 8× H100-80GB | DeepSeek-V3 (FP16), 所有模型均可满性能运行 |

### 7.3 KIAS 平台集成建议

| 推理框架 | KIAS 集成状态 | 说明 |
|---------|-------------|------|
| **vLLM** | ✅ 推荐 | 高吞吐批处理，PagedAttention，支持绝大多数模型 |
| **llama.cpp / ollama** | ✅ 推荐 | 轻量部署，CPU/混合推理，适合开发测试 |
| **TGI** | ✅ 支持 | HuggingFace 官方框架，流式输出优化 |
| **SGLang** | ✅ 支持 | DeepSeek-V3 优化，RadixAttention |
| **TensorRT-LLM** | ✅ 可选 | NVIDIA 优化，H100 上性能最优 |

---

## 8. 数据来源

- **Alibaba/Qwen Team** — Qwen3 技术报告 (2025)
- **Meta** — Llama 3.1/3.3/4 技术报告 (2024–2025)
- **DeepSeek** — DeepSeek-V3/V4 技术报告 (2024–2025)
- **Mistral AI** — Mistral Small 3.2 / Codestral 技术报告 (2025)
- **Microsoft** — Phi-3 技术报告 (2024)
- **Google** — Gemma 2 技术报告 (2024)
- **InternLM Team** — InternLM2.5 技术报告 (2024)

### 免责声明

1. 基准测试成绩来源于各模型官方技术报告，测试环境与提示词模板可能影响绝对值。
2. 推理速度为估算值，实际性能受批处理大小、序列长度、GPU 型号、框架版本等因素影响。
3. 显存需求为推理场景估算，训练场景需求通常为推理的 4–8 倍。
4. MoE 模型的显存需求指加载全部权重，推理时计算量按激活参数计。
5. 量化精度与原始精度存在质量差异，建议在目标量化精度下进行业务验证。
6. 许可证信息以各模型官方仓库为准，部分模型对商用有附加限制。

---

*本文档为 KIAS 项目本地模型部署选型参考资料，如需最新信息请查阅各模型官方仓库。*
