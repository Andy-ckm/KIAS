use kias_goal_engine::{DefaultEvaluator, Goal, GoalLoopRunner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS Goal Engine");

    // 创建目标（借鉴 Claude Code /goal）
    let mut goal = Goal::new("test/auth 下所有测试通过，lint 干净");

    // 添加条件（好目标三要素）
    goal.add_condition("tests_pass", "所有测试通过", "npm test", "exit code 0");

    goal.add_condition("lint_clean", "lint 干净", "npm run lint", "no errors");

    // 添加约束
    goal.add_constraint("no_break", "不修改其他测试文件", "git diff");

    // 设置最大轮数
    goal.set_max_rounds(20);

    // 创建评估器（裁判分离）
    let evaluator = Box::new(DefaultEvaluator::new());

    // 创建循环运行器
    let runner = GoalLoopRunner::with_default_executor(evaluator);

    // 运行训练循环（model.fit() = /goal）
    let result = runner.run(goal).await?;

    println!("Goal status: {:?}", result.status);
    println!("Rounds: {}", result.current_round);

    tracing::info!("KIAS Goal Engine finished");
    Ok(())
}
