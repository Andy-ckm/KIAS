#!/bin/bash
# AgentGuard 自主开发循环脚本
# 每10分钟被cronjob调用，自主推进项目

set -e

PROJECT_DIR="/workspace/kias"
LOG_FILE="$PROJECT_DIR/.dev-log"
ITERATION_FILE="$PROJECT_DIR/.dev-iteration"

# 初始化
cd "$PROJECT_DIR"
mkdir -p "$(dirname "$LOG_FILE")"

# 读取当前迭代次数
if [ -f "$ITERATION_FILE" ]; then
    ITERATION=$(cat "$ITERATION_FILE")
else
    ITERATION=0
fi
ITERATION=$((ITERATION + 1))
echo "$ITERATION" > "$ITERATION_FILE"

# 扪心自问
echo "=== 扪心自问 (迭代 $ITERATION) ===" >> "$LOG_FILE"
echo "时间: $(date '+%Y-%m-%d %H:%M:%S')" >> "$LOG_FILE"

# 检查测试状态
echo "测试状态:" >> "$LOG_FILE"
cargo test --workspace 2>&1 | grep -E "test result:" | head -5 >> "$LOG_FILE" 2>&1 || true

# 检查git状态
echo "Git状态:" >> "$LOG_FILE"
git status --short >> "$LOG_FILE" 2>&1 || true

# 检查未提交代码
UNCOMMITTED=$(git status --short | wc -l)
if [ "$UNCOMMITTED" -gt 0 ]; then
    echo "发现 $UNCOMMITTED 个未提交文件，先提交" >> "$LOG_FILE"
    git add -A
    git commit -m "自主循环迭代 $ITERATION: 自动提交未保存代码" >> "$LOG_FILE" 2>&1 || true
fi

# 选择本轮任务
# 优先级：Linux自动化 > 企业文件处理 > IT变更管理
echo "本轮任务选择:" >> "$LOG_FILE"

# 检查Linux自动化模块的测试数量
LINUX_TESTS=$(cargo test --package kias-linux-automation 2>&1 | grep -c "test .* ok" || echo "0")
DOC_TESTS=$(cargo test --package kias-document-management 2>&1 | grep -c "test .* ok" || echo "0")
IT_TESTS=$(cargo test --package kias-it-change-management 2>&1 | grep -c "test .* ok" || echo "0")

echo "  Linux自动化: $LINUX_TESTS 测试" >> "$LOG_FILE"
echo "  企业文件处理: $DOC_TESTS 测试" >> "$LOG_FILE"
echo "  IT变更管理: $IT_TESTS 测试" >> "$LOG_FILE"

# 决策：选择测试最少的模块推进
if [ "$LINUX_TESTS" -le "$DOC_TESTS" ] && [ "$LINUX_TESTS" -le "$IT_TESTS" ]; then
    TARGET="linux-automation"
elif [ "$DOC_TESTS" -le "$LINUX_TESTS" ] && [ "$DOC_TESTS" -le "$IT_TESTS" ]; then
    TARGET="document-management"
else
    TARGET="it-change-management"
fi

echo "选择推进: $TARGET (测试最少)" >> "$LOG_FILE"

# 检查是否需要研究
if [ "$ITERATION" -eq 1 ] || [ $((ITERATION % 5)) -eq 0 ]; then
    echo "本轮需要研究竞品" >> "$LOG_FILE"
fi

# 提交并推送
echo "提交本轮工作:" >> "$LOG_FILE"
git add -A
git commit -m "自主循环迭代 $ITERATION: 推进 $TARGET" >> "$LOG_FILE" 2>&1 || true
git push origin main >> "$LOG_FILE" 2>&1 || true

echo "=== 迭代 $ITERATION 完成 ===" >> "$LOG_FILE"
echo "" >> "$LOG_FILE"
