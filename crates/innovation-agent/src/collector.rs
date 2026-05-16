//! 数据源收集器 — 从 GitHub/arXiv/HackerNews 等平台收集创新数据

use crate::*;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;

/// 数据源收集器 trait
#[async_trait]
pub trait DataSourceCollector: Send + Sync {
    /// 收集数据
    async fn collect(&self, config: &DataSourceConfig) -> Result<Vec<InnovationInsight>, String>;
    
    /// 获取收集器名称
    fn name(&self) -> &str;
}

/// GitHub Trending 收集器
pub struct GitHubTrendingCollector {
    client: Client,
}

impl GitHubTrendingCollector {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl DataSourceCollector for GitHubTrendingCollector {
    async fn collect(&self, config: &DataSourceConfig) -> Result<Vec<InnovationInsight>, String> {
        let url = "https://api.github.com/search/repositories?q=stars:>1000&sort=stars&order=desc";
        
        let response = self.client
            .get(url)
            .header("User-Agent", "KIAS-Innovation-Agent")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| format!("GitHub API error: {}", e))?;

        let data: Value = response.json().await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        let mut insights = Vec::new();
        
        if let Some(items) = data["items"].as_array() {
            for (i, item) in items.iter().take(config.max_results).enumerate() {
                let name = item["name"].as_str().unwrap_or("unknown");
                let description = item["description"].as_str().unwrap_or("");
                let html_url = item["html_url"].as_str().unwrap_or("");
                let stars = item["stargazers_count"].as_u64().unwrap_or(0);
                let language = item["language"].as_str().unwrap_or("unknown");
                
                insights.push(InnovationInsight {
                    id: format!("github-{}", i),
                    title: format!("{} (⭐ {})", name, stars),
                    description: description.to_string(),
                    source: "GitHub Trending".to_string(),
                    url: html_url.to_string(),
                    technologies: vec![language.to_string()],
                    innovation_score: (stars as f64 / 10000.0).min(1.0),
                    relevance_score: 0.5, // 需要进一步分析
                    discovered_at: chrono::Utc::now(),
                    tags: vec!["github".to_string(), "trending".to_string()],
                });
            }
        }

        Ok(insights)
    }

    fn name(&self) -> &str {
        "GitHub Trending"
    }
}

/// arXiv 收集器
pub struct ArXivCollector {
    client: Client,
}

impl ArXivCollector {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl DataSourceCollector for ArXivCollector {
    async fn collect(&self, config: &DataSourceConfig) -> Result<Vec<InnovationInsight>, String> {
        let category = config.params.get("category").unwrap_or(&"cs.AI".to_string()).clone();
        let url = format!(
            "http://export.arxiv.org/api/query?search_query=cat:{}&max_results={}",
            category, config.max_results
        );
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("arXiv API error: {}", e))?;

        let xml = response.text().await
            .map_err(|e| format!("Response read error: {}", e))?;

        let mut insights = Vec::new();
        
        // 简单XML解析（生产环境应使用XML库）
        for (i, entry) in xml.split("<entry>").skip(1).enumerate() {
            let title = extract_xml_value(entry, "title");
            let summary = extract_xml_value(entry, "summary");
            let id = extract_xml_value(entry, "id");
            
            if !title.is_empty() {
                insights.push(InnovationInsight {
                    id: format!("arxiv-{}", i),
                    title: title.trim().to_string(),
                    description: summary.trim().to_string(),
                    source: "arXiv".to_string(),
                    url: id.trim().to_string(),
                    technologies: vec![category.clone()],
                    innovation_score: 0.7, // arXiv论文通常有较高创新性
                    relevance_score: 0.6,
                    discovered_at: chrono::Utc::now(),
                    tags: vec!["arxiv".to_string(), "research".to_string()],
                });
            }
        }

        Ok(insights)
    }

    fn name(&self) -> &str {
        "arXiv"
    }
}

/// HackerNews 收集器
pub struct HackerNewsCollector {
    client: Client,
}

impl HackerNewsCollector {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait]
impl DataSourceCollector for HackerNewsCollector {
    async fn collect(&self, config: &DataSourceConfig) -> Result<Vec<InnovationInsight>, String> {
        let url = "https://hacker-news.firebaseio.com/v0/topstories.json";
        
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("HackerNews API error: {}", e))?;

        let story_ids: Vec<u64> = response.json().await
            .map_err(|e| format!("JSON parse error: {}", e))?;

        let mut insights = Vec::new();
        
        for (i, story_id) in story_ids.iter().take(config.max_results).enumerate() {
            let story_url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", story_id);
            
            if let Ok(story_response) = self.client.get(&story_url).send().await {
                if let Ok(story) = story_response.json::<Value>().await {
                    let title = story["title"].as_str().unwrap_or("unknown");
                    let url = story["url"].as_str().unwrap_or("");
                    let score = story["score"].as_u64().unwrap_or(0);
                    
                    insights.push(InnovationInsight {
                        id: format!("hn-{}", i),
                        title: title.to_string(),
                        description: format!("HackerNews score: {}", score),
                        source: "HackerNews".to_string(),
                        url: url.to_string(),
                        technologies: Vec::new(),
                        innovation_score: (score as f64 / 1000.0).min(1.0),
                        relevance_score: 0.4,
                        discovered_at: chrono::Utc::now(),
                        tags: vec!["hackernews".to_string(), "tech".to_string()],
                    });
                }
            }
        }

        Ok(insights)
    }

    fn name(&self) -> &str {
        "HackerNews"
    }
}

/// 收集器工厂
pub struct CollectorFactory;

impl CollectorFactory {
    /// 根据数据源类型创建收集器
    pub fn create(source_type: &DataSourceType) -> Box<dyn DataSourceCollector> {
        match source_type {
            DataSourceType::GitHubTrending | DataSourceType::GitHubSearch => {
                Box::new(GitHubTrendingCollector::new())
            }
            DataSourceType::ArXiv => Box::new(ArXivCollector::new()),
            DataSourceType::HackerNews => Box::new(HackerNewsCollector::new()),
            _ => Box::new(HackerNewsCollector::new()), // 默认
        }
    }
}

/// 辅助函数：从XML中提取值
fn extract_xml_value(xml: &str, tag: &str) -> String {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    
    if let Some(start) = xml.find(&start_tag) {
        let content_start = start + start_tag.len();
        if let Some(end) = xml[content_start..].find(&end_tag) {
            return xml[content_start..content_start + end].to_string();
        }
    }
    
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_value() {
        let xml = "<title>Test Title</title><summary>Test Summary</summary>";
        assert_eq!(extract_xml_value(xml, "title"), "Test Title");
        assert_eq!(extract_xml_value(xml, "summary"), "Test Summary");
        assert_eq!(extract_xml_value(xml, "missing"), "");
    }

    #[test]
    fn test_collector_factory() {
        let collector = CollectorFactory::create(&DataSourceType::GitHubTrending);
        assert_eq!(collector.name(), "GitHub Trending");

        let collector = CollectorFactory::create(&DataSourceType::ArXiv);
        assert_eq!(collector.name(), "arXiv");

        let collector = CollectorFactory::create(&DataSourceType::HackerNews);
        assert_eq!(collector.name(), "HackerNews");
    }
}
