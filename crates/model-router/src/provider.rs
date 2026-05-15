//! LLM provider trait and implementations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{RouterError, RouterResult};
use crate::types::*;

// ---------------------------------------------------------------------------
// Provider Configuration
// ---------------------------------------------------------------------------

/// Configuration for an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider name (unique identifier).
    pub name: String,
    /// Provider type (openai, anthropic, deepseek, qwen, ollama).
    pub provider_type: String,
    /// API endpoint URL.
    pub endpoint: String,
    /// API key (optional for local providers).
    pub api_key: Option<String>,
    /// Supported models.
    pub models: Vec<String>,
    /// Maximum concurrent requests.
    pub max_concurrency: u32,
    /// Request timeout (seconds).
    pub timeout_secs: u64,
    /// Priority (higher = preferred).
    pub priority: u32,
    /// Weight for weighted random routing.
    pub weight: f64,
    /// Custom headers.
    pub headers: HashMap<String, String>,
    /// Rate limit (requests per minute).
    pub rate_limit_rpm: Option<u32>,
    /// Monthly budget (USD).
    pub monthly_budget: Option<f64>,
}

impl ProviderConfig {
    /// Create a new provider configuration.
    pub fn new(name: &str, provider_type: &str, endpoint: &str, models: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            provider_type: provider_type.to_string(),
            endpoint: endpoint.to_string(),
            api_key: None,
            models,
            max_concurrency: 10,
            timeout_secs: 30,
            priority: 0,
            weight: 1.0,
            headers: HashMap::new(),
            rate_limit_rpm: None,
            monthly_budget: None,
        }
    }

    /// Create an OpenAI-compatible provider configuration.
    pub fn openai(name: &str, api_key: &str, models: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            provider_type: "openai".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            api_key: Some(api_key.to_string()),
            models,
            max_concurrency: 10,
            timeout_secs: 60,
            priority: 0,
            weight: 1.0,
            headers: HashMap::new(),
            rate_limit_rpm: Some(60),
            monthly_budget: None,
        }
    }

    /// Create an Anthropic provider configuration.
    pub fn anthropic(name: &str, api_key: &str, models: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            provider_type: "anthropic".to_string(),
            endpoint: "https://api.anthropic.com/v1".to_string(),
            api_key: Some(api_key.to_string()),
            models,
            max_concurrency: 5,
            timeout_secs: 120,
            priority: 0,
            weight: 1.0,
            headers: HashMap::new(),
            rate_limit_rpm: Some(50),
            monthly_budget: None,
        }
    }

    /// Create an Ollama provider configuration.
    pub fn ollama(name: &str, endpoint: &str, models: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            provider_type: "ollama".to_string(),
            endpoint: endpoint.to_string(),
            api_key: None,
            models,
            max_concurrency: 4,
            timeout_secs: 300,
            priority: 0,
            weight: 1.0,
            headers: HashMap::new(),
            rate_limit_rpm: None,
            monthly_budget: None,
        }
    }

    /// Set the API key.
    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Set the weight.
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// Set the monthly budget.
    pub fn with_budget(mut self, budget: f64) -> Self {
        self.monthly_budget = Some(budget);
        self
    }
}

// ---------------------------------------------------------------------------
// Provider Health Tracker
// ---------------------------------------------------------------------------

/// Tracks provider health metrics.
#[derive(Debug, Clone)]
pub struct HealthTracker {
    /// Total requests.
    pub total_requests: u64,
    /// Successful requests.
    pub success_requests: u64,
    /// Failed requests.
    pub failed_requests: u64,
    /// Total latency (for average calculation).
    pub total_latency_ms: u64,
    /// Last error message.
    pub last_error: Option<String>,
    /// Last success time.
    pub last_success: Option<Instant>,
    /// Last failure time.
    pub last_failure: Option<Instant>,
}

