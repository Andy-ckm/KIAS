//! 代码自动生成 — KIAS自循环的核心
//!
//! 自动生成修复代码，包括：
//! - 代码补丁生成
//! - 配置文件生成
//! - 测试代码生成
//! - 文档生成

use serde::{Deserialize, Serialize};

use crate::planner::GeneratedPlan;

/// 生成的代码补丁
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodePatch {
    /// 补丁ID
    pub id: String,
    /// 目标文件
    pub target_file: String,
    /// 补丁类型
    pub patch_type: PatchType,
    /// 补丁内容
    pub content: String,
    /// 补丁描述
    pub description: String,
    /// 生成时间
    pub generated_at: chrono::DateTime<chrono::Utc>,
}

/// 补丁类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchType {
    /// 代码修改
    CodeChange,
    /// 配置修改
    ConfigChange,
    /// 测试添加
    TestAddition,
    /// 文档更新
    DocumentationUpdate,
}

/// 代码生成器 trait
pub trait CodeGenerator: Send + Sync {
    /// 生成代码补丁
    fn generate(&self, plan: &GeneratedPlan) -> Vec<CodePatch>;

    /// 获取生成器名称
    fn name(&self) -> &str;
}

/// 持久化代码生成器
pub struct PersistenceCodeGenerator;

impl Default for PersistenceCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PersistenceCodeGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl CodeGenerator for PersistenceCodeGenerator {
    #[allow(clippy::vec_init_then_push)]
    fn generate(&self, _plan: &GeneratedPlan) -> Vec<CodePatch> {
        let mut patches = Vec::new();

        // 生成AppState修改补丁
        patches.push(CodePatch {
            id: uuid::Uuid::new_v4().to_string(),
            target_file: "crates/api-server/src/lib.rs".to_string(),
            patch_type: PatchType::CodeChange,
            content: r#"/// Shared application state passed to all handlers
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<KiasConfig>,
    /// Agent持久化存储（SQLite）
    pub agent_repository: Option<Arc<kias_data_store::AgentRepository>>,
    /// Agent内存缓存（启动时从SQLite加载）
    pub agents: Arc<RwLock<std::collections::HashMap<String, models::agent::Agent>>>,
    pub nodes: Arc<RwLock<std::collections::HashMap<String, models::node::Node>>>,
    pub workflows: Arc<RwLock<std::collections::HashMap<String, handlers::workflows::Workflow>>>,
    pub audit_log: Arc<MemoryAuditLog>,
    /// SQLite-backed audit log for production persistence (optional)
    pub sqlite_audit_log: Option<Arc<kias_data_store::SqliteAuditLog>>,
    /// Dead letter queue for failed tasks (optional)
    pub dead_letter_queue: Option<Arc<kias_data_store::DeadLetterQueue>>,
    pub event_bus: EventBus,
    pub a2a_tasks: handlers::a2a::A2aTaskStore,
    /// Tracks active WebSocket connections and metrics.
    pub connection_registry: ConnectionRegistry,
    /// Ring buffer for event replay to new WebSocket clients.
    pub event_replay_buffer: EventReplayBuffer,
    /// Knowledge base retriever (vector search + hybrid retrieval)
    pub knowledge_retriever: Arc<dyn Retriever>,
}"#
            .to_string(),
            description: "添加agent_repository字段到AppState".to_string(),
            generated_at: chrono::Utc::now(),
        });

        // 生成初始化方法修改补丁
        patches.push(CodePatch {
            id: uuid::Uuid::new_v4().to_string(),
            target_file: "crates/api-server/src/lib.rs".to_string(),
            patch_type: PatchType::CodeChange,
            content: r#"        Self {
            config: Arc::new(config),
            agent_repository: None, // Will be set up if SQLite is configured
            agents: Arc::new(RwLock::new(std::collections::HashMap::new())),
            nodes: Arc::new(RwLock::new(nodes)),
            workflows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            audit_log: Arc::new(MemoryAuditLog::new()),
            sqlite_audit_log: None,  // Will be set up if SQLite is configured
            dead_letter_queue: None, // Will be set up if SQLite is configured
            event_bus: EventBus::default(),
            a2a_tasks: handlers::a2a::A2aTaskStore::new(),
            connection_registry: ConnectionRegistry::default(),
            event_replay_buffer: EventReplayBuffer::default(),
            knowledge_retriever: Arc::new(knowledge_retriever),
        }"#
            .to_string(),
            description: "初始化agent_repository字段".to_string(),
            generated_at: chrono::Utc::now(),
        });

        patches
    }

    fn name(&self) -> &str {
        "PersistenceCodeGenerator"
    }
}

