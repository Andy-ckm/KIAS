use kias_agent_view::{AgentView, Session};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    println!("AgentGuard Agent View starting...");

    // Create agent view
    let mut view = AgentView::new("agent-1");

    // Create sessions
    let session1 = Session::new("session-1", "agent-1");
    let session2 = Session::new("session-2", "agent-1");
    let session3 = Session::new("session-3", "agent-1");

    view.add_session(session1);
    view.add_session(session2);
    view.add_session(session3);

    // Display summary
    view.display_summary();

    Ok(())
}
