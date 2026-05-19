# AgentGuard 任务编排系统

> 借鉴 Hermes Kanban 的多 Agent 协作设计

## 核心理念

**文件式交接 + 状态推进**：Agent 之间通过文件传递数据，通过看板状态推进任务

## 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           AgentGuard Kanban System                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                        Board Layer (看板层)                          │   │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐ │   │
│  │  │ Triage  │  │  Todo   │  │  Ready  │  │  In     │  │ Blocked │ │   │
│  │  │ 待梳理  │  │  待办   │  │  就绪   │  │Progress │  │  阻塞   │ │   │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘  └─────────┘ │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Dispatcher Layer (调度层)                       │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │   │
│  │  │  Scanner    │  │  Allocator  │  │  Reclaimer  │                 │   │
│  │  │  扫描器     │  │  分配器     │  │  回收器     │                 │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Agent Layer (执行层)                            │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │   │
│  │  │ Researcher  │  │   Writer    │  │   Coder     │                 │   │
│  │  │   调研员    │  │    写手     │  │   开发者    │                 │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. Board（看板）

**六列状态**：
```
Triage → Todo → Ready → In Progress → Blocked → Done
待梳理    待办    就绪     进行中        阻塞      完成
```

**看板隔离**：每个项目一块看板，数据库、工作空间、调度器完全隔离

```rust
struct Board {
    id: String,
    name: String,
    description: String,
    tasks: Vec<Task>,
    created_at: DateTime<Utc>,
}

impl Board {
    /// 创建任务
    fn create_task(&mut self, task: Task) {
        self.tasks.push(task);
    }
    
    /// 获取指定状态的任务
    fn get_tasks_by_status(&self, status: TaskStatus) -> Vec<&Task> {
        self.tasks.iter().filter(|t| t.status == status).collect()
    }
}
```

### 2. Task（任务）

```rust
#[derive(Debug, Clone)]
struct Task {
    id: String,
    title: String,
    description: String,
    status: TaskStatus,
    assignee: String,        // 负责的 Profile
    tenant: Option<String>,  // 项目标签
    parents: Vec<String>,    // 父任务 ID
    children: Vec<String>,   // 子任务 ID
    workspace: PathBuf,      // 任务工作目录
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
enum TaskStatus {
    Triage,      // 待梳理
    Todo,        // 待办
    Ready,       // 就绪
    InProgress,  // 进行中
    Blocked,     // 阻塞
    Done,        // 完成
}
```

### 3. Profile（岗位说明书）

```rust
struct Profile {
    name: String,
    description: String,
    skills: Vec<String>,     // 技能列表
    tools: Vec<String>,      // 可用工具
    memory: Option<String>,  // 持久记忆
}

impl Profile {
    /// 创建 Profile
    fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            skills: Vec::new(),
            tools: Vec::new(),
            memory: None,
        }
    }
    
    /// 添加技能
    fn with_skill(mut self, skill: &str) -> Self {
        self.skills.push(skill.to_string());
        self
    }
    
    /// 添加工具
    fn with_tool(mut self, tool: &str) -> Self {
        self.tools.push(tool.to_string());
        self
    }
}
```

### 4. Dispatcher（调度器）

```rust
struct Dispatcher {
    boards: HashMap<String, Board>,
    profiles: HashMap<String, Profile>,
    interval: Duration,      // 扫描间隔
    max_concurrent: usize,   // 最大并发数
}

impl Dispatcher {
    /// 扫描并分配任务
    async fn scan_and_dispatch(&self) {
        for board in self.boards.values() {
            // 1. 获取 Ready 状态的任务
            let ready_tasks = board.get_tasks_by_status(TaskStatus::Ready);
            
            // 2. 分配给对应的 Agent
            for task in ready_tasks {
                if let Some(profile) = self.profiles.get(&task.assignee) {
                    self.spawn_agent(task, profile).await;
                }
            }
        }
    }
    
    /// 启动 Agent 执行任务
    async fn spawn_agent(&self, task: &Task, profile: &Profile) {
        // 1. 创建工作目录
        let workspace = self.create_workspace(task);
        
        // 2. 准备上下文（父任务摘要）
        let context = self.prepare_context(task);
        
        // 3. 启动 Agent 进程
        let agent = Agent::new(profile.clone(), workspace, context);
        agent.execute(task).await;
    }
    
    /// 回收超时任务
    async fn reclaim_stale_tasks(&self) {
        for board in self.boards.values() {
            let in_progress = board.get_tasks_by_status(TaskStatus::InProgress);
            
            for task in in_progress {
                if self.is_stale(task) {
                    // 回收任务
                    self.reclaim_task(task);
                    
                    // 检查是否需要熔断
                    if task.failure_count >= 3 {
                        self.circuit_break(task);
                    }
                }
            }
        }
    }
}
```