/// 配置修复代码生成器
pub struct ConfigFixCodeGenerator;

impl Default for ConfigFixCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigFixCodeGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl CodeGenerator for ConfigFixCodeGenerator {
    #[allow(clippy::vec_init_then_push)]
    fn generate(&self, _plan: &GeneratedPlan) -> Vec<CodePatch> {
        let mut patches = Vec::new();

        // 生成配置文件修改补丁
        patches.push(CodePatch {
            id: uuid::Uuid::new_v4().to_string(),
            target_file: "config/kias.toml".to_string(),
            patch_type: PatchType::ConfigChange,
            content: r#"[api_server]
host = "0.0.0.0"
port = 8080

[model]
provider = "openai"
model = "gpt-4o"
api_key = "sk-00000000000000000000000000000000"
# base_url = "https://api.openai.com/v1"

[database]
path = "kias.db"

[logging]
level = "info"
format = "text"
"#
            .to_string(),
            description: "修复配置文件，使用placeholder API key".to_string(),
            generated_at: chrono::Utc::now(),
        });

        patches
    }

    fn name(&self) -> &str {
        "ConfigFixCodeGenerator"
    }
}

/// 代码生成器管理器
pub struct CodeGeneratorManager {
    /// 生成器列表
    generators: Vec<Box<dyn CodeGenerator>>,
    /// 生成历史
    history: Vec<CodePatch>,
}

impl Default for CodeGeneratorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeGeneratorManager {
    pub fn new() -> Self {
        Self {
            generators: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 注册生成器
    pub fn register_generator(&mut self, generator: Box<dyn CodeGenerator>) {
        self.generators.push(generator);
    }

    /// 生成代码补丁
    pub fn generate_patches(&mut self, plan: &GeneratedPlan) -> Vec<CodePatch> {
        let mut patches = Vec::new();

        for generator in &self.generators {
            let generated_patches = generator.generate(plan);
            for patch in generated_patches {
                patches.push(patch.clone());
                self.history.push(patch);
            }
        }

        patches
    }

    /// 获取生成历史
    pub fn history(&self) -> &[CodePatch] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::PlanType;

    #[test]
    fn test_persistence_code_generator() {
        let generator = PersistenceCodeGenerator::new();
        let plan = GeneratedPlan {
            id: "test".to_string(),
            plan_type: PlanType::CodeChange,
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            steps: vec![],
            expected_outcome: "Test".to_string(),
            risks: vec![],
            requires_human: false,
            generated_at: chrono::Utc::now(),
        };

        let patches = generator.generate(&plan);
        assert!(!patches.is_empty());
    }

    #[test]
    fn test_config_fix_code_generator() {
        let generator = ConfigFixCodeGenerator::new();
        let plan = GeneratedPlan {
            id: "test".to_string(),
            plan_type: PlanType::ConfigChange,
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            steps: vec![],
            expected_outcome: "Test".to_string(),
            risks: vec![],
            requires_human: false,
            generated_at: chrono::Utc::now(),
        };

        let patches = generator.generate(&plan);
        assert!(!patches.is_empty());
    }

    #[test]
    fn test_code_generator_manager() {
        let mut manager = CodeGeneratorManager::new();

        manager.register_generator(Box::new(PersistenceCodeGenerator::new()));
        manager.register_generator(Box::new(ConfigFixCodeGenerator::new()));

        let plan = GeneratedPlan {
            id: "test".to_string(),
            plan_type: PlanType::CodeChange,
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            steps: vec![],
            expected_outcome: "Test".to_string(),
            risks: vec![],
            requires_human: false,
            generated_at: chrono::Utc::now(),
        };

        let patches = manager.generate_patches(&plan);
        assert!(!patches.is_empty());
    }
}
