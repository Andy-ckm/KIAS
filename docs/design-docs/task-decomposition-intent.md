# AgentGuard 任务拆解与意图识别技术方案

> 基于 GitHub 开源项目和最佳实践

## 1. 核心技术点

### 1.1 任务拆解（Task Decomposition）

**目标**：将复杂任务拆解为可执行的子任务，分配给不同的 Agent

**参考项目**：
- [SkyworkAI/DeepResearchAgent](https://github.com/SkyworkAI/DeepResearchAgent) - 分层多 Agent 系统
- [langgenius/dify](https://github.com/langgenius/dify) - Agent 工作流平台
- [microsoft/agent-framework](https://github.com/microsoft/agent-framework) - Agent 编排框架

### 1.2 意图识别（Intent Recognition）

**目标**：准确识别用户意图，区分主动和被动场景

**参考项目**：
- [lbs-researcher/LBS-IntentBench](https://github.com/lbs-researcher/LBS-IntentBench) - 意图理解基准
- [arcane-bear/agent-router](https://github.com/arcane-bear/agent-router) - Agent 路由器
- [mjunior/whatsapp-ai-pix-agent](https://github.com/mjunior/whatsapp-ai-pix-agent) - 意图分类状态机

---

## 2. 任务拆解架构

### 2.1 分层任务规划（Hierarchical Task Planning）

```
用户请求
    │
    ▼
┌─────────────────────────────────────┐
│  Level 1: 意图识别                   │
│  - 任务类型分类                      │
│  - 复杂度评估                        │
│  - 优先级判断                        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Level 2: 任务规划                   │
│  - DAG 构建                         │
│  - 依赖分析                         │
│  - 资源估算                         │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Level 3: 任务分配                   │
│  - Agent 能力匹配                   │
│  - 负载均衡                         │
│  - 优先级调度                       │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Level 4: 执行监控                   │
│  - 进度追踪                         │
│  - 异常处理                         │
│  - 结果聚合                         │
└─────────────────────────────────────┘
```

### 2.2 DAG 任务图（Directed Acyclic Graph）

```rust
/// 任务节点
#[derive(Debug, Clone)]
struct TaskNode {
    id: String,
    task_type: TaskType,
    description: String,
    dependencies: Vec<String>,  // 依赖的任务 ID
    required_skills: Vec<String>,
    estimated_duration: Duration,
    priority: Priority,
}

/// 任务图
struct TaskGraph {
    nodes: HashMap<String, TaskNode>,
    edges: Vec<(String, String)>,  // (from, to)
}

impl TaskGraph {
    /// 拓扑排序，获取执行顺序
    fn topological_sort(&self) -> Result<Vec<String>, CycleError> {
        // Kahn's algorithm
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node_id in self.nodes.keys() {
            in_degree.entry(node_id.clone()).or_insert(0);
        }
        for (_, to) in &self.edges {
            *in_degree.get_mut(to).unwrap() += 1;
        }
        
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();
        
        let mut result = Vec::new();
        while let Some(node_id) = queue.pop() {
            result.push(node_id.clone());
            for (from, to) in &self.edges {
                if *from == node_id {
                    let deg = in_degree.get_mut(to).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(to.clone());
                    }
                }
            }
        }
        
        if result.len() == self.nodes.len() {
            Ok(result)
        } else {
            Err(CycleError)
        }
    }
}
```

### 2.3 任务规划器（Task Planner）

```rust
/// 任务规划器
struct TaskPlanner {
    llm: LlmClient,
    skill_registry: SkillRegistry,
}

impl TaskPlanner {
    /// 规划任务
    async fn plan(&self, request: &str) -> Result<TaskGraph, PlanningError> {
        // 1. 意图识别
        let intent = self.recognize_intent(request).await?;
        
        // 2. 任务拆解
        let tasks = self.decompose_task(request, &intent).await?;
        
        // 3. 依赖分析
        let dependencies = self.analyze_dependencies(&tasks).await?;
        
        // 4. 构建 DAG
        let graph = self.build_graph(tasks, dependencies);
        
        Ok(graph)
    }
    
    /// 识别意图
    async fn recognize_intent(&self, request: &str) -> Result<Intent, PlanningError> {
        let prompt = format!(
            r#"分析以下用户请求，识别意图：
            
用户请求：{request}

请输出 JSON 格式：
{{
    "task_type": "code_generation|bug_fix|research|analysis|deployment",
    "complexity": "simple|medium|complex",
    "priority": "low|medium|high|urgent",
    "required_skills": ["skill1", "skill2"],
    "estimated_duration": "minutes|hours|days"
}}"#
        );
        
        let response = self.llm.complete(&prompt).await?;
        let intent: Intent = serde_json::from_str(&response)?;
        
        Ok(intent)
    }
    
    /// 拆解任务
    async fn decompose_task(
        &self,
        request: &str,
        intent: &Intent,
    ) -> Result<Vec<TaskNode>, PlanningError> {
        let prompt = format!(
            r#"将以下任务拆解为子任务：

用户请求：{request}
任务类型：{task_type}
复杂度：{complexity}

请输出 JSON 数组，每个子任务包含：
{{
    "id": "task_1",
    "description": "子任务描述",
    "required_skills": ["skill1"],
    "estimated_duration": "10m"
}}"#,
            task_type = intent.task_type,
            complexity = intent.complexity,
        );
        
        let response = self.llm.complete(&prompt).await?;
        let tasks: Vec<TaskNode> = serde_json::from_str(&response)?;
        
        Ok(tasks)
    }
    
    /// 分析依赖
    async fn analyze_dependencies(
        &self,
        tasks: &[TaskNode],
    ) -> Result<Vec<(String, String)>, PlanningError> {
        let prompt = format!(
            r#"分析以下子任务的依赖关系：

{tasks}

请输出 JSON 数组，每个依赖包含：
{{
    "from": "task_1",
    "to": "task_2",
    "reason": "task_2 依赖 task_1 的输出"
}}"#,
            tasks = serde_json::to_string(tasks)?,
        );
        
        let response = self.llm.complete(&prompt).await?;
        let dependencies: Vec<(String, String)> = serde_json::from_str(&response)?;
        
        Ok(dependencies)
    }
}
```

---

## 3. 意图识别架构

### 3.1 主动意图识别（Proactive Intent Recognition）

**场景**：用户主动发起请求

```rust
/// 主动意图识别器
struct ProactiveIntentRecognizer {
    llm: LlmClient,
    classifiers: Vec<Box<dyn Classifier>>,
}

impl ProactiveIntentRecognizer {
    /// 识别主动意图
    async fn recognize(&self, input: &str) -> Result<Intent, IntentError> {
        // 1. 关键词提取
        let keywords = self.extract_keywords(input).await?;
        
        // 2. 分类器投票
        let mut votes: HashMap<IntentType, f64> = HashMap::new();
        for classifier in &self.classifiers {
            let (intent_type, confidence) = classifier.classify(input).await?;
            *votes.entry(intent_type).or_insert(0.0) += confidence;
        }
        
        // 3. 选择最高票数的意图
        let intent_type = votes
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, _)| k.clone())
            .unwrap_or(IntentType::Unknown);
        
        // 4. 使用 LLM 细化意图
        let refined_intent = self.refine_with_llm(input, &intent_type).await?;
        
        Ok(refined_intent)
    }
    
    /// 关键词提取
    async fn extract_keywords(&self, input: &str) -> Result<Vec<String>, IntentError> {
        let prompt = format!(
            r#"从以下文本中提取关键词：

{input}

请输出 JSON 数组：["keyword1", "keyword2"]"#
        );
        
        let response = self.llm.complete(&prompt).await?;
        let keywords: Vec<String> = serde_json::from_str(&response)?;
        
        Ok(keywords)
    }
    
    /// LLM 细化意图
    async fn refine_with_llm(
        &self,
        input: &str,
        intent_type: &IntentType,
    ) -> Result<Intent, IntentError> {
        let prompt = format!(
            r#"用户请求：{input}
初步意图分类：{intent_type}

请细化意图，输出 JSON：
{{
    "type": "{intent_type}",
    "subtype": "具体子类型",
    "action": "具体动作",
    "target": "操作对象",
    "constraints": ["约束1", "约束2"],
    "priority": "low|medium|high|urgent"
}}"#
        );
        
        let response = self.llm.complete(&prompt).await?;
        let intent: Intent = serde_json::from_str(&response)?;
        
        Ok(intent)
    }
}
```

### 3.2 被动意图识别（Reactive Intent Recognition）

**场景**：系统主动发现问题并处理

```rust
/// 被动意图识别器
struct ReactiveIntentRecognizer {
    monitors: Vec<Box<dyn Monitor>>,
    llm: LlmClient,
}

impl ReactiveIntentRecognizer {
    /// 监控并识别被动意图
    async fn monitor_and_recognize(&self) -> Vec<Intent> {
        let mut intents = Vec::new();
        
        for monitor in &self.monitors {
            // 1. 监控系统状态
            let events = monitor.check().await;
            
            for event in events {
                // 2. 分析事件
                let intent = self.analyze_event(&event).await;
                
                if let Ok(intent) = intent {
                    intents.push(intent);
                }
            }
        }
        
        intents
    }
    
    /// 分析事件
    async fn analyze_event(&self, event: &Event) -> Result<Intent, IntentError> {
        let prompt = format!(
            r#"分析以下系统事件，识别需要采取的行动：

事件类型：{event_type}
事件详情：{details}
时间：{timestamp}

请输出 JSON：
{{
    "type": "proactive",
    "action": "建议的行动",
    "urgency": "low|medium|high|critical",
    "reason": "采取行动的原因"
}}"#,
            event_type = event.event_type,
            details = event.details,
            timestamp = event.timestamp,
        );
        
        let response = self.llm.complete(&prompt).await?;
        let intent: Intent = serde_json::from_str(&response)?;
        
        Ok(intent)
    }
}

/// 监控器 trait
#[async_trait]
trait Monitor: Send + Sync {
    async fn check(&self) -> Vec<Event>;
}

/// Agent 健康监控
struct AgentHealthMonitor {
    storage: Arc<Storage>,
}

#[async_trait]
impl Monitor for AgentHealthMonitor {
    async fn check(&self) -> Vec<Event> {
        let mut events = Vec::new();
        
        let agents = self.storage.list_agents().await.unwrap_or_default();
        for agent in agents {
            // 检查心跳
            if agent.last_heartbeat.elapsed() > Duration::from_secs(60) {
                events.push(Event {
                    event_type: "agent_heartbeat_timeout".to_string(),
                    details: format!("Agent {} 心跳超时", agent.name),
                    timestamp: Utc::now(),
                });
            }
            
            // 检查资源使用
            if agent.cpu_usage > 90.0 {
                events.push(Event {
                    event_type: "agent_high_cpu".to_string(),
                    details: format!("Agent {} CPU 使用率过高: {}%", agent.name, agent.cpu_usage),
                    timestamp: Utc::now(),
                });
            }
        }
        
        events
    }
}
```

### 3.3 意图分类器（Intent Classifier）

```rust
/// 意图类型
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum IntentType {
    // 代码相关
    CodeGeneration,
    BugFix,
    CodeReview,
    Refactoring,
    
    // 研究相关
    Research,
    Analysis,
    Documentation,
    
    // 运维相关
    Deployment,
    Monitoring,
    Troubleshooting,
    
    // 知识相关
    KnowledgeQuery,
    KnowledgeUpdate,
    
    // 未知
    Unknown,
}

/// 意图分类器
#[async_trait]
trait Classifier: Send + Sync {
    async fn classify(&self, input: &str) -> Result<(IntentType, f64), IntentError>;
}

/// 基于关键词的分类器
struct KeywordClassifier {
    keywords: HashMap<IntentType, Vec<String>>,
}

#[async_trait]
impl Classifier for KeywordClassifier {
    async fn classify(&self, input: &str) -> Result<(IntentType, f64), IntentError> {
        let input_lower = input.to_lowercase();
        
        let mut scores: HashMap<IntentType, f64> = HashMap::new();
        
        for (intent_type, keywords) in &self.keywords {
            let matches = keywords
                .iter()
                .filter(|kw| input_lower.contains(kw.as_str()))
                .count();
            
            if matches > 0 {
                let score = matches as f64 / keywords.len() as f64;
                scores.insert(intent_type.clone(), score);
            }
        }
        
        let best = scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap());
        
        match best {
            Some((intent_type, &score)) => Ok((intent_type.clone(), score)),
            None => Ok((IntentType::Unknown, 0.0)),
        }
    }
}

/// 基于 LLM 的分类器
struct LlmClassifier {
    llm: LlmClient,
}

#[async_trait]
impl Classifier for LlmClassifier {
    async fn classify(&self, input: &str) -> Result<(IntentType, f64), IntentError> {
        let prompt = format!(
            r#"分类以下用户请求的意图：

{input}

可选分类：
- code_generation: 代码生成
- bug_fix: Bug 修复
- code_review: 代码审查
- refactoring: 代码重构
- research: 研究调查
- analysis: 数据分析
- documentation: 文档编写
- deployment: 部署运维
- monitoring: 监控告警
- troubleshooting: 故障排查
- knowledge_query: 知识查询
- knowledge_update: 知识更新

请输出 JSON：
{{
    "intent_type": "分类结果",
    "confidence": 0.95
}}"#
        );
        
        let response = self.llm.complete(&prompt).await?;
        let result: (IntentType, f64) = serde_json::from_str(&response)?;
        
        Ok(result)
    }
}
```

---

## 4. Agent 路由器（Agent Router）

```rust
/// Agent 路由器
struct AgentRouter {
    agents: Vec<Agent>,
    intent_classifier: IntentClassifier,
    skill_matcher: SkillMatcher,
}

impl AgentRouter {
    /// 路由请求到合适的 Agent
    async fn route(&self, request: &str) -> Result<Agent, RouterError> {
        // 1. 识别意图
        let intent = self.intent_classifier.classify(request).await?;
        
        // 2. 匹配技能
        let required_skills = self.map_intent_to_skills(&intent);
        
        // 3. 找到具备技能的 Agent
        let candidates = self.skill_matcher.find_agents(&required_skills);
        
        // 4. 选择最佳 Agent
        let best_agent = self.select_best_agent(&candidates, &intent);
        
        Ok(best_agent)
    }
    
    /// 意图到技能的映射
    fn map_intent_to_skills(&self, intent: &Intent) -> Vec<String> {
        match intent.intent_type {
            IntentType::CodeGeneration => vec![
                "fullstack-developer".to_string(),
                "frontend-design".to_string(),
            ],
            IntentType::BugFix => vec![
                "fix".to_string(),
                "code-reviewer".to_string(),
            ],
            IntentType::CodeReview => vec![
                "code-reviewer".to_string(),
                "frontend-code-review".to_string(),
            ],
            IntentType::Documentation => vec![
                "update-docs".to_string(),
            ],
            IntentType::KnowledgeQuery => vec![
                "knowledge-query".to_string(),
            ],
            _ => vec![],
        }
    }
    
    /// 选择最佳 Agent
    fn select_best_agent(&self, candidates: &[Agent], intent: &Intent) -> Agent {
        // 考虑因素：
        // 1. 技能匹配度
        // 2. 当前负载
        // 3. 历史表现
        // 4. 缓存亲和性
        
        candidates
            .iter()
            .min_by_key(|agent| {
                let skill_score = self.calculate_skill_score(agent, intent);
                let load_score = agent.current_load() as f64;
                let performance_score = agent.performance_score();
                
                // 加权评分
                (skill_score * 0.4 + load_score * 0.3 + performance_score * 0.3) as u64
            })
            .cloned()
            .unwrap()
    }
}
```

---

## 5. 任务执行引擎（Task Execution Engine）

```rust
/// 任务执行引擎
struct TaskExecutionEngine {
    task_graph: TaskGraph,
    agent_pool: AgentPool,
    scheduler: Scheduler,
}

impl TaskExecutionEngine {
    /// 执行任务图
    async fn execute(&self, graph: TaskGraph) -> Result<ExecutionResult, ExecutionError> {
        // 1. 拓扑排序
        let execution_order = graph.topological_sort()?;
        
        // 2. 并行执行
        let mut handles = Vec::new();
        let mut completed: HashSet<String> = HashSet::new();
        
        for task_id in execution_order {
            let task = graph.nodes.get(&task_id).unwrap();
            
            // 检查依赖是否完成
            let dependencies_met = task.dependencies.iter()
                .all(|dep| completed.contains(dep));
            
            if dependencies_met {
                // 调度到 Agent
                let agent = self.scheduler.schedule(task).await?;
                
                // 执行任务
                let handle = tokio::spawn({
                    let task = task.clone();
                    let agent = agent.clone();
                    async move {
                        agent.execute(task).await
                    }
                });
                
                handles.push((task_id.clone(), handle));
                completed.insert(task_id);
            }
        }
        
        // 3. 等待所有任务完成
        let mut results = HashMap::new();
        for (task_id, handle) in handles {
            let result = handle.await??;
            results.insert(task_id, result);
        }
        
        Ok(ExecutionResult { results })
    }
}
```

---

## 6. 参考项目总结

### 6.1 任务拆解

| 项目 | Stars | 核心价值 |
|------|-------|----------|
| DeepResearchAgent | 3387 | 分层多 Agent 系统，自动任务拆解 |
| dify | 141226 | Agent 工作流平台，可视化编排 |
| agent-framework | 10398 | 微软 Agent 编排框架 |

### 6.2 意图识别

| 项目 | Stars | 核心价值 |
|------|-------|----------|
| LBS-IntentBench | 11 | 隐式意图理解基准 |
| agent-router | 4 | 轻量级 Agent 路由器 |
| whatsapp-ai-pix-agent | 14 | 意图分类状态机 |

### 6.3 任务调度

| 项目 | Stars | 核心价值 |
|------|-------|----------|
| hatchet | 7138 | Agent 工作流编排引擎 |
| trigger.dev | 14905 | AI Agent 工作流平台 |

### 6.4 工具调用

| 项目 | Stars | 核心价值 |
|------|-------|----------|
| aci | 4768 | 开源工具调用平台 |
| tool-calling-guide | 82 | 工具调用学习指南 |

### 6.5 记忆管理

| 项目 | Stars | 核心价值 |
|------|-------|----------|
| OpenViking | 23860 | Agent 上下文数据库 |
| Memoria | 265 | Agent 安全记忆管理 |

---

## 7. 实施计划

### Phase 1: 基础框架（1 周）
- [ ] 实现意图分类器
- [ ] 实现任务图数据结构
- [ ] 实现基础任务规划器

### Phase 2: 核心功能（2 周）
- [ ] 实现 DAG 拓扑排序
- [ ] 实现 Agent 路由器
- [ ] 实现任务执行引擎

### Phase 3: 高级功能（2 周）
- [ ] 实现被动意图识别
- [ ] 实现任务依赖分析
- [ ] 实现执行监控

### Phase 4: 优化迭代（1 周）
- [ ] 性能优化
- [ ] 测试覆盖
- [ ] 文档完善