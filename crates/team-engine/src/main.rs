use kias_team_engine::{Team, owner::DefaultOwner, worker::{CodeWorker, ResearchWorker}, verifier::{CodeVerifier, ResearchVerifier}};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    tracing::info!("Starting KIAS Team Engine");
    
    // 创建 Team（Owner-Worker-Verifier 架构）
    let owner = Box::new(DefaultOwner::new());
    let mut team = Team::new(owner);
    
    // 添加 Worker
    team.add_worker(Box::new(CodeWorker::new("code-worker-1")));
    team.add_worker(Box::new(ResearchWorker::new("research-worker-1")));
    
    // 添加 Verifier
    team.add_verifier(Box::new(CodeVerifier::new("code-verifier-1")));
    team.add_verifier(Box::new(ResearchVerifier::new("research-verifier-1")));
    
    // 执行任务
    let result = team.execute("写一个 Hello World 程序").await?;
    
    println!("Result: {}", result);
    
    tracing::info!("KIAS Team Engine finished");
    Ok(())
}
