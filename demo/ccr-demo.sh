#!/usr/bin/env bash
# KIAS CCR Demo — 变更控制记录完整闭环
# 演示：变更请求 → 审批流 → 副作用预演 → 电子签名 → 审计报告
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  KIAS CCR Demo — AI Agent 合规免疫系统                      ║${NC}"
echo -e "${BLUE}║  场景：制药企业 GxP 系统变更控制                             ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Step 1: 提交变更请求
echo -e "${YELLOW}[Step 1/6] 提交变更请求${NC}"
echo "  变更编号: CCR-2026-0518-001"
echo "  变更类型: 配置变更"
echo "  变更描述: 更新 LIMS 系统的样品检测阈值参数"
echo "  影响范围: 生产环境 / LIMS / QC 部门"
echo "  风险等级: 高（影响质量检测结果）"
echo -e "  ${GREEN}✓ 变更请求已创建，进入审批流${NC}"
echo ""

# Step 2: 审批流
echo -e "${YELLOW}[Step 2/6] 审批流（9 状态机）${NC}"
echo "  Draft → Reviewing → Approved → Published"
echo "  审批人 1: QA 主管 (zhang.qa@pharma.com)"
echo "  审批人 2: IT 主管 (li.it@pharma.com)"
echo "  审批人 3: 质量总监 (wang.quality@pharma.com)"
echo -e "  ${GREEN}✓ 3/3 审批人已批准${NC}"
echo ""

# Step 3: 电子签名
echo -e "${YELLOW}[Step 3/6] 电子签名（21 CFR Part 11 §11.50）${NC}"
echo "  签名方式: TOTP 二因素认证"
echo "  签名含义: '本人已审阅变更内容，批准执行'"
echo "  签名哈希: sha256:a3f2b8c1d4e5..."
echo -e "  ${GREEN}✓ 电子签名已记录，不可篡改${NC}"
echo ""

# Step 4: 副作用预演
echo -e "${YELLOW}[Step 4/6] 副作用预演（Dry-Run）${NC}"
echo "  预演操作: 修改 LIMS 配置文件 /etc/lims/thresholds.yaml"
echo "  变更差异:"
echo "    - 检测阈值: 0.05 → 0.03"
echo "    - 影响样品数: 预计 1,200 个/月"
echo "    - 可逆性: 是（配置文件可回滚）"
echo "  风险评估: HIGH（影响质量检测结果）"
echo -e "  ${GREEN}✓ 预演完成，未发现冲突${NC}"
echo ""

# Step 5: 执行变更
echo -e "${YELLOW}[Step 5/6] 执行变更${NC}"
echo "  执行时间: 2026-05-18 08:30:00 UTC"
echo "  执行人: system-agent (KIAS auto-loop)"
echo "  执行结果: 配置文件已更新，LIMS 服务已重载"
echo "  验证结果: 新阈值生效，样品检测正常"
echo -e "  ${GREEN}✓ 变更执行成功，验证通过${NC}"
echo ""

# Step 6: 审计报告
echo -e "${YELLOW}[Step 6/6] 审计报告（SHA-256 哈希链）${NC}"
echo "  ┌─────────────────────────────────────────────────┐"
echo "  │ 审计链摘要                                       │"
echo "  │                                                  │"
echo "  │ 序列号: 001 → 002 → 003 → 004 → 005 → 006     │"
echo "  │ 哈希链: sha256:e3b0c44298fc... → sha256:a3f2... │"
echo "  │                                                  │"
echo "  │ 事件 001: 变更请求创建 (zhang.qa)               │"
echo "  │ 事件 002: 审批通过 (li.it)                       │"
echo "  │ 事件 003: 审批通过 (wang.quality)                │"
echo "  │ 事件 004: 电子签名 (zhang.qa, TOTP)             │"
echo "  │ 事件 005: 副作用预演通过                         │"
echo "  │ 事件 006: 变更执行 + 验证通过                    │"
echo "  │                                                  │"
echo "  │ 完整性: ✓ 哈希链完整，无篡改                    │"
echo "  │ 合规标准: 21 CFR Part 11 / EU Annex 11 / ALCOA+ │"
echo "  └─────────────────────────────────────────────────┘"
echo ""

echo -e "${GREEN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Demo 完成                                                  ║${NC}"
echo -e "${GREEN}║  全程: 0 人工审计日志整理，100% 自动生成合规报告            ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════╝${NC}"
