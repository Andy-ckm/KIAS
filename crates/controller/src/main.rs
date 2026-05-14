use chrono::Utc;
use kias_controller::{
    AgentConfig, AgentInfo, AgentStatus, ControllerState, DefaultReconciler, DesiredState,
    HealthCheckConfig, HealthChecker, NoOpSpawner, Reconciler, ResourceRequirements,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting KIAS Controller");

    let reconciler = DefaultReconciler::new(NoOpSpawner);

    let mut state = ControllerState {
        desired: DesiredState {
            replicas: 3,
            agent_config: AgentConfig {
                name: "worker-agent".to_string(),
                image: "kias/agent:latest".to_string(),
                resources: ResourceRequirements {
                    cpu: "100m".to_string(),
                    memory: "128Mi".to_string(),
                },
            },
        },
        actual: kias_controller::ActualState {
            running_replicas: 0,
            agent_status: AgentStatus::Pending,
            last_updated: Utc::now(),
        },
        agents: HashMap::new(),
    };

    // Initialize the health checker with default configuration.
    let health_config = HealthCheckConfig::default();
    let mut health_checker = HealthChecker::new(health_config.clone());

    tracing::info!(
        heartbeat_timeout_secs = health_config.heartbeat.timeout_secs,
        max_recovery_retries = health_config.recovery.max_retries,
        check_interval_ms = health_config.check_interval_ms,
        "Health checker configured"
    );

    // Simulate registering agents.
    for i in 1..=state.desired.replicas {
        let agent_id = format!("agent-{i}");
        let mut agent = AgentInfo::new(
            &agent_id,
            format!("{}-{i}", state.desired.agent_config.name),
        );
        agent.status = AgentStatus::Running;
        state.agents.insert(agent_id.clone(), agent);
        health_checker.register_agent(&agent_id);
    }

    // Execute reconciliation.
    reconciler.reconcile(&mut state).await?;

    println!(
        "Controller state reconciled: {} replicas running",
        state.actual.running_replicas
    );

    // Run a health check cycle.
    let summary = health_checker.check(&mut state);

    tracing::info!(
        agents_checked = summary.agents_checked,
        alive = summary.alive,
        timed_out = summary.timed_out,
        restarted = summary.restarted,
        permanently_failed = summary.permanently_failed,
        "Health check summary"
    );

    println!(
        "Health check: {} agents checked, {} alive, {} timed out, {} restarted",
        summary.agents_checked, summary.alive, summary.timed_out, summary.restarted
    );

    tracing::info!("KIAS Controller finished");
    Ok(())
}