impl Default for HealthTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthTracker {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            success_requests: 0,
            failed_requests: 0,
            total_latency_ms: 0,
            last_error: None,
            last_success: None,
            last_failure: None,
        }
    }

    pub fn record_success(&mut self, latency_ms: u64) {
        self.total_requests += 1;
        self.success_requests += 1;
        self.total_latency_ms += latency_ms;
        self.last_success = Some(Instant::now());
    }

    pub fn record_failure(&mut self, error: String) {
        self.total_requests += 1;
        self.failed_requests += 1;
        self.last_error = Some(error);
        self.last_failure = Some(Instant::now());
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 1.0;
        }
        self.success_requests as f64 / self.total_requests as f64
    }

    pub fn avg_latency_ms(&self) -> u64 {
        if self.success_requests == 0 {
            return 0;
        }
        self.total_latency_ms / self.success_requests
    }

    pub fn is_healthy(&self) -> bool {
        // Consider unhealthy if:
        // 1. Success rate < 50% AND has at least 5 requests
        // 2. Last 3 requests all failed
        if self.total_requests >= 5 && self.success_rate() < 0.5 {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Provider Trait
// ---------------------------------------------------------------------------

/// Trait for LLM providers.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get provider name.
    fn name(&self) -> &str;

    /// Get provider configuration.
    fn config(&self) -> &ProviderConfig;

    /// Get supported models.
    fn supported_models(&self) -> &[String];

    /// Check if provider supports a model.
    fn supports_model(&self, model: &str) -> bool {
        self.supported_models().iter().any(|m| m == model)
    }

    /// Get health status.
    async fn health(&self) -> ProviderHealth;

    /// Execute a chat completion request.
    async fn chat(&self, request: &ChatRequest) -> RouterResult<ChatResponse>;

    /// Execute an embedding request.
    async fn embedding(&self, request: &EmbeddingRequest) -> RouterResult<EmbeddingResponse>;

    /// Get current cost (for budget tracking).
    async fn current_cost(&self) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------
// OpenAI-Compatible Provider
// ---------------------------------------------------------------------------

/// OpenAI-compatible provider implementation.
pub struct OpenAICompatibleProvider {
    config: ProviderConfig,
    client: reqwest::Client,
    health: Arc<RwLock<HealthTracker>>,
    current_cost: Arc<RwLock<f64>>,
}

impl OpenAICompatibleProvider {
    pub fn new(config: ProviderConfig) -> RouterResult<Self> {
        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(ref api_key) = config.api_key {
            headers.insert(
                "Authorization",
                format!("Bearer {}", api_key).parse().map_err(|_| {
                    RouterError::InvalidRequest("Invalid API key format".to_string())
                })?,
            );
        }

        for (key, value) in &config.headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|_| RouterError::InvalidRequest(format!("Invalid header: {}", key)))?,
                value.parse().map_err(|_| {
                    RouterError::InvalidRequest(format!("Invalid header value: {}", value))
                })?,
            );
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .default_headers(headers)
            .build()
            .map_err(|e| RouterError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            health: Arc::new(RwLock::new(HealthTracker::new())),
            current_cost: Arc::new(RwLock::new(0.0)),
        })
    }
}

