//! Local model server support (Ollama, vLLM, llama.cpp, LocalAI, TGI).
//!
//! Provides unified interface for local LLM inference servers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported local model servers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LocalServerType {
    /// Ollama - easy local model management
    Ollama,
    /// vLLM - high-throughput serving
    Vllm,
    /// LlamaCpp - llama.cpp server
    LlamaCpp,
    /// LocalAI - OpenAI-compatible local API
    LocalAi,
    /// Text Generation Inference (HuggingFace)
    Tgi,
    /// Custom OpenAI-compatible endpoint
    Custom,
}

impl std::fmt::Display for LocalServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ollama => write!(f, "ollama"),
            Self::Vllm => write!(f, "vllm"),
            Self::LlamaCpp => write!(f, "llama.cpp"),
            Self::LocalAi => write!(f, "localai"),
            Self::Tgi => write!(f, "tgi"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Configuration for a local model server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelConfig {
    /// Server type.
    pub server_type: LocalServerType,
    /// Server endpoint (e.g., "http://localhost:11434").
    pub endpoint: String,
    /// Model name/ID (e.g., "llama3.1:8b", "meta-llama/Llama-3.1-8B-Instruct").
    pub model: String,
    /// Display name.
    pub display_name: Option<String>,
    /// API key (optional, some servers support auth).
    pub api_key: Option<String>,
    /// Custom headers.
    pub headers: HashMap<String, String>,
    /// Max concurrent requests.
    pub max_concurrency: u32,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Enable streaming.
    pub stream: bool,
    /// Model parameters.
    pub model_params: ModelParams,
}

/// Model inference parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParams {
    /// Temperature (0.0 - 2.0).
    pub temperature: Option<f64>,
    /// Top-p sampling.
    pub top_p: Option<f64>,
    /// Top-k sampling.
    pub top_k: Option<u32>,
    /// Max tokens to generate.
    pub max_tokens: Option<u32>,
    /// Repeat penalty.
    pub repeat_penalty: Option<f64>,
    /// Stop sequences.
    pub stop: Option<Vec<String>>,
    /// System prompt.
    pub system_prompt: Option<String>,
    /// Custom parameters (server-specific).
    pub custom: HashMap<String, serde_json::Value>,
}

impl Default for ModelParams {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: None,
            max_tokens: Some(2048),
            repeat_penalty: None,
            stop: None,
            system_prompt: None,
            custom: HashMap::new(),
        }
    }
}

