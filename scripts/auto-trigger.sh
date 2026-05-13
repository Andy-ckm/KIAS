#!/bin/bash
# KIAS 循环开发自动触发脚本
# 每10分钟运行一次

WORKSPACE="/mnt/workspace/kias"
LOG_FILE="$WORKSPACE/logs/auto-trigger.log"

mkdir -p "$WORKSPACE/logs"

echo "=== $(date) ===" >> "$LOG_FILE"
echo "KIAS 循环开发自动触发" >> "$LOG_FILE"
echo "开发完成就迭代，按着验收标准，去测试，修改。完了就继续创新，然后再开发，这才叫循环开发。" >> "$LOG_FILE"
echo "---" >> "$LOG_FILE"
