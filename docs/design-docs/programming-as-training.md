# KIAS 核心理念：编程即训练

> 参考来源：Claude Code /goal、François Chollet
> 核心洞察：足够先进的 AI 编程，本质上就是机器学习

## 核心公式

```
model.fit() = /goal
```

## 对应关系

| 机器学习 | AI 编程 | KIAS 实现 |
|----------|---------|-----------|
| Loss Function | 需求文档 | Goal 定义 |
| 验证集 | 测试用例 | GoalCondition |
| Training Step | Agent 迭代 | Round |
| 模型权重 | 代码库 | 最终输出 |
| 优化目标 | /goal 描述 | Goal.description |
| 搜索空间 | 约束条件 | Constraint |

## 关键问题（ML 领域的坑）

### 1. 过拟合
**问题**：Agent 找到捷径，测试全过但逻辑歪了
**解决**：
- 约束条件要严格
- 验证标准要全面
- 多角度测试

### 2. 数据泄露
**问题**：验证信息泄露到训练过程
**解决**：
- 裁判分离（Worker 和 Verifier 独立）
- 评估模型独立于工作模型

### 3. 概念漂移
**问题**：目标在迭代过程中发生变化
**解决**：
- 目标跨会话保持
- 目标状态面板实时显示
- 明确的停止条件

## KIAS 设计原则

### 1. 优化目标定对了吗？
- 好目标三要素：可衡量的终态、验证方式、约束
- 目标条件最长 4000 字符
- 可以写运行时长限制

### 2. 验证标准够不够严？
- 独立评估模型（裁判分离）
- 多轮对抗验证（Worker-Verifier）
- 自动化测试

### 3. 约束条件会不会让 Agent 找到捷径？
- 明确的约束定义
- 约束检查机制
- 人工审核高风险变更

## 创新点

### 1. 训练循环自动化
```rust
// 定义优化目标
let goal = Goal::new("test/auth 下所有测试通过，lint 干净");

// 定义验证标准
goal.add_condition("tests_pass", "所有测试通过", "npm test", "exit code 0");
goal.add_condition("lint_clean", "lint 干净", "npm run lint", "no errors");

// 定义约束
goal.add_constraint("no_break", "不修改其他测试文件", "git diff");

// 运行训练循环
let result = goal_loop.run(goal).await?;
```

### 2. 过拟合检测
```rust
// 检测 Agent 是否找到捷径
if result.逻辑正确但实现方式可疑() {
    // 触发人工审核
    escalate_to_human();
}
```

### 3. 概念漂移检测
```rust
// 检测目标是否在迭代过程中变化
if goal.description != original_description {
    // 提醒用户目标已变化
    notify_user();
}
```

## 实现计划

### Phase 1：基础训练循环
- [ ] 实现 Goal 定义
- [ ] 实现 GoalCondition
- [ ] 实现 Constraint
- [ ] 实现 GoalLoopRunner

### Phase 2：裁判分离
- [ ] 实现独立评估模型
- [ ] 实现理由反馈
- [ ] 实现多轮对抗

### Phase 3：过拟合检测
- [ ] 实现捷径检测
- [ ] 实现逻辑正确性验证
- [ ] 实现人工审核机制

### Phase 4：概念漂移检测
- [ ] 实现目标变化检测
- [ ] 实现用户通知
- [ ] 实现目标版本管理

---

## 总结

> 真正要练的本事，已经不是写代码了。
> 是想清楚自己到底要什么，然后，定义好验收标准。
> 剩下的，交给训练循环。

KIAS 的目标就是把这个「训练循环」正式自动化。