impl LocalModelConfig {
    /// Create an Ollama config.
    pub fn ollama(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::Ollama,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("Ollama/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 4,
            timeout_secs: 300,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Create a vLLM config.
    pub fn vllm(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::Vllm,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("vLLM/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 8,
            timeout_secs: 120,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Create a llama.cpp server config.
    pub fn llama_cpp(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::LlamaCpp,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("llama.cpp/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 2,
            timeout_secs: 300,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Create a LocalAI config.
    pub fn localai(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::LocalAi,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("LocalAI/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 4,
            timeout_secs: 120,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Create a TGI config.
    pub fn tgi(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::Tgi,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("TGI/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 4,
            timeout_secs: 120,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Create a custom OpenAI-compatible config.
    pub fn custom(endpoint: &str, model: &str) -> Self {
        Self {
            server_type: LocalServerType::Custom,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            display_name: Some(format!("Custom/{}", model)),
            api_key: None,
            headers: HashMap::new(),
            max_concurrency: 4,
            timeout_secs: 120,
            stream: true,
            model_params: ModelParams::default(),
        }
    }

    /// Set display name.
    pub fn with_display_name(mut self, name: &str) -> Self {
        self.display_name = Some(name.to_string());
        self
    }

    /// Set API key.
    pub fn with_api_key(mut self, key: &str) -> Self {
        self.api_key = Some(key.to_string());
        self
    }

    /// Set max concurrency.
    pub fn with_concurrency(mut self, max: u32) -> Self {
        self.max_concurrency = max;
        self
    }

    /// Set timeout.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set temperature.
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.model_params.temperature = Some(temp);
        self
    }

    /// Set max tokens.
    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.model_params.max_tokens = Some(tokens);
        self
    }

    /// Set system prompt.
    pub fn with_system_prompt(mut self, prompt: &str) -> Self {
        self.model_params.system_prompt = Some(prompt.to_string());
        self
    }
}

/// Health check result for a local model server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalServerHealth {
    /// Server is reachable.
    pub reachable: bool,
    /// Server type detected.
    pub server_type: Option<LocalServerType>,
    /// Available models.
    pub models: Vec<String>,
    /// GPU info (if available).
    pub gpu_info: Option<GpuInfo>,
    /// Server version.
    pub version: Option<String>,
    /// Response time in milliseconds.
    pub latency_ms: u64,
}

/// GPU information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// GPU name.
    pub name: String,
    /// Total VRAM in MB.
    pub vram_total_mb: u64,
    /// Used VRAM in MB.
    pub vram_used_mb: u64,
    /// GPU utilization percentage.
    pub utilization: f64,
}

/// Model information from local server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelInfo {
    /// Model ID/name.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Model size in bytes.
    pub size_bytes: Option<u64>,
    /// Quantization level (e.g., "Q4_K_M", "FP16").
    pub quantization: Option<String>,
    /// Parameter count (e.g., "7B", "13B").
    pub parameters: Option<String>,
    /// Context length.
    pub context_length: Option<u32>,
    /// Model is loaded in memory.
    pub loaded: bool,
}

/// Auto-detect local model server type.
pub async fn detect_server(endpoint: &str) -> Option<LocalServerType> {
    let client = reqwest::Client::new();

    // Try Ollama
    if client
        .get(format!("{}/api/tags", endpoint))
        .send()
        .await
        .ok()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        return Some(LocalServerType::Ollama);
    }

    // Try vLLM (OpenAI-compatible)
    if client
        .get(format!("{}/v1/models", endpoint))
        .send()
        .await
        .ok()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        return Some(LocalServerType::Vllm);
    }

    // Try llama.cpp
    if client
        .get(format!("{}/health", endpoint))
        .send()
        .await
        .ok()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        return Some(LocalServerType::LlamaCpp);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_model_config_ollama() {
        let config = LocalModelConfig::ollama("http://localhost:11434", "llama3.1:8b");
        assert_eq!(config.server_type, LocalServerType::Ollama);
        assert_eq!(config.endpoint, "http://localhost:11434");
        assert_eq!(config.model, "llama3.1:8b");
        assert!(config.stream);
    }

    #[test]
    fn test_local_model_config_vllm() {
        let config =
            LocalModelConfig::vllm("http://localhost:8000", "meta-llama/Llama-3.1-8B-Instruct")
                .with_temperature(0.5)
                .with_max_tokens(4096);
        assert_eq!(config.server_type, LocalServerType::Vllm);
        assert_eq!(config.model_params.temperature, Some(0.5));
        assert_eq!(config.model_params.max_tokens, Some(4096));
    }

    #[test]
    fn test_local_model_config_llama_cpp() {
        let config =
            LocalModelConfig::llama_cpp("http://localhost:8080", "model.gguf").with_concurrency(1);
        assert_eq!(config.server_type, LocalServerType::LlamaCpp);
        assert_eq!(config.max_concurrency, 1);
    }

    #[test]
    fn test_model_params_default() {
        let params = ModelParams::default();
        assert_eq!(params.temperature, Some(0.7));
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.max_tokens, Some(2048));
    }

    #[test]
    fn test_server_type_display() {
        assert_eq!(LocalServerType::Ollama.to_string(), "ollama");
        assert_eq!(LocalServerType::Vllm.to_string(), "vllm");
        assert_eq!(LocalServerType::LlamaCpp.to_string(), "llama.cpp");
    }
}