#[async_trait]
impl Provider for OpenAICompatibleProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn config(&self) -> &ProviderConfig {
        &self.config
    }

    fn supported_models(&self) -> &[String] {
        &self.config.models
    }

    async fn health(&self) -> ProviderHealth {
        let health = self.health.read().await;
        ProviderHealth {
            healthy: health.is_healthy(),
            success_rate: health.success_rate(),
            avg_latency_ms: health.avg_latency_ms(),
            total_requests: health.total_requests,
            failed_requests: health.failed_requests,
            last_error: health.last_error.clone(),
            last_check: SystemTime::now(),
        }
    }

    async fn chat(&self, request: &ChatRequest) -> RouterResult<ChatResponse> {
        let start = Instant::now();

        // Build request body
        let body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "top_p": request.top_p,
            "stop": request.stop,
            "stream": false,
            "tools": request.tools,
        });

        // Send request
        let url = format!("{}/chat/completions", self.config.endpoint);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if let Ok(mut health) = self.health.try_write() {
                    health.record_failure(e.to_string());
                }
                RouterError::ProviderError {
                    provider: self.config.name.clone(),
                    message: e.to_string(),
                }
            })?;

        let latency_ms = start.elapsed().as_millis() as u64;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if let Ok(mut health) = self.health.try_write() {
                health.record_failure(format!("HTTP {}: {}", status, body));
            }
            return Err(RouterError::ProviderError {
                provider: self.config.name.clone(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        // Parse response
        let json: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| RouterError::ProviderError {
                    provider: self.config.name.clone(),
                    message: format!("Failed to parse response: {}", e),
                })?;

        let usage = Usage {
            prompt_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: json["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        // Calculate cost
        let _model_info = self.config.models.iter().find(|m| *m == &request.model);
        let cost_per_million_input = 0.0; // Would be looked up from model registry
        let cost_per_million_output = 0.0;
        let cost = (usage.prompt_tokens as f64 * cost_per_million_input
            + usage.completion_tokens as f64 * cost_per_million_output)
            / 1_000_000.0;

        // Update cost tracking
        {
            let mut current_cost = self.current_cost.write().await;
            *current_cost += cost;
        }

        // Record success
        {
            let mut health = self.health.write().await;
            health.record_success(latency_ms);
        }

        let choices = json["choices"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
            .map(|(i, choice)| ChatChoice {
                index: i as u32,
                message: ChatMessage {
                    role: choice["message"]["role"]
                        .as_str()
                        .unwrap_or("assistant")
                        .to_string(),
                    content: choice["message"]["content"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    tool_call_id: None,
                    tool_calls: choice["message"]["tool_calls"]
                        .as_array()
                        .map(|tc| tc.to_vec()),
                },
                finish_reason: choice["finish_reason"].as_str().map(|s| s.to_string()),
            })
            .collect();

        Ok(ChatResponse {
            id: json["id"].as_str().unwrap_or("").to_string(),
            model: request.model.clone(),
            provider: self.config.name.clone(),
            choices,
            usage,
            latency_ms,
            cost_usd: cost,
        })
    }

    async fn embedding(&self, request: &EmbeddingRequest) -> RouterResult<EmbeddingResponse> {
        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
        });

        let url = format!("{}/embeddings", self.config.endpoint);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| RouterError::ProviderError {
                provider: self.config.name.clone(),
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(RouterError::ProviderError {
                provider: self.config.name.clone(),
                message: format!("HTTP {}: {}", status, body),
            });
        }

        let json: serde_json::Value =
            response
                .json()
                .await
                .map_err(|e| RouterError::ProviderError {
                    provider: self.config.name.clone(),
                    message: format!("Failed to parse response: {}", e),
                })?;

        let embeddings = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|d| {
                d["embedding"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                    .collect()
            })
            .collect();

        let usage = Usage {
            prompt_tokens: json["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: 0,
            total_tokens: json["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(EmbeddingResponse {
            model: request.model.clone(),
            provider: self.config.name.clone(),
            embeddings,
            usage,
            cost_usd: 0.0,
        })
    }

    async fn current_cost(&self) -> f64 {
        let cost = self.current_cost.read().await;
        *cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_openai() {
        let config = ProviderConfig::openai(
            "openai",
            "sk-test",
            vec!["gpt-4".to_string(), "gpt-3.5-turbo".to_string()],
        );

        assert_eq!(config.name, "openai");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.models.len(), 2);
    }

    #[test]
    fn test_provider_config_anthropic() {
        let config = ProviderConfig::anthropic(
            "anthropic",
            "sk-ant-test",
            vec!["claude-3-opus".to_string()],
        );

        assert_eq!(config.name, "anthropic");
        assert_eq!(config.provider_type, "anthropic");
    }

    #[test]
    fn test_provider_config_ollama() {
        let config = ProviderConfig::ollama(
            "ollama",
            "http://localhost:11434",
            vec!["llama3".to_string()],
        );

        assert_eq!(config.name, "ollama");
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_health_tracker() {
        let mut tracker = HealthTracker::new();
        assert!(tracker.is_healthy());

        tracker.record_success(100);
        tracker.record_success(200);
        tracker.record_failure("error".to_string());

        assert_eq!(tracker.total_requests, 3);
        assert_eq!(tracker.success_requests, 2);
        assert_eq!(tracker.failed_requests, 1);
        assert!((tracker.success_rate() - 0.666).abs() < 0.01);
        assert_eq!(tracker.avg_latency_ms(), 150);
    }

    #[test]
    fn test_health_tracker_unhealthy() {
        let mut tracker = HealthTracker::new();

        for _ in 0..5 {
            tracker.record_failure("error".to_string());
        }

        assert!(!tracker.is_healthy());
    }

    #[test]
    fn test_routing_strategy_default() {
        let strategy = RoutingStrategy::default();
        assert_eq!(strategy, RoutingStrategy::RoundRobin);
    }
}
