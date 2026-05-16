//! 创新Agent — 多源数据收集 + 可配置 + 可开关
//!
//! 参考 OpenHuman 的创新发现机制，从 GitHub/arXiv/HackerNews 等平台
//! 自动发现新技术趋势和创新点，为 KIAS 系统提供持续创新动力。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 数据源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// 数据源名称
    pub name: String,
    /// 数据源类型
    pub source_type: DataSourceType,
    /// 是否启用
    pub enabled: bool,
    /// 刷新间隔（分钟）
    pub refresh_interval_minutes: u32,
    /// 最大结果数
    pub max_results: usize,
    /// 自定义参数
    pub params: HashMap<String, String>,
}

/// 数据源类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceType {
    /// GitHub Trending
    GitHubTrending,
    /// GitHub Search
    GitHubSearch,
    /// arXiv
    ArXiv,
    /// HackerNews
    HackerNews,
    /// ProductHunt
    ProductHunt,
    /// 自定义RSS
    CustomRss,
}

/// 创新洞察
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationInsight {
    /// 洞察ID
    pub id: String,
    /// 标题
    pub title: String,
    /// 描述
    pub description: String,
    /// 来源
    pub source: String,
    /// 来源URL
    pub url: String,
    /// 相关技术
    pub technologies: Vec<String>,
    /// 创新分数 (0.0-1.0)
    pub innovation_score: f64,
    /// 与KIAS的相关性 (0.0-1.0)
    pub relevance_score: f64,
    /// 发现时间
    pub discovered_at: chrono::DateTime<chrono::Utc>,
    /// 标签
    pub tags: Vec<String>,
}

/// 创新Agent配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnovationAgentConfig {
    /// 是否启用创新Agent
    pub enabled: bool,
    /// 数据源列表
    pub data_sources: Vec<DataSourceConfig>,
    /// 创新分数阈值
    pub innovation_threshold: f64,
    /// 相关性阈值
    pub relevance_threshold: f64,
    /// 最大并发收集数
    pub max_concurrent_collections: usize,
    /// 输出格式
    pub output_format: OutputFormat,
}

/// 输出格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    /// Markdown报告
    Markdown,
    /// JSON数据
    Json,
    /// 结构化洞察
    Structured,
}

impl Default for InnovationAgentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            data_sources: vec![
                DataSourceConfig {
                    name: "GitHub Trending".to_string(),
                    source_type: DataSourceType::GitHubTrending,
                    enabled: true,
                    refresh_interval_minutes: 60,
                    max_results: 20,
                    params: HashMap::new(),
                },
                DataSourceConfig {
                    name: "arXiv AI".to_string(),
                    source_type: DataSourceType::ArXiv,
                    enabled: true,
                    refresh_interval_minutes: 120,
                    max_results: 10,
                    params: {
                        let mut p = HashMap::new();
                        p.insert("category".to_string(), "cs.AI".to_string());
                        p
                    },
                },
                DataSourceConfig {
                    name: "HackerNews".to_string(),
                    source_type: DataSourceType::HackerNews,
                    enabled: true,
                    refresh_interval_minutes: 30,
                    max_results: 15,
                    params: HashMap::new(),
                },
            ],
            innovation_threshold: 0.6,
            relevance_threshold: 0.5,
            max_concurrent_collections: 3,
            output_format: OutputFormat::Markdown,
        }
    }
}

/// 创新Agent
pub struct InnovationAgent {
    config: InnovationAgentConfig,
    insights: Vec<InnovationInsight>,
}

impl InnovationAgent {
    /// 创建新的创新Agent
    pub fn new(config: InnovationAgentConfig) -> Self {
        Self {
            config,
            insights: Vec::new(),
        }
    }

    /// 检查是否启用
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// 获取配置
    pub fn config(&self) -> &InnovationAgentConfig {
        &self.config
    }

    /// 获取已发现的洞察
    pub fn insights(&self) -> &[InnovationInsight] {
        &self.insights
    }

    /// 添加洞察
    pub fn add_insight(&mut self, insight: InnovationInsight) {
        self.insights.push(insight);
    }

    /// 过滤高价值洞察
    pub fn high_value_insights(&self) -> Vec<&InnovationInsight> {
        self.insights
            .iter()
            .filter(|i| {
                i.innovation_score >= self.config.innovation_threshold
                    && i.relevance_score >= self.config.relevance_threshold
            })
            .collect()
    }

    /// 生成Markdown报告
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# KIAS 创新洞察报告\n\n");
        report.push_str(&format!(
            "生成时间: {}\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        let high_value = self.high_value_insights();
        report.push_str(&format!("## 高价值洞察 ({} 个)\n\n", high_value.len()));

        for insight in &high_value {
            report.push_str(&format!("### {}\n", insight.title));
            report.push_str(&format!("**来源:** {}\n", insight.source));
            report.push_str(&format!("**URL:** {}\n", insight.url));
            report.push_str(&format!("**创新分数:** {:.2}\n", insight.innovation_score));
            report.push_str(&format!("**相关性:** {:.2}\n", insight.relevance_score));
            report.push_str(&format!(
                "**技术栈:** {}\n",
                insight.technologies.join(", ")
            ));
            report.push_str(&format!("**标签:** {}\n\n", insight.tags.join(", ")));
            report.push_str(&format!("{}\n\n", insight.description));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_innovation_agent_creation() {
        let config = InnovationAgentConfig::default();
        let agent = InnovationAgent::new(config);
        assert!(agent.is_enabled());
        assert_eq!(agent.insights().len(), 0);
    }

    #[test]
    fn test_high_value_insights() {
        let config = InnovationAgentConfig::default();
        let mut agent = InnovationAgent::new(config);

        // 添加低价值洞察
        agent.add_insight(InnovationInsight {
            id: "1".to_string(),
            title: "Low Value".to_string(),
            description: "Test".to_string(),
            source: "GitHub".to_string(),
            url: "https://github.com".to_string(),
            technologies: vec!["Rust".to_string()],
            innovation_score: 0.3,
            relevance_score: 0.2,
            discovered_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
        });

        // 添加高价值洞察
        agent.add_insight(InnovationInsight {
            id: "2".to_string(),
            title: "High Value".to_string(),
            description: "Test".to_string(),
            source: "arXiv".to_string(),
            url: "https://arxiv.org".to_string(),
            technologies: vec!["AI".to_string()],
            innovation_score: 0.8,
            relevance_score: 0.7,
            discovered_at: chrono::Utc::now(),
            tags: vec!["innovation".to_string()],
        });

        let high_value = agent.high_value_insights();
        assert_eq!(high_value.len(), 1);
        assert_eq!(high_value[0].title, "High Value");
    }

    #[test]
    fn test_generate_report() {
        let config = InnovationAgentConfig::default();
        let mut agent = InnovationAgent::new(config);

        agent.add_insight(InnovationInsight {
            id: "1".to_string(),
            title: "Test Innovation".to_string(),
            description: "A test innovation".to_string(),
            source: "GitHub".to_string(),
            url: "https://github.com".to_string(),
            technologies: vec!["Rust".to_string(), "AI".to_string()],
            innovation_score: 0.9,
            relevance_score: 0.8,
            discovered_at: chrono::Utc::now(),
            tags: vec!["test".to_string()],
        });

        let report = agent.generate_report();
        assert!(report.contains("KIAS 创新洞察报告"));
        assert!(report.contains("Test Innovation"));
    }
}
