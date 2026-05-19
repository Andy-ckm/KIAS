use async_trait::async_trait;
use kias_common::KiasResult;
use kias_skills::{Skill, SkillRegistry};

struct GreetSkill;

#[async_trait]
impl Skill for GreetSkill {
    fn name(&self) -> &str {
        "greet"
    }

    fn description(&self) -> &str {
        "A simple greeting skill"
    }

    async fn execute(&self, params: serde_json::Value) -> KiasResult<serde_json::Value> {
        let name = params["name"].as_str().unwrap_or("World");
        Ok(serde_json::json!({"greeting": format!("Hello, {}!", name)}))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting AgentGuard Skills Service");

    let mut registry = SkillRegistry::new();
    registry.register(Box::new(GreetSkill));

    println!("Registered skills: {:?}", registry.list_skills());

    if let Some(skill) = registry.get("greet") {
        let result = skill.execute(serde_json::json!({"name": "AgentGuard"})).await?;
        println!("Skill result: {}", result);
    }

    tracing::info!("AgentGuard Skills Service finished");
    Ok(())
}
