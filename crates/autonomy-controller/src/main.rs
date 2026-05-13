use kias_autonomy_controller::{AutonomyController, AutonomyLevel, ToolPolicy, ToolPermission};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("KIAS Autonomy Controller starting...");

    let mut controller = AutonomyController::new();

    // 设置策略
    controller.set_tool_policy(
        ToolPolicy::new("terminal", ToolPermission::RequireConfirmation)
            .with_sandbox(true)
            .with_network(false)
            .with_timeout(60),
    );

    controller.set_tool_policy(
        ToolPolicy::new("file_write", ToolPermission::AutoApprove)
            .with_sandbox(true),
    );

    // 测试不同自主级别
    println!("\n=== Suggest Mode ===");
    controller.set_level(AutonomyLevel::Suggest);
    println!("{:?}", controller.check_execution_allowed("terminal"));
    println!("{:?}", controller.check_execution_allowed("file_write"));

    println!("\n=== AutoEdit Mode ===");
    controller.set_level(AutonomyLevel::AutoEdit);
    println!("{:?}", controller.check_execution_allowed("terminal"));
    println!("{:?}", controller.check_execution_allowed("file_write"));

    println!("\n=== FullAuto Mode ===");
    controller.set_level(AutonomyLevel::FullAuto);
    println!("{:?}", controller.check_execution_allowed("terminal"));
    println!("{:?}", controller.check_execution_allowed("file_write"));

    Ok(())
}
