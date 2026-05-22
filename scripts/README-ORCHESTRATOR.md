# MiniMax + mimo 正循环系统

## 架构图
```
┌─────────────────────────────────────────────────────────────┐
│                         零(用户)                              │
│                           │                                  │
│                    ┌──────▼──────┐                           │
│                    │   MiniMax   │                           │
│                    │  (前台协调)  │                           │
│                    └──────┬──────┘                           │
│                           │                                  │
│         ┌─────────────────┼─────────────────┐               │
│         │                 │                 │               │
│         ▼                 ▼                 ▼               │
│    ┌─────────┐      ┌─────────┐      ┌─────────┐           │
│    │  监控   │      │  审核   │      │  分析   │           │
│    │(30min) │      │ (1hour) │      │ (按需)  │           │
│    └────┬────┘      └────┬────┘      └────┬────┘           │
│         │                 │                 │               │
│         └─────────────────┼─────────────────┘               │
│                           │                                  │
│                    ┌──────▼──────┐                           │
│                    │    mimo     │                           │
│                    │  (后台开发)  │                           │
│                    └──────┬──────┘                           │
│                           │                                  │
│                    ┌──────▼──────┐                           │
│                    │   验证汇报   │                           │
│                    └─────────────┘                           │
└─────────────────────────────────────────────────────────────┘
```

## 文件说明
- `scripts/minimax-orchestrator.py` - 主协调器（Python）
- `scripts/minimax-loop.sh` - Shell循环脚本
- `scripts/minimax-review.py` - 代码审核脚本
- `logs/orchestrator/` - 日志和报告目录

## 使用方式
```bash
# 单次运行
python3 scripts/minimax-orchestrator.py

# 持续循环（30分钟间隔）
python3 scripts/minimax-orchestrator.py --loop

# 处理指定任务
python3 scripts/minimax-orchestrator.py --task "实现Harness可视化"

# Shell版本
bash scripts/minimax-loop.sh           # 单次
bash scripts/minimax-loop.sh --loop    # 循环
bash scripts/minimax-loop.sh --review  # 仅审核
bash scripts/minimax-loop.sh --monitor # 仅监控
```

## Cron任务
- 每30分钟：自动监控（编译/测试/clippy/unwrap/git）
- 每1小时：自动审核未提交代码

## 状态文件
- `.orchestrator-state.json` - 运行状态
- `logs/orchestrator/report-*.md` - 运营报告
- `logs/orchestrator/status.json` - 当前状态
