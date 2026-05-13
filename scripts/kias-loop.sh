#!/bin/bash
# KIAS 循环开发自动触发脚本
# 每20分钟运行一次，发送开发指令给 LLM
# 用法: ./kias-loop.sh

set -euo pipefail

WORKSPACE="/mnt/workspace/kias"
LOG_DIR="$WORKSPACE/logs"
LOG_FILE="$LOG_DIR/kias-loop-$(date +%Y%m%d).log"
LOCK_FILE="/tmp/kias-loop.lock"

mkdir -p "$LOG_DIR"

# 防止重复运行
if [ -f "$LOCK_FILE" ]; then
    pid=$(cat "$LOCK_FILE" 2>/dev/null)
    if kill -0 "$pid" 2>/dev/null; then
        echo "[$(date)] Already running (pid=$pid), skipping" >> "$LOG_FILE"
        exit 0
    fi
fi
echo $$ > "$LOCK_FILE"
trap "rm -f $LOCK_FILE" EXIT

echo "========================================" >> "$LOG_FILE"
echo "[$(date)] KIAS 循环开发触发" >> "$LOG_FILE"
echo "========================================" >> "$LOG_FILE"

# 记录当前项目状态
cd "$WORKSPACE"
echo "[$(date)] Git status:" >> "$LOG_FILE"
git status --short >> "$LOG_FILE" 2>&1 || true

echo "[$(date)] Last commit:" >> "$LOG_FILE"  
git log --oneline -1 >> "$LOG_FILE" 2>&1 || true

echo "[$(date)] Crates:" >> "$LOG_FILE"
ls crates/ >> "$LOG_FILE" 2>&1

echo "[$(date)] Trigger complete" >> "$LOG_FILE"
