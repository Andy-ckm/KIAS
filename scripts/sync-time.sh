#!/bin/bash
# 系统时间同步脚本
# 每12小时运行一次，强制NTP同步并记录偏差

set -euo pipefail

LOG_FILE="/mnt/workspace/kias/logs/time-sync.log"
mkdir -p "$(dirname "$LOG_FILE")"

echo "========================================" >> "$LOG_FILE"
echo "[$(date)] 时间同步开始" >> "$LOG_FILE"

# 1. 强制 chrony 同步
chronyc makestep 0.1 1 >> "$LOG_FILE" 2>&1 || true
chronyc burst 2/3 >> "$LOG_FILE" 2>&1 || true

# 2. 记录同步后状态
echo "[$(date)] 同步后状态:" >> "$LOG_FILE"
chronyc tracking >> "$LOG_FILE" 2>&1

# 3. 提取关键信息
OFFSET=$(chronyc tracking 2>/dev/null | grep "System time" | awk '{print $4, $5, $6, $7}')
STRATUM=$(chronyc tracking 2>/dev/null | grep "Stratum" | awk '{print $3}')
echo "[$(date)] 偏差: $OFFSET | 层级: $STRATUM" >> "$LOG_FILE"
echo "[$(date)] 时间同步完成" >> "$LOG_FILE"
