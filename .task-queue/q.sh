#!/bin/bash
# 快速入队任务
# 用法: ./q.sh <类型> <描述> [优先级]
# 类型: dev, fix, research, review
# 优先级: 1(最高) - 10(最低), 默认5

TYPE="${1:-dev}"
DESC="${2:-empty task}"
PRIORITY="${3:-5}"

cd /workspace/kias
python3 .task-queue/queue.py enqueue "$TYPE" "{\"task\":\"$DESC\",\"priority\":$PRIORITY}"
echo "队列状态:"
python3 .task-queue/queue.py stats
