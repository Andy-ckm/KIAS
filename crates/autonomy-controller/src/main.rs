use kias_autonomy_controller::{AutonomyController, AutonomyLevel, ToolPermission, ToolPolicy};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("AgentGuard Autonomy Controller starting...");

    let mut controller = AutonomyController::new();

    // 设置策略
    controller.set_tool_policy(
        ToolPolicy::new("terminal", ToolPermission::RequireConfirmation)
            .with_sandbox(true)
            .with_network(false)
            .with_timeout(60),
    );

    controller.set_tool_policy(
        ToolPolicy::new("file_write", ToolPermission::AutoApprove).with_sandbox(true),
    );

    // 测试不同自主级别
    tracing::info!("=== Suggest Mode ===");
    controller.set_level(AutonomyLevel::Suggest);
    tracing::info!("{:?}", controller.check_execution_allowed("terminal"));
    tracing::info!("{:?}", controller.check_execution_allowed("file_write"));

    tracing::info!("=== AutoEdit Mode ===");
    controller.set_level(AutonomyLevel::AutoEdit);
    tracing::info!("{:?}", controller.check_execution_allowed("terminal"));
    tracing::info!("{:?}", controller.check_execution_allowed("file_write"));

    tracing::info!("=== FullAuto Mode ===");
    controller.set_level(AutonomyLevel::FullAuto);
    tracing::info!("{:?}", controller.check_execution_allowed("terminal"));
    tracing::info!("{:?}", controller.check_execution_allowed("file_write"));

    Ok(())
}
