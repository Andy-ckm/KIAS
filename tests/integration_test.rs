//! KIAS 集成测试

use kias_common::error::KiasResult;

#[test]
fn test_common_error_types() {
    // 测试错误类型是否正确
    let error = kias_common::error::KiasError::Scheduler("test".to_string());
    assert!(error.to_string().contains("Scheduler"));
}

#[test]
fn test_common_config_default() {
    // 测试默认配置
    let config = kias_common::config::AppConfig::default();
    assert_eq!(config.api_port, 8080);
}

#[tokio::test]
async fn test_scheduler_round_robin() {
    use kias_scheduler::{SchedulerEngine, RoundRobin};
    use std::sync::Arc;
    
    let strategy = Arc::new(RoundRobin::new());
    let engine = SchedulerEngine::new(strategy);
    
    let nodes = vec![
        "node-1".to_string(),
        "node-2".to_string(),
        "node-3".to_string(),
    ];
    
    // 测试轮询调度
    let result1 = engine.schedule_task("task-1", &nodes).await.unwrap();
    let result2 = engine.schedule_task("task-2", &nodes).await.unwrap();
    let result3 = engine.schedule_task("task-3", &nodes).await.unwrap();
    let result4 = engine.schedule_task("task-4", &nodes).await.unwrap();
    
    // 应该轮询分配
    assert_eq!(result1, "node-1");
    assert_eq!(result2, "node-2");
    assert_eq!(result3, "node-3");
    assert_eq!(result4, "node-1"); // 循环回来
}

#[tokio::test]
async fn test_controller_reconcile() {
    use kias_controller::{DefaultReconciler, ControllerState, DesiredState, ActualState, AgentStatus, AgentConfig, ResourceRequirements};
    use chrono::Utc;
    
    let reconciler = DefaultReconciler::new();
    
    let mut state = ControllerState {
        desired: DesiredState {
            replicas: 3,
            agent_config: AgentConfig {
                name: "test-agent".to_string(),
                image: "test:latest".to_string(),
                resources: ResourceRequirements {
                    cpu: "100m".to_string(),
                    memory: "128Mi".to_string(),
                },
            },
        },
        actual: ActualState {
            running_replicas: 0,
            agent_status: AgentStatus::Pending,
            last_updated: Utc::now(),
        },
    };
    
    // 执行调和
    reconciler.reconcile(&mut state).await.unwrap();
    
    // 验证状态已更新
    assert_eq!(state.actual.running_replicas, 3);
}

#[tokio::test]
async fn test_knowledge_graph() {
    use kias_knowledge::KnowledgeGraph;
    use kias_knowledge::graph::{KnowledgeNode, NodeType};
    
    let mut graph = KnowledgeGraph::new();
    
    // 添加节点
    graph.add_node(KnowledgeNode {
        id: "node-1".to_string(),
        content: "Test content".to_string(),
        node_type: NodeType::Document,
        metadata: Default::default(),
    });
    
    // 查询节点
    let node = graph.get_node("node-1");
    assert!(node.is_some());
    assert_eq!(node.unwrap().content, "Test content");
}

#[tokio::test]
async fn test_cache_lru() {
    use kias_cache::{CacheHub, LRUStrategy};
    use kias_cache::hub::CacheEntry;
    use chrono::Utc;
    
    let strategy = Box::new(LRUStrategy::new());
    let hub = CacheHub::new(strategy);
    
    // 设置缓存
    let entry = CacheEntry {
        key: "test-key".to_string(),
        value: b"test-value".to_vec(),
        created_at: Utc::now(),
        ttl: None,
    };
    
    hub.set(entry).await.unwrap();
    
    // 获取缓存
    let cached = hub.get("test-key").await.unwrap();
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().value, b"test-value");
}

#[tokio::test]
async fn test_monitor_telemetry() {
    use kias_monitor::{TelemetryCollector};
    use kias_monitor::telemetry::{TelemetryEvent, EventType};
    use chrono::Utc;
    use uuid::Uuid;
    
    let mut telemetry = TelemetryCollector::new();
    
    let event = TelemetryEvent {
        event_id: Uuid::new_v4().to_string(),
        event_type: EventType::TaskStarted,
        agent_id: "agent-1".to_string(),
        timestamp: Utc::now(),
        data: serde_json::json!({"task": "test"}),
    };
    
    telemetry.collect(event);
    
    assert_eq!(telemetry.get_events().len(), 1);
}

#[tokio::test]
async fn test_executor_task() {
    use kias_executor::{TaskRuntime, Task};
    use kias_executor::runtime::TaskExecutor;
    use async_trait::async_trait;
    use kias_common::error::KiasResult;
    use chrono::Utc;
    use uuid::Uuid;
    
    struct TestExecutor;
    
    #[async_trait]
    impl TaskExecutor for TestExecutor {
        async fn execute(&self, _task: &Task) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"result": "success"}))
        }
    }
    
    let executor = Box::new(TestExecutor);
    let runtime = TaskRuntime::new(executor);
    
    let task = Task {
        id: Uuid::new_v4().to_string(),
        name: "test-task".to_string(),
        agent_id: "agent-1".to_string(),
        payload: serde_json::json!({}),
        created_at: Utc::now(),
        timeout: None,
    };
    
    let result = runtime.run_task(&task).await.unwrap();
    assert!(result.output.is_some());
}

#[tokio::test]
async fn test_skills_registry() {
    use kias_skills::{SkillRegistry, Skill};
    use async_trait::async_trait;
    use kias_common::error::KiasResult;
    
    struct TestSkill;
    
    #[async_trait]
    impl Skill for TestSkill {
        fn name(&self) -> &str {
            "test"
        }
        
        fn description(&self) -> &str {
            "A test skill"
        }
        
        async fn execute(&self, _params: serde_json::Value) -> KiasResult<serde_json::Value> {
            Ok(serde_json::json!({"result": "ok"}))
        }
    }
    
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(TestSkill));
    
    assert!(registry.get("test").is_some());
    assert!(registry.get("nonexistent").is_none());
}
