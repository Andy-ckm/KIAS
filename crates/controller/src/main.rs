use chrono::Utc;
use kias_controller::{
    AgentConfig, AgentStatus, ControllerLoop, ControllerLoopConfig, ControllerState, DesiredState,
    HealthCheckConfig, ResourceRequirements,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting AgentGuard Controller with Runtime Loop");

    // ── Build initial state ──
    let state = Arc::new(Mutex::new(ControllerState {
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
    }));

    // ── Configure and run the controller loop ──
    let config = ControllerLoopConfig {
        runtime: kias_controller::RuntimeLoopConfig {
            max_rounds: 10,
            loop_timeout: std::time::Duration::from_secs(120),
            round_timeout: std::time::Duration::from_secs(30),
            quality_threshold: 0.95,
            stop_on_achieve: true,
            cooldown: std::time::Duration::from_millis(500),
        },
        health: HealthCheckConfig::default(),
    };

    let controller = ControllerLoop::new(config, state.clone()).await?;

    tracing::info!("Controller loop starting — execute→observe→adjust→re-execute");

    let metrics = controller.run().await?;

    // ── Report results ──
    println!("═══════════════════════════════════════════════════════");
    println!("  Controller Loop Results");
    println!("═══════════════════════════════════════════════════════");
    println!("  Status:           {:?}", metrics.status);
    println!("  Rounds executed:  {}", metrics.rounds_executed);
    println!(
        "  Achieved on:      {}",
        metrics
            .achieved_on_round
            .map(|r| r.to_string())
            .unwrap_or_else(|| "N/A".to_string())
    );
    println!(
        "  Total duration:   {:.2}s",
        metrics.total_duration.as_secs_f64()
    );
    println!("  Quality scores:   {:?}", metrics.quality_scores);
    println!(
        "  Feedback chain:   {} entries",
        metrics.feedback_chain.len()
    );
    println!("═══════════════════════════════════════════════════════");

    // Show final state
    let final_state = state.lock().await;
    println!(
        "  Final state:      {}/{} replicas running",
        final_state.actual.running_replicas, final_state.desired.replicas
    );
    println!("  Agents tracked:   {}", final_state.agents.len());

    tracing::info!("AgentGuard Controller finished");
    Ok(())
}
