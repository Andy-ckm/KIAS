# PRISM-VQ: 向量量化离散潜变量 × 金融先验
> 来源: RiskX + Hanyang 大学, 2026
> 状态: 已入队深度研究

## 核心创新
1. **VQ 信息瓶颈** — 512 个离散 Codebook，强制聚类过滤噪音
2. **两阶段解耦** — 空间学习(截面原型) + 时序学习(动态MoE)
3. **FiLM 先验注入** — 13 个 JKP 因子作为条件锚点
4. **离散 Code 路由 MoE** — Code 决定调用哪个专家，非连续 gating

## 关键数据
- CSI 300: RankIC 0.0646, Sharpe 1.57 (费后)
- S&P 500: RankIC 0.0141, Sharpe 0.67 (费后)
- 去掉 VQ 后 S&P 500 RankIC 变负 (-0.0024)

## AgentGuard 映射
| PRISM-VQ | AgentGuard |
|----------|------|
| VQ Codebook (512 codes) | Agent 状态离散化原型 |
| 两阶段解耦 | Agent 画像 → 调度决策 |
| FiLM 先验注入 | Agent 能力标签硬约束 |
| MoE 离散路由 | 任务→Agent 匹配机制 |
| Contrastive Learning | Agent 能力区分度训练 |

## 复现陷阱
1. VQ 死码问题 — EMA 衰减率 + 定期重启
2. 辅助 Loss 梯度统治 — 需验证 VQ 本身是否立得住
3. JKP 因子依赖 — 因子质量直接决定 Codebook 质量
4. 低持续性(月度留码率仅 4.9%) — 换手率天生高