### 5. Orchestrator（编排器）

```rust
struct Orchestrator {
    dispatcher: Dispatcher,
    llm: LlmClient,
}

impl Orchestrator {
    /// 自然语言驱动任务拆解
    async fn orchestrate(&self, request: &str) -> Result<Vec<Task>, OrchestrateError> {
        // 1. 解析用户意图
        let intent = self.parse_intent(request).await?;
        
        // 2. 拆解任务
        let tasks = self.decompose_tasks(&intent).await?;
        
        // 3. 建立依赖关系
        let linked_tasks = self.link_tasks(tasks).await?;
        
        // 4. 创建到看板
        for task in &linked_tasks {
            self.dispatcher.create_task(task).await?;
        }
        
        Ok(linked_tasks)
    }
    
    /// 拆解任务
    async fn decompose_tasks(&self, intent: &Intent) -> Result<Vec<Task>, OrchestrateError> {
        let prompt = format!(
            r#"将以下目标拆解为子任务：

目标：{description}

可用角色：
- researcher: 调研员
- writer: 写手
- coder: 开发者
- video-worker: 视频创作者

请输出 JSON 数组，每个任务包含：
{{
    "title": "任务标题",
    "assignee": "负责角色",
    "dependencies": ["依赖的任务标题"]
}}"#,
            description = intent.description,
        );
        
        let response = self.llm.complete(&prompt).await?;
        let tasks: Vec<Task> = serde_json::from_str(&response)?;
        
        Ok(tasks)
    }
    
    /// 建立任务依赖
    async fn link_tasks(&self, tasks: Vec<Task>) -> Result<Vec<Task>, OrchestrateError> {
        let mut linked_tasks = Vec::new();
        
        for task in tasks {
            let mut linked_task = task.clone();
            
            // 设置父任务
            for dep in &task.dependencies {
                if let Some(parent) = linked_tasks.iter().find(|t| &t.title == dep) {
                    linked_task.parents.push(parent.id.clone());
                }
            }
            
            linked_tasks.push(linked_task);
        }
        
        // 更新子任务
        for i in 0..linked_tasks.len() {
            let parents = linked_tasks[i].parents.clone();
            for parent_id in parents {
                if let Some(parent) = linked_tasks.iter_mut().find(|t| t.id == parent_id) {
                    parent.children.push(linked_tasks[i].id.clone());
                }
            }
        }
        
        Ok(linked_tasks)
    }
}
```

## 文件式交接

### 工作空间结构

```
~/.kias/kanban/workspaces/
├── t_f3021692/           # 任务 T1 的工作空间
│   ├── summary.md        # 任务摘要
│   ├── notes/            # 工作笔记
│   └── output/           # 产出文件
├── t_391e4acc/           # 任务 T2 的工作空间
│   ├── summary.md
│   └── input/            # 从 T1 获取的输入
└── t_8ca0e4dc/           # 任务 T3 的工作空间
    └── ...
```

### 交接流程

```rust
/// 任务完成时
fn on_task_complete(task: &Task) {
    // 1. 写入摘要
    let summary = Summary {
        task_id: task.id.clone(),
        description: task.description.clone(),
        output_files: list_output_files(task),
        key_findings: extract_key_findings(task),
    };
    write_summary(task, &summary);
    
    // 2. 提升子任务状态
    for child_id in &task.children {
        if let Some(child) = get_task(child_id) {
            if child.parents.iter().all(|p| is_task_done(p)) {
                promote_task(child);
            }
        }
    }
}

/// 任务开始时
fn on_task_start(task: &Task) {
    // 1. 读取父任务摘要
    for parent_id in &task.parents {
        if let Some(parent) = get_task(parent_id) {
            let summary = read_summary(parent);
            inject_context(task, &summary);
        }
    }
    
    // 2. 设置工作目录
    let workspace = get_workspace(task);
    set_working_directory(workspace);
}
```

