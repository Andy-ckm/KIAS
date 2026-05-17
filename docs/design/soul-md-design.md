# SOUL.md 身份层设计方案

## 目标
给每个 Agent 加一个 SOUL.md 文件，定义其身份、性格、约束、知识域。
灵感来自 Hermes Agent 的 SOUL.md 机制。

## 文件格式

```yaml
---
name: researcher
role: intelligence
version: "1.0"

personality:
  style: analytical
  thoroughness: high
  communication: formal

knowledge_domains:
  - ai
  - ml
  - nlp

capabilities:
  - pattern_recognition
  - literature_review
  - data_analysis

constraints:
  - always_cite_sources
  - never_speculate_without_evidence
  - prefer_peer_reviewed

tools:
  - web_search
  - arxiv
  - code_exec
---

# Research Agent

Detailed description of the agent's purpose, expertise, and behavior patterns.
This markdown body is also injected into the system prompt.
```

## 代码结构

### crates/team-engine/src/soul.rs

```rust
/// Soul configuration parsed from SOUL.md
pub struct SoulConfig {
    // YAML frontmatter
    pub name: String,
    pub role: String,
    pub version: String,
    pub personality: Personality,
    pub knowledge_domains: Vec<String>,
    pub capabilities: Vec<String>,
    pub constraints: Vec<String>,
    pub tools: Vec<String>,
    // Markdown body
    pub description: String,
}

pub struct Personality {
    pub style: String,
    pub thoroughness: String,
    pub communication: String,
}

/// Soul loader with file watching
pub struct SoulLoader {
    souls_dir: PathBuf,
    cache: HashMap<String, SoulConfig>,
}

impl SoulLoader {
    /// Load soul from file
    pub fn load(&mut self, agent_id: &str) -> Result<&SoulConfig>;
    
    /// Check if file changed and reload
    pub fn reload_if_changed(&mut self, agent_id: &str) -> Result<bool>;
    
    /// Build system prompt from soul
    pub fn build_system_prompt(&self, soul: &SoulConfig) -> String;
}
```

## 集成点

1. AgentProfile 加 optional soul: Option<SoulConfig>
2. TeamEngine 启动时加载所有 SOUL.md
3. 每次 LLM 调用前注入 soul 到 system prompt
4. 文件变更检测（mtime 或 notify crate）

## 测试

1. YAML 解析测试
2. Markdown body 提取测试
3. System prompt 构建测试
4. 文件变更检测测试
