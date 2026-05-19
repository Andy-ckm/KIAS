# AgentGuard 知识管理系统

> 借鉴 LLM Wiki + GBrain + Obsidian-Wiki，实现 Agent 知识的自组织与自进化

## 核心定位

AgentGuard 知识管理系统 = **LLM Wiki 的三层架构** + **GBrain 的混合检索** + **Agent 集群调度**

## 架构设计

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         AgentGuard Knowledge System                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Knowledge Layer (Wiki)                         │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │   │
│  │  │  Schema     │  │  Wiki       │  │  Index      │                 │   │
│  │  │  (规则)      │  │  (知识)      │  │  (导航)      │                 │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Retrieval Layer (GBrain)                       │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │   │
│  │  │  Vector     │  │  Keyword    │  │  Graph      │                 │   │
│  │  │  Search     │  │  Search     │  │  Boost      │                 │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      Storage Layer                                  │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                 │   │
│  │  │  Markdown   │  │  Vector DB  │  │  Graph DB   │                 │   │
│  │  │  Files      │  │  (SQLite)   │  │  (关系)      │                 │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. Knowledge Layer（LLM Wiki 三层架构）

**借鉴**：[karpathy/llm-wiki](https://github.com/karpathy/llm-wiki)

#### Schema Layer（规则层）
```yaml
# knowledge/schema.yaml
rules:
  - name: "agent-knowledge"
    description: "Agent 相关知识的组织规则"
    structure:
      - "agents/{agent-name}/README.md"      # Agent 概述
      - "agents/{agent-name}/skills.md"       # 技能清单
      - "agents/{agent-name}/context.md"      # 上下文配置
      - "agents/{agent-name}/history.md"      # 交互历史
    
  - name: "project-knowledge"
    description: "项目相关知识的组织规则"
    structure:
      - "projects/{project-name}/README.md"   # 项目概述
      - "projects/{project-name}/arch.md"     # 架构设计
      - "projects/{project-name}/decisions.md" # 决策记录
```

#### Wiki Layer（知识层）
```markdown
# agents/code-reviewer/README.md

## 概述
代码审查 Agent，专注于 Rust 项目的代码质量检查。

## 核心能力
- 代码风格检查（clippy, rustfmt）
- 安全漏洞扫描
- 性能优化建议
- 架构合规性验证

## 知识来源
- [[projects/kias/arch.md]] - AgentGuard 架构设计
- [[skills/rust-best-practices.md]] - Rust 最佳实践
- [[references/rust-security.md]] - Rust 安全指南

## 最后更新
2026-05-13: 添加了 AgentGuard 项目的审查规则
```

#### Index Layer（索引层）
```markdown
# knowledge/index.md

## 导航

### 按领域
- [[agents/]] - Agent 知识库
- [[projects/]] - 项目知识库
- [[skills/]] - 技能知识库
- [[references/]] - 参考资料

### 按时间
- [[recent-changes.md]] - 最近变更
- [[2026-05/]] - 2026年5月

### 按关联
- [[graph/]] - 知识图谱
```

### 2. Retrieval Layer（GBrain 混合检索）

**借鉴**：[gbraintools/gbrain](https://github.com/gbraintools/gbrain)

#### 检索流程
```
用户查询
    │
    ▼
┌─────────────────────────────────────┐
│  Step 1: 混合搜索（Hybrid Search）   │
│  - 向量相似度（语义匹配）             │
│  - 关键词匹配（精确匹配）             │
│  - 图谱加权（入度 boost）            │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Step 2: 分块确认（Chunk Confirm）   │
│  - 筛选相关片段（~2KB）              │
│  - 确认页面相关性                    │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Step 3: 整页加载（Full Page Load）  │
│  - 加载完整 Markdown 内容            │
│  - 获取上下文                        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Step 4: 分层呈现（Layered Feeding） │
│  - 优先：编译真相（摘要/结论）        │
│  - 其次：时间线证据（历史/来源）       │
└─────────────────────────────────────┘
```

#### 搜索实现
```rust
struct HybridSearch {
    vector_index: VectorIndex,
    keyword_index: KeywordIndex,
    graph: KnowledgeGraph,
}

impl HybridSearch {
    async fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        // 1. 向量搜索
        let vector_results = self.vector_index.search(query, limit * 2).await;
        
        // 2. 关键词搜索
        let keyword_results = self.keyword_index.search(query, limit * 2).await;
        
        // 3. 合并结果
        let mut merged = self.merge_results(vector_results, keyword_results);
        
        // 4. 图谱加权
        for result in &mut merged {
            let backlinks = self.graph.get_backlinks(&result.page_id).len();
            result.score += 0.1 * backlinks as f64;  // α = 0.1
        }
        
        // 5. 排序
        merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        merged.truncate(limit);
        
        merged
    }
}
```

### 3. Graph Layer（知识图谱）

**借鉴**：GBrain 的实体关系抽取

#### 实体抽取（规则驱动，非 AI）
```rust
struct EntityExtractor {
    // 正则匹配 wikilink
    wikilink_pattern: Regex,  // \[\[entity/name\]\]
    // 关系动词模式
    relation_patterns: HashMap<String, Regex>,
}

impl EntityExtractor {
    fn extract(&self, text: &str) -> Vec<Entity> {
        let mut entities = Vec::new();
        
        // 提取 wikilink
        for cap in self.wikilink_pattern.captures_iter(text) {
            entities.push(Entity {
                id: cap[1].to_string(),
                source: "wikilink".to_string(),
            });
        }
        
        entities
    }
    
    fn classify_relations(&self, text: &str, entities: &[Entity]) -> Vec<Relation> {
        let mut relations = Vec::new();
        
        // 关键词匹配关系
        for (relation_type, pattern) in &self.relation_patterns {
            if pattern.is_match(text) {
                // 提取主体和客体
                if let (Some(subject), Some(object)) = self.extract_subject_object(text) {
                    relations.push(Relation {
                        subject,
                        object,
                        relation_type: relation_type.clone(),
                    });
                }
            }
        }
        
        relations
    }
}
```

#### 反向链接强制化
```rust
struct BacklinkEnforcer {
    graph: KnowledgeGraph,
}

impl BacklinkEnforcer {
    fn enforce(&mut self, source: &str, target: &str) {
        // 如果 A 提到了 B，自动在 B 的页面添加指向 A 的反向链接
        self.graph.add_backlink(target, source);
        
        // 更新 Markdown 文件
        self.update_markdown_backlinks(target, source);
    }
    
    fn update_markdown_backlinks(&self, page: &str, backlink: &str) {
        let content = self.read_page(page);
        let backlink_section = format!(
            "\n## 反向链接\n- [[{}]]\n",
            backlink
        );
        
        if !content.contains(&backlink_section) {
            self.append_to_page(page, &backlink_section);
        }
    }
}
```

## 数据模型

### Knowledge Page
```rust
struct KnowledgePage {
    id: String,              // 唯一标识
    path: String,            // 文件路径
    content: String,         // Markdown 内容
    metadata: PageMetadata,  // 元数据
    backlinks: Vec<String>,  // 反向链接
    created_at: DateTime,    // 创建时间
    updated_at: DateTime,    // 更新时间
}

struct PageMetadata {
    tags: Vec<String>,       // 标签
    entities: Vec<String>,   // 实体
    confidence: f64,         // 置信度
    source: Option<String>,  // 来源
}
```

### Knowledge Graph
```rust
struct KnowledgeGraph {
    // 实体 -> 页面
    entities: HashMap<String, EntityNode>,
    // 关系列表
    relations: Vec<Relation>,
    // 反向链接索引
    backlinks: HashMap<String, Vec<String>>,
}

struct EntityNode {
    id: String,
    page_path: String,
    entity_type: EntityType,  // Person, Project, Skill, etc.
    properties: HashMap<String, String>,
}

struct Relation {
    subject: String,
    object: String,
    relation_type: String,  // works_at, founded, uses, etc.
    confidence: f64,
}
```

## API 设计

### 知识摄入
```bash
# 摄入新知识
kias knowledge ingest --source article.md --type article

# 批量摄入
kias knowledge ingest --source ./docs/ --type project

# 自动摄入（监控文件变化）
kias knowledge watch --path ./knowledge/
```

### 知识查询
```bash
# 查询知识
kias knowledge query "AgentGuard 的调度算法有哪些？"

# 查询并展示来源
kias knowledge query "AgentGuard 的调度算法有哪些？" --show-sources

# 查询相关实体
kias knowledge query --entity "kias-scheduler"
```

### 知识维护
```bash
# 健康检查
kias knowledge health

# 识别矛盾
kias knowledge lint

# 清理过时声明
kias knowledge cleanup --older-than 30d

# 重建索引
kias knowledge reindex
```

### 图谱操作
```bash
# 查看图谱
kias knowledge graph show

# 查询实体关系
kias knowledge graph query --entity "john-doe"

# 导出图谱
kias knowledge graph export --format json
```

## 配置

```yaml
# config/knowledge.yaml
knowledge:
  storage:
    path: "./knowledge"
    vector_db: "./data/knowledge_vectors.db"
    graph_db: "./data/knowledge_graph.db"
  
  retrieval:
    vector_weight: 0.6
    keyword_weight: 0.3
    graph_boost_weight: 0.1
    max_results: 10
  
  graph:
    auto_backlinks: true
    confidence_threshold: 0.7
  
  sync:
    enabled: true
    watch_paths: ["./docs", "./src"]
    interval: 300s  # 5分钟
```

## 与 Agent 集成

### Agent 知识注入
```rust
struct AgentWithKnowledge {
    agent: Agent,
    knowledge: KnowledgeSystem,
}

impl AgentWithKnowledge {
    async fn query(&self, question: &str) -> String {
        // 1. 查询知识库
        let results = self.knowledge.query(question).await;
        
        // 2. 构建上下文
        let context = self.build_context(results);
        
        // 3. 调用 LLM
        let response = self.agent.llm.complete(&context, question).await;
        
        // 4. 记录交互（可选）
        self.knowledge.record_interaction(question, &response).await;
        
        response
    }
    
    fn build_context(&self, results: Vec<SearchResult>) -> String {
        let mut context = String::new();
        
        // 分层呈现
        // 1. 优先：编译真相（摘要）
        for result in results.iter().filter(|r| r.is_summary) {
            context.push_str(&format!("## {}\n{}\n\n", result.title, result.content));
        }
        
        // 2. 其次：时间线证据
        for result in results.iter().filter(|r| !r.is_summary) {
            context.push_str(&format!("### 来源：{}\n{}\n\n", result.source, result.content));
        }
        
        context
    }
}
```

## 最佳实践

### 1. 知识组织
```
knowledge/
├── schema.yaml           # 规则层
├── index.md              # 索引层
├── agents/               # Agent 知识
│   ├── code-reviewer/
│   │   ├── README.md
│   │   ├── skills.md
│   │   └── context.md
│   └── ...
├── projects/             # 项目知识
│   ├── kias/
│   │   ├── README.md
│   │   ├── arch.md
│   │   └── decisions.md
│   └── ...
├── skills/               # 技能知识
│   ├── rust-best-practices.md
│   └── ...
└── references/           # 参考资料
    ├── articles/
    └── ...
```

### 2. 渐进式披露
- **AGENTS.md**：地图，告诉 Agent 去哪里找什么
- **Knowledge Index**：导航，快速定位相关知识
- **Full Page**：详情，深度阅读完整内容

### 3. 自动化维护
- 文件变化自动摄入
- 定期健康检查
- 矛盾自动识别
- 过时内容清理

## 参考

- [LLM Wiki](https://github.com/karpathy/llm-wiki)
- [GBrain](https://github.com/gbraintools/gbrain)
- [Obsidian-Wiki](https://github.com/bobheadxi/obsidian-wiki)
- [Obsidian](https://obsidian.md)