## 失败处理

### 自动重试

```rust
struct RetryPolicy {
    max_attempts: usize,      // 最大重试次数
    backoff: Duration,        // 退避时间
    circuit_break: bool,      // 是否熔断
}

impl RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            backoff: Duration::from_secs(60),
            circuit_break: true,
        }
    }
}

/// 任务失败时
fn on_task_failure(task: &Task, error: &Error) {
    task.failure_count += 1;
    
    if task.failure_count >= 3 {
        // 熔断：锁定任务，等待人工介入
        task.status = TaskStatus::Blocked;
        task.block_reason = format!("连续失败 {} 次: {}", task.failure_count, error);
    } else {
        // 重试：重新放回 Ready 队列
        task.status = TaskStatus::Ready;
        task.retry_after = Utc::now() + Duration::from_secs(60);
    }
}
```

### 人工介入

```rust
/// 阻塞任务
fn block_task(task: &Task, reason: &str) {
    task.status = TaskStatus::Blocked;
    task.block_reason = reason.to_string();
}

/// 解除阻塞
fn unblock_task(task: &Task, comment: &str) {
    task.status = TaskStatus::Ready;
    task.comments.push(comment.to_string());
}
```

## 多看板管理

```rust
struct BoardManager {
    boards: HashMap<String, Board>,
    current_board: String,
}

impl BoardManager {
    /// 创建看板
    fn create_board(&mut self, name: &str, description: &str) {
        let board = Board::new(name, description);
        self.boards.insert(name.to_string(), board);
    }
    
    /// 切换看板
    fn switch_board(&mut self, name: &str) {
        if self.boards.contains_key(name) {
            self.current_board = name.to_string();
        }
    }
    
    /// 获取当前看板
    fn current(&self) -> &Board {
        self.boards.get(&self.current_board).unwrap()
    }
}
```

## 命令行接口

### 任务管理

```bash
# 创建任务
kias kanban create "写一篇技术博客" \
    --assignee writer \
    --tenant blog-content

# 查看任务列表
kias kanban list

# 查看任务详情
kias kanban show t_058c4d9c

# 添加评论
kias kanban comment t_058c4d9c "请用中文撰写，字数 2000 左右"

# 解除阻塞
kias kanban unblock t_058c4d9c

# 查看任务依赖树
kias kanban tree t_f3021692
```

### 看板管理

```bash
# 创建看板
kias kanban boards create blog-content "博客内容看板"

# 切换看板
kias kanban boards switch blog-content

# 列出看板
kias kanban boards list

# 查看当前看板
kias kanban boards show
```

### 编排器

```bash
# 自然语言驱动
kias kanban orchestrate "创建一个产品介绍视频，先调研产品亮点，再写脚本，最后生成视频"

# 查看编排结果
kias kanban tree <orchestration_id>
```

## 配置

```yaml
# ~/.kias/config.yaml

kanban:
  # 调度器配置
  dispatcher:
    interval: 60s           # 扫描间隔
    max_concurrent: 5       # 最大并发数
    enable_leader_election: true  # 多实例选主
  
  # 重试策略
  retry:
    max_attempts: 3
    backoff: 60s
    circuit_break: true
  
  # 工作空间
  workspace:
    path: ~/.kias/kanban/workspaces
    cleanup_after: 30d      # 完成 30 天后清理
  
  # Profile 配置
  profiles:
    researcher:
      skills: ["research", "analysis"]
      tools: ["web_search", "file_read"]
    
    writer:
      skills: ["writing", "editing"]
      tools: ["file_write", "markdown"]
    
    coder:
      skills: ["programming", "debugging"]
      tools: ["terminal", "file_edit"]
```

## 参考

- [Hermes Agent](https://github.com/nousresearch/hermes-agent)
- [Hermes Kanban 文档](https://hermes-agent.nousresearch.com/docs/kanban)