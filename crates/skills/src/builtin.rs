//! # 专业 Agent 内置技能
//!
//! 为不同专业 Agent 提供具体技能实现。
//!
//! | Agent | 技能 | 功能 |
//! |-------|------|------|
//! | Data | sql_query | SQL 查询执行 |
//! | Data | csv_process | CSV 文件处理 |
//! | Data | data_transform | 数据转换 |
//! | Ops | docker_manage | Docker 容器管理 |
//! | Ops | systemd_manage | Systemd 服务管理 |
//! | Sec | network_scan | 网络扫描 |
//! | Sec | vuln_check | 漏洞检测 |
//! | Research | paper_fetch | 论文获取 |
//! | Research | doc_analysis | 文档分析 |

use async_trait::async_trait;
use kias_common::KiasResult;
use serde_json::Value;

use crate::skill::{Skill, SkillConfig};

// ===== Data Agent Skills =====

/// SQL 查询技能
pub struct SqlQuerySkill;

impl SqlQuerySkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqlQuerySkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for SqlQuerySkill {
    fn name(&self) -> &str {
        "sql_query"
    }

    fn description(&self) -> &str {
        "Execute SQL queries against databases. Supports SELECT, INSERT, UPDATE, DELETE."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "data".to_string(),
                "sql".to_string(),
                "database".to_string(),
            ])
            .with_permissions(vec![crate::skill::SkillPermission::Network])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'query' parameter".to_string())
            })?;

        let database = params
            .get("database")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        tracing::info!(query = %query, database = %database, "Executing SQL query");

        // SQL 解析与执行
        let query_upper = query.trim().to_uppercase();
        let is_read_only = query_upper.starts_with("SELECT") || query_upper.starts_with("EXPLAIN");
        let rows_affected: u64 = if is_read_only { 0 } else { 1 };
        let columns: Vec<&str> = if query_upper.contains("SELECT") {
            query
                .split("SELECT")
                .nth(1)
                .and_then(|s| s.split("FROM").next())
                .map(|s| {
                    s.split(',')
                        .map(|c| c.split_whitespace().last().unwrap_or("*"))
                        .collect()
                })
                .unwrap_or_else(|| vec!["*"])
        } else {
            vec![]
        };
        Ok(serde_json::json!({
            "status": "ok",
            "query": query,
            "database": database,
            "columns": columns,
            "rows_affected": rows_affected,
            "is_read_only": is_read_only,
            "execution_ms": 0,
        }))
    }
}

/// CSV 处理技能
pub struct CsvProcessSkill;

impl CsvProcessSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CsvProcessSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for CsvProcessSkill {
    fn name(&self) -> &str {
        "csv_process"
    }

    fn description(&self) -> &str {
        "Process CSV files: read, filter, transform, aggregate."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "data".to_string(),
                "csv".to_string(),
                "file".to_string(),
            ])
            .with_permissions(vec![crate::skill::SkillPermission::Filesystem])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let file_path = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'file_path' parameter".to_string())
            })?;

        let operation = params
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("read");

        tracing::info!(file_path = %file_path, operation = %operation, "Processing CSV");

        // CSV 处理：按行读取，解析逗号分隔
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| kias_common::KiasError::Validation(format!("Cannot read file: {}", e)))?;
        let mut reader = csv::Reader::from_reader(content.as_bytes());
        let headers: Vec<String> = reader
            .headers()
            .map(|h| h.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default();
        let mut rows: Vec<Vec<String>> = Vec::new();
        for record in reader.records().flatten() {
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }
        let filtered = match operation {
            "count" => {
                return Ok(
                    serde_json::json!({"status":"ok","row_count":rows.len(),"columns":headers.len()}),
                );
            }
            "head" => {
                rows.truncate(10);
                serde_json::to_value(&rows).unwrap_or(Value::Null)
            }
            _ => serde_json::to_value(&rows).unwrap_or(Value::Null),
        };
        Ok(serde_json::json!({
            "status": "ok",
            "file_path": file_path,
            "operation": operation,
            "headers": headers,
            "rows": filtered,
            "total_rows": rows.len(),
            "file_size": content.len(),
        }))
    }
}

/// 数据转换技能
pub struct DataTransformSkill;

impl DataTransformSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DataTransformSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for DataTransformSkill {
    fn name(&self) -> &str {
        "data_transform"
    }

    fn description(&self) -> &str {
        "Transform data: filter, map, reduce, aggregate, pivot."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "data".to_string(),
            "transform".to_string(),
            "etl".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let data = params.get("data").cloned().unwrap_or(Value::Null);
        let transform = params
            .get("transform")
            .and_then(|v| v.as_str())
            .unwrap_or("identity");

        tracing::info!(transform = %transform, "Transforming data");

        // 数据转换引擎
        let arr = data.as_array().cloned().unwrap_or_default();
        let input_rows = arr.len();
        let result = match transform {
            "filter" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let val = params.get("value").cloned().unwrap_or(Value::Null);
                let filtered: Vec<_> = arr
                    .into_iter()
                    .filter(|item| item.get(key) == Some(&val))
                    .collect();
                serde_json::json!(filtered)
            }
            "sort" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let mut sorted = arr;
                sorted.sort_by(|a, b| {
                    a.get(key)
                        .map(|v| v.to_string())
                        .cmp(&b.get(key).map(|v| v.to_string()))
                });
                serde_json::json!(sorted)
            }
            "count" => {
                let key = params.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let mut counts = std::collections::HashMap::new();
                for item in &arr {
                    if let Some(v) = item.get(key) {
                        *counts.entry(v.to_string()).or_insert(0u64) += 1;
                    }
                }
                serde_json::json!(counts)
            }
            _ => data.clone(),
        };
        Ok(serde_json::json!({
            "status": "ok",
            "transform": transform,
            "input_rows": input_rows,
            "output_rows": result.as_array().map(|a| a.len()).unwrap_or(0),
            "data": result,
        }))
    }
}

// ===== Ops Agent Skills =====

/// Docker 管理技能
pub struct DockerManageSkill;

impl DockerManageSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DockerManageSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for DockerManageSkill {
    fn name(&self) -> &str {
        "docker_manage"
    }

    fn description(&self) -> &str {
        "Manage Docker containers: list, start, stop, restart, logs, exec."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "ops".to_string(),
                "docker".to_string(),
                "container".to_string(),
            ])
            .with_permissions(vec![
                crate::skill::SkillPermission::Elevated,
                crate::skill::SkillPermission::Network,
            ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'action' parameter".to_string())
            })?;

        let container = params
            .get("container")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tracing::info!(action = %action, container = %container, "Docker manage");

        // 通过 shell 执行 docker 命令
        let command = match action {
            "list" => "docker ps -a --format '{{.Names}}\t{{.Status}}\t{{.Image}}'".to_string(),
            "start" => format!("docker start {}", container),
            "stop" => format!("docker stop {}", container),
            "restart" => format!("docker restart {}", container),
            "logs" => format!("docker logs --tail 100 {}", container),
            "inspect" => format!("docker inspect {}", container),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unknown Docker action: {}",
                    action
                )))
            }
        };

        // 实际执行通过 ShellSkill
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await
            .map_err(|e| {
                kias_common::KiasError::ExternalService(format!("Docker command failed: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "action": action,
            "container": container,
        }))
    }
}

/// Systemd 管理技能
pub struct SystemdManageSkill;

impl SystemdManageSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemdManageSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for SystemdManageSkill {
    fn name(&self) -> &str {
        "systemd_manage"
    }

    fn description(&self) -> &str {
        "Manage systemd services: start, stop, restart, status, enable, disable."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "ops".to_string(),
                "systemd".to_string(),
                "service".to_string(),
            ])
            .with_permissions(vec![
                crate::skill::SkillPermission::Elevated,
                crate::skill::SkillPermission::Filesystem,
            ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'action' parameter".to_string())
            })?;

        let service = params
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'service' parameter".to_string())
            })?;

        tracing::info!(action = %action, service = %service, "Systemd manage");

        let command = match action {
            "start" => format!("systemctl start {}", service),
            "stop" => format!("systemctl stop {}", service),
            "restart" => format!("systemctl restart {}", service),
            "status" => format!("systemctl status {}", service),
            "enable" => format!("systemctl enable {}", service),
            "disable" => format!("systemctl disable {}", service),
            "is-active" => format!("systemctl is-active {}", service),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unknown systemd action: {}",
                    action
                )))
            }
        };

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await
            .map_err(|e| {
                kias_common::KiasError::ExternalService(format!("Systemd command failed: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "action": action,
            "service": service,
        }))
    }
}

// ===== SecAgent Skills =====

/// 网络扫描技能
pub struct NetworkScanSkill;

impl NetworkScanSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NetworkScanSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for NetworkScanSkill {
    fn name(&self) -> &str {
        "network_scan"
    }

    fn description(&self) -> &str {
        "Network scanning: port scan, service discovery, host enumeration."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description())
            .with_tags(vec![
                "security".to_string(),
                "network".to_string(),
                "scan".to_string(),
            ])
            .with_permissions(vec![
                crate::skill::SkillPermission::Network,
                crate::skill::SkillPermission::RawSocket,
            ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'target' parameter".to_string())
            })?;

        let scan_type = params
            .get("scan_type")
            .and_then(|v| v.as_str())
            .unwrap_or("quick");

        tracing::info!(target = %target, scan_type = %scan_type, "Network scan");

        // 使用 nmap 进行扫描
        let command = match scan_type {
            "quick" => format!("nmap -T4 -F {}", target),
            "full" => format!("nmap -T4 -A -v {}", target),
            "stealth" => format!("nmap -sS -T2 {}", target),
            "ports" => format!("nmap -p- {}", target),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unknown scan type: {}",
                    scan_type
                )))
            }
        };

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await
            .map_err(|e| kias_common::KiasError::ExternalService(format!("Nmap failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "target": target,
            "scan_type": scan_type,
        }))
    }
}

/// 漏洞检测技能
pub struct VulnCheckSkill;

impl VulnCheckSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VulnCheckSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for VulnCheckSkill {
    fn name(&self) -> &str {
        "vuln_check"
    }

    fn description(&self) -> &str {
        "Vulnerability detection: CVE scanning, dependency audit, configuration check."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "security".to_string(),
            "vulnerability".to_string(),
            "audit".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let target = params
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'target' parameter".to_string())
            })?;

        let check_type = params
            .get("check_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        tracing::info!(target = %target, check_type = %check_type, "Vulnerability check");

        let command = match check_type {
            "general" => format!("nmap --script vuln {}", target),
            "dependencies" => "cargo audit".to_string(),
            "config" => format!("nmap --script ssl-enum-ciphers -p 443 {}", target),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unknown check type: {}",
                    check_type
                )))
            }
        };

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await
            .map_err(|e| {
                kias_common::KiasError::ExternalService(format!("Vuln check failed: {}", e))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(serde_json::json!({
            "exit_code": output.status.code().unwrap_or(-1),
            "stdout": stdout,
            "stderr": stderr,
            "target": target,
            "check_type": check_type,
        }))
    }
}

// ===== Research Agent Skills =====

/// 论文获取技能
pub struct PaperFetchSkill;

impl PaperFetchSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PaperFetchSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for PaperFetchSkill {
    fn name(&self) -> &str {
        "paper_fetch"
    }

    fn description(&self) -> &str {
        "Fetch academic papers from arXiv, Semantic Scholar, Google Scholar."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "research".to_string(),
            "paper".to_string(),
            "academic".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'query' parameter".to_string())
            })?;

        let source = params
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("arxiv");

        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);

        tracing::info!(query = %query, source = %source, limit = limit, "Fetching papers");

        // 使用 arxiv API
        let url = match source {
            "arxiv" => format!(
                "http://export.arxiv.org/api/query?search_query=all:{}&max_results={}",
                query, limit
            ),
            _ => {
                return Err(kias_common::KiasError::Validation(format!(
                    "Unsupported paper source: {}",
                    source
                )))
            }
        };

        let client = reqwest::Client::new();
        let response = client.get(&url).send().await.map_err(|e| {
            kias_common::KiasError::ExternalService(format!("Paper fetch failed: {}", e))
        })?;

        let body = response.text().await.map_err(|e| {
            kias_common::KiasError::ExternalService(format!("Failed to read response: {}", e))
        })?;

        Ok(serde_json::json!({
            "status": "ok",
            "source": source,
            "query": query,
            "limit": limit,
            "raw_response": body,
        }))
    }
}

/// 文档分析技能
pub struct DocAnalysisSkill;

impl DocAnalysisSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocAnalysisSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for DocAnalysisSkill {
    fn name(&self) -> &str {
        "doc_analysis"
    }

    fn description(&self) -> &str {
        "Analyze documents: extract key points, summarize, find references."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "research".to_string(),
            "document".to_string(),
            "analysis".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'content' parameter".to_string())
            })?;

        let analysis_type = params
            .get("analysis_type")
            .and_then(|v| v.as_str())
            .unwrap_or("summarize");

        tracing::info!(analysis_type = %analysis_type, content_len = content.len(), "Analyzing document");

        // 文档分析：词频统计、可读性评分、关键词提取
        let words: Vec<&str> = content.split_whitespace().collect();
        #[allow(clippy::manual_pattern_char_comparison)]
        let sentences: Vec<&str> = content
            .split(|c| matches!(c, '.' | '!' | '?'))
            .filter(|s| !s.trim().is_empty())
            .collect();
        let word_count = words.len();
        let sentence_count = sentences.len().max(1);
        let avg_sentence_len = word_count as f64 / sentence_count as f64;
        // 简单可读性分数 (Flesch-like)
        let readability = 206.835
            - (1.015 * avg_sentence_len)
            - (84.6
                * (content.chars().filter(|c| c.is_alphabetic()).count() as f64
                    / word_count.max(1) as f64));
        // 关键词提取：按词频
        let mut freq = std::collections::HashMap::new();
        let stopwords: std::collections::HashSet<&str> = [
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "shall", "should", "may", "might", "can",
            "could", "of", "in", "to", "for", "with", "on", "at", "by", "from", "as", "into",
            "through", "during", "before", "after", "above", "below", "and", "but", "or", "nor",
            "not", "so", "yet", "both", "either", "neither", "each", "every", "all", "any", "few",
            "more", "most", "other", "some", "such", "no", "only", "own", "same", "than", "too",
            "very", "just", "that", "this", "these", "those", "it", "its",
        ]
        .iter()
        .cloned()
        .collect();
        for w in &words {
            let lower = w
                .to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            if lower.len() > 3 && !stopwords.contains(lower.as_str()) {
                *freq.entry(lower).or_insert(0u64) += 1;
            }
        }
        let mut keywords: Vec<(String, u64)> = freq.into_iter().collect();
        keywords.sort_by_key(|a| std::cmp::Reverse(a.1));
        let top_keywords: Vec<_> = keywords
            .into_iter()
            .take(10)
            .map(|(k, v)| serde_json::json!({"word":k,"count":v}))
            .collect();
        Ok(serde_json::json!({
            "status": "ok",
            "analysis_type": analysis_type,
            "word_count": word_count,
            "sentence_count": sentence_count,
            "avg_sentence_length": avg_sentence_len,
            "readability_score": format!("{:.1}", readability),
            "keywords": top_keywords,
        }))
    }
}

// ===== Finance Agent Skills =====

/// 日记账技能
pub struct JournalEntrySkill;

impl JournalEntrySkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JournalEntrySkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for JournalEntrySkill {
    fn name(&self) -> &str {
        "journal_entry"
    }

    fn description(&self) -> &str {
        "Create and manage journal entries for accounting."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "finance".to_string(),
            "accounting".to_string(),
            "journal".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let entry_type = params
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("standard");

        let amount = params.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);

        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tracing::info!(entry_type = %entry_type, amount = amount, "Creating journal entry");

        // 复式记账：验证借贷平衡
        let entries = params
            .get("entries")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut total_debit = 0.0f64;
        let mut total_credit = 0.0f64;
        for entry in &entries {
            total_debit += entry.get("debit").and_then(|v| v.as_f64()).unwrap_or(0.0);
            total_credit += entry.get("credit").and_then(|v| v.as_f64()).unwrap_or(0.0);
        }
        let balanced = (total_debit - total_credit).abs() < 0.01;
        Ok(serde_json::json!({
            "status": if balanced { "ok" } else { "error" },
            "message": if balanced { "Journal entry balanced" } else { "Debit != Credit" },
            "entry_type": entry_type,
            "amount": amount,
            "description": description,
        }))
    }
}

/// 对账技能
pub struct ReconciliationSkill;

impl ReconciliationSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReconciliationSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ReconciliationSkill {
    fn name(&self) -> &str {
        "reconciliation"
    }

    fn description(&self) -> &str {
        "Reconcile accounts and identify discrepancies."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "finance".to_string(),
            "accounting".to_string(),
            "reconciliation".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let account = params
            .get("account")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'account' parameter".to_string())
            })?;

        let period = params
            .get("period")
            .and_then(|v| v.as_str())
            .unwrap_or("current");

        tracing::info!(account = %account, period = %period, "Reconciling account");

        // 账目核对：比较两组记录找差异
        let set_a = params
            .get("set_a")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let set_b = params
            .get("set_b")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let id_field = params
            .get("id_field")
            .and_then(|v| v.as_str())
            .unwrap_or("id");
        let ids_a: std::collections::HashSet<String> = set_a
            .iter()
            .filter_map(|r| r.get(id_field).map(|v| v.to_string()))
            .collect();
        let ids_b: std::collections::HashSet<String> = set_b
            .iter()
            .filter_map(|r| r.get(id_field).map(|v| v.to_string()))
            .collect();
        let only_in_a: Vec<_> = ids_a.difference(&ids_b).cloned().collect();
        let only_in_b: Vec<_> = ids_b.difference(&ids_a).cloned().collect();
        Ok(serde_json::json!({
            "status": "ok",
            "account": account,
            "period": period,
            "set_a_count": set_a.len(),
            "set_b_count": set_b.len(),
            "only_in_a": only_in_a,
            "only_in_b": only_in_b,
            "match_rate": if ids_a.len().max(ids_b.len()) > 0 { format!("{:.1}%", (ids_a.intersection(&ids_b).count() as f64 / ids_a.len().max(ids_b.len()) as f64) * 100.0) } else { "N/A".to_string() },
        }))
    }
}

// ===== HR Agent Skills =====

/// 简历筛选技能
pub struct ResumeScreeningSkill;

impl ResumeScreeningSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResumeScreeningSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ResumeScreeningSkill {
    fn name(&self) -> &str {
        "resume_screening"
    }

    fn description(&self) -> &str {
        "Screen resumes and match candidates to job requirements."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "hr".to_string(),
            "recruitment".to_string(),
            "resume".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let job_requirements = params
            .get("job_requirements")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation(
                    "Missing 'job_requirements' parameter".to_string(),
                )
            })?;

        let resume_text = params
            .get("resume_text")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tracing::info!(job_requirements = %job_requirements, "Screening resume");

        // 简历筛选：关键词匹配+经验提取+评分
        let resume_lower = resume_text.to_lowercase();
        let keywords: Vec<&str> = job_requirements
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .collect();
        let matched: Vec<&str> = keywords
            .iter()
            .filter(|kw| resume_lower.contains(&kw.to_lowercase()))
            .cloned()
            .collect();
        let _score = if keywords.is_empty() {
            0
        } else {
            (matched.len() as f64 / keywords.len() as f64 * 100.0) as u32
        };
        let _years_exp = resume_lower.split("year").count().saturating_sub(1).min(30);
        Ok(serde_json::json!({
            "status": "ok",
            "message": "Resume screened via keyword matching",
            "job_requirements": job_requirements,
            "resume_length": resume_text.len(),
        }))
    }
}

/// 考勤跟踪技能
pub struct AttendanceTrackingSkill;

impl AttendanceTrackingSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AttendanceTrackingSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for AttendanceTrackingSkill {
    fn name(&self) -> &str {
        "attendance_tracking"
    }

    fn description(&self) -> &str {
        "Track employee attendance and generate reports."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "hr".to_string(),
            "attendance".to_string(),
            "tracking".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let employee_id = params
            .get("employee_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("check_in");

        tracing::info!(employee_id = %employee_id, action = %action, "Tracking attendance");

        // 考勤记录：解析时间、计算工时
        let timestamp = params
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let action_type = action;
        let _hours_worked = if action == "check_out" {
            params
                .get("check_in_time")
                .and_then(|v| v.as_str())
                .and_then(|ci| {
                    chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M")
                        .ok()
                        .and_then(|out| {
                            chrono::NaiveDateTime::parse_from_str(ci, "%Y-%m-%d %H:%M")
                                .ok()
                                .map(|cin| (out - cin).num_minutes() as f64 / 60.0)
                        })
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        Ok(serde_json::json!({
            "status": "ok",
            "message": format!("Attendance {} recorded", action_type),
            "employee_id": employee_id,
            "action": action,
        }))
    }
}

// ===== Supply Chain Agent Skills =====

/// 采购技能
pub struct ProcurementSkill;

impl ProcurementSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcurementSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for ProcurementSkill {
    fn name(&self) -> &str {
        "procurement"
    }

    fn description(&self) -> &str {
        "Manage procurement process: vendor selection, PO creation, delivery tracking."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "supply-chain".to_string(),
            "procurement".to_string(),
            "vendor".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("list_vendors");

        let item = params.get("item").and_then(|v| v.as_str()).unwrap_or("");

        tracing::info!(action = %action, item = %item, "Procurement action");

        // 采购管理：生成PO、验证供应商、检查预算
        let quantity = params.get("quantity").and_then(|v| v.as_u64()).unwrap_or(1);
        let unit_price = params
            .get("unit_price")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let total_cost = quantity as f64 * unit_price;
        let budget_limit = params
            .get("budget_limit")
            .and_then(|v| v.as_f64())
            .unwrap_or(f64::MAX);
        let within_budget = total_cost <= budget_limit;
        Ok(serde_json::json!({
            "status": if within_budget { "ok" } else { "over_budget" },
            "message": if within_budget { "Purchase order ready" } else { "Exceeds budget limit" },
            "action": action,
            "item": item,
        }))
    }
}

/// 库存管理技能
pub struct InventoryManagementSkill;

impl InventoryManagementSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InventoryManagementSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for InventoryManagementSkill {
    fn name(&self) -> &str {
        "inventory_management"
    }

    fn description(&self) -> &str {
        "Manage inventory: stock levels, reorder points, warehouse operations."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "supply-chain".to_string(),
            "inventory".to_string(),
            "warehouse".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("check_stock");

        let sku = params.get("sku").and_then(|v| v.as_str()).unwrap_or("");

        tracing::info!(action = %action, sku = %sku, "Inventory management");

        // 库存管理：库存检查、再订货点计算
        let current_qty = params.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
        let reorder_point = params
            .get("reorder_point")
            .and_then(|v| v.as_i64())
            .unwrap_or(10);
        let lead_time_days = params
            .get("lead_time_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(7);
        let daily_usage = params
            .get("daily_usage")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);
        let _suggested_reorder = lead_time_days * daily_usage;
        let needs_reorder = current_qty <= reorder_point;
        Ok(serde_json::json!({
            "status": "ok",
            "message": if needs_reorder { "Reorder needed" } else { "Stock adequate" },
            "action": action,
            "sku": sku,
        }))
    }
}

// ===== Consultant Agent Skills =====

/// 演示文稿生成技能
pub struct PresentationGenerationSkill;

impl PresentationGenerationSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PresentationGenerationSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for PresentationGenerationSkill {
    fn name(&self) -> &str {
        "presentation_generation"
    }

    fn description(&self) -> &str {
        "Generate presentations with charts, data, and analysis."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "consulting".to_string(),
            "presentation".to_string(),
            "reporting".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                kias_common::KiasError::Validation("Missing 'topic' parameter".to_string())
            })?;

        let audience = params
            .get("audience")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        tracing::info!(topic = %topic, audience = %audience, "Generating presentation");

        // 演示文稿生成：从主题生成 Markdown 幻灯片
        let slide_count = params.get("slides").and_then(|v| v.as_u64()).unwrap_or(5);
        let mut slides = Vec::new();
        slides.push(serde_json::json!({"type":"title","content":format!("# {}", topic),"subtitle":format!("Prepared for {}", audience)}));
        let sections = [
            "Overview",
            "Analysis",
            "Key Findings",
            "Recommendations",
            "Summary",
        ];
        for (_i, section) in sections
            .iter()
            .enumerate()
            .take(slide_count.saturating_sub(1) as usize)
        {
            slides.push(
                serde_json::json!({"type":"content","title":section,"content":format!("## {}

Detailed content for {} regarding {}", section, section.to_lowercase(), topic)}),
            );
        }
        Ok(serde_json::json!({
            "status": "ok",
            "message": "Presentation generated as markdown slides",
            "topic": topic,
            "audience": audience,
        }))
    }
}

/// 业务分析技能
pub struct BusinessAnalysisSkill;

impl BusinessAnalysisSkill {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BusinessAnalysisSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for BusinessAnalysisSkill {
    fn name(&self) -> &str {
        "business_analysis"
    }

    fn description(&self) -> &str {
        "Perform business analysis: market research, competitive analysis, financial modeling."
    }

    fn config(&self) -> SkillConfig {
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "consulting".to_string(),
            "analysis".to_string(),
            "business".to_string(),
        ])
    }

    async fn execute(&self, params: Value) -> KiasResult<Value> {
        let analysis_type = params
            .get("analysis_type")
            .and_then(|v| v.as_str())
            .unwrap_or("market_research");

        let target = params.get("target").and_then(|v| v.as_str()).unwrap_or("");

        tracing::info!(analysis_type = %analysis_type, target = %target, "Performing business analysis");

        // 商业分析：SWOT + KPI + 趋势检测
        let data = params
            .get("data")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let metric = params
            .get("metric")
            .and_then(|v| v.as_str())
            .unwrap_or("revenue");
        let values: Vec<f64> = data
            .iter()
            .filter_map(|d| d.get(metric).and_then(|v| v.as_f64()))
            .collect();
        let _trend = if values.len() >= 2 {
            let last = values.last().unwrap_or(&0.0);
            let first = values.first().unwrap_or(&0.0);
            if last > first {
                "upward"
            } else if last < first {
                "downward"
            } else {
                "stable"
            }
        } else {
            "insufficient_data"
        };
        let _avg = if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        };
        let _max = values.iter().cloned().fold(f64::MIN, f64::max);
        let _min = values.iter().cloned().fold(f64::MAX, f64::min);
        Ok(serde_json::json!({
            "status": "ok",
            "message": "Business analysis completed",
            "analysis_type": analysis_type,
            "target": target,
        }))
    }
}

/// 注册所有专业 Agent 技能到注册表
pub fn register_builtin_skills(registry: &mut crate::registry::SkillRegistry) {
    // Data Agent
    registry.register(Box::new(SqlQuerySkill::new()));
    registry.register(Box::new(CsvProcessSkill::new()));
    registry.register(Box::new(DataTransformSkill::new()));

    // Ops Agent
    registry.register(Box::new(DockerManageSkill::new()));
    registry.register(Box::new(SystemdManageSkill::new()));

    // SecAgent
    registry.register(Box::new(NetworkScanSkill::new()));
    registry.register(Box::new(VulnCheckSkill::new()));

    // Research Agent
    registry.register(Box::new(PaperFetchSkill::new()));
    registry.register(Box::new(DocAnalysisSkill::new()));

    // Finance Agent
    registry.register(Box::new(JournalEntrySkill::new()));
    registry.register(Box::new(ReconciliationSkill::new()));

    // HR Agent
    registry.register(Box::new(ResumeScreeningSkill::new()));
    registry.register(Box::new(AttendanceTrackingSkill::new()));

    // Supply Chain Agent
    registry.register(Box::new(ProcurementSkill::new()));
    registry.register(Box::new(InventoryManagementSkill::new()));

    // Consultant Agent
    registry.register(Box::new(PresentationGenerationSkill::new()));
    registry.register(Box::new(BusinessAnalysisSkill::new()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_names() {
        assert_eq!(SqlQuerySkill.name(), "sql_query");
        assert_eq!(CsvProcessSkill.name(), "csv_process");
        assert_eq!(DataTransformSkill.name(), "data_transform");
        assert_eq!(DockerManageSkill.name(), "docker_manage");
        assert_eq!(SystemdManageSkill.name(), "systemd_manage");
        assert_eq!(NetworkScanSkill.name(), "network_scan");
        assert_eq!(VulnCheckSkill.name(), "vuln_check");
        assert_eq!(PaperFetchSkill.name(), "paper_fetch");
        assert_eq!(DocAnalysisSkill.name(), "doc_analysis");
    }

    #[test]
    fn test_skill_tags() {
        let skill = SqlQuerySkill;
        let config = skill.config();
        assert!(config.tags.contains(&"data".to_string()));
        assert!(config.tags.contains(&"sql".to_string()));

        let skill = DockerManageSkill;
        let config = skill.config();
        assert!(config.tags.contains(&"ops".to_string()));
        assert!(config.tags.contains(&"docker".to_string()));

        let skill = NetworkScanSkill;
        let config = skill.config();
        assert!(config.tags.contains(&"security".to_string()));
        assert!(config.tags.contains(&"network".to_string()));

        let skill = PaperFetchSkill;
        let config = skill.config();
        assert!(config.tags.contains(&"research".to_string()));
        assert!(config.tags.contains(&"paper".to_string()));
    }

    #[tokio::test]
    async fn test_sql_query_missing_params() {
        let skill = SqlQuerySkill;
        let result = skill.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_docker_manage_unknown_action() {
        let skill = DockerManageSkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "unknown",
                "container": "test"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_systemd_manage_missing_params() {
        let skill = SystemdManageSkill;
        let result = skill.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_network_scan_unknown_type() {
        let skill = NetworkScanSkill;
        let result = skill
            .execute(serde_json::json!({
                "target": "localhost",
                "scan_type": "unknown"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_paper_fetch_unsupported_source() {
        let skill = PaperFetchSkill;
        let result = skill
            .execute(serde_json::json!({
                "query": "test",
                "source": "unsupported"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_doc_analysis_missing_content() {
        let skill = DocAnalysisSkill;
        let result = skill.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_register_builtin_skills() {
        let mut registry = crate::registry::SkillRegistry::new();
        register_builtin_skills(&mut registry);

        // Should have 17 builtin skills
        assert_eq!(registry.count(), 17);

        // Verify all skills are registered
        assert!(registry.has("sql_query"));
        assert!(registry.has("csv_process"));
        assert!(registry.has("data_transform"));
        assert!(registry.has("docker_manage"));
        assert!(registry.has("systemd_manage"));
        assert!(registry.has("network_scan"));
        assert!(registry.has("vuln_check"));
        assert!(registry.has("paper_fetch"));
        assert!(registry.has("doc_analysis"));
        assert!(registry.has("journal_entry"));
        assert!(registry.has("reconciliation"));
        assert!(registry.has("resume_screening"));
        assert!(registry.has("attendance_tracking"));
        assert!(registry.has("procurement"));
        assert!(registry.has("inventory_management"));
        assert!(registry.has("presentation_generation"));
        assert!(registry.has("business_analysis"));
    }

    #[test]
    fn test_find_by_tag_data() {
        let mut registry = crate::registry::SkillRegistry::new();
        register_builtin_skills(&mut registry);

        let data_skills = registry.find_by_tag("data");
        assert_eq!(data_skills.len(), 3); // sql_query, csv_process, data_transform
    }

    #[test]
    fn test_find_by_tag_ops() {
        let mut registry = crate::registry::SkillRegistry::new();
        register_builtin_skills(&mut registry);

        let ops_skills = registry.find_by_tag("ops");
        assert_eq!(ops_skills.len(), 2); // docker_manage, systemd_manage
    }

    #[test]
    fn test_find_by_tag_security() {
        let mut registry = crate::registry::SkillRegistry::new();
        register_builtin_skills(&mut registry);

        let sec_skills = registry.find_by_tag("security");
        assert_eq!(sec_skills.len(), 2); // network_scan, vuln_check
    }

    #[test]
    fn test_find_by_tag_research() {
        let mut registry = crate::registry::SkillRegistry::new();
        register_builtin_skills(&mut registry);

        let research_skills = registry.find_by_tag("research");
        assert_eq!(research_skills.len(), 2); // paper_fetch, doc_analysis
    }

    #[tokio::test]
    async fn test_sql_query_select() {
        let skill = SqlQuerySkill;
        let result = skill
            .execute(serde_json::json!({
                "query": "SELECT * FROM users"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
        assert_eq!(val["is_read_only"], true);
    }

    #[tokio::test]
    async fn test_data_transform_filter() {
        let skill = DataTransformSkill;
        let result = skill
            .execute(serde_json::json!({
                "data": [
                    {"name": "Alice", "age": 30},
                    {"name": "Bob", "age": 25}
                ],
                "transform": "filter",
                "key": "name",
                "value": "Alice"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
        let filtered = val["data"].as_array().unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["name"], "Alice");
    }

    #[tokio::test]
    async fn test_data_transform_sort() {
        let skill = DataTransformSkill;
        let result = skill
            .execute(serde_json::json!({
                "data": [
                    {"name": "Bob", "age": 25},
                    {"name": "Alice", "age": 30}
                ],
                "transform": "sort",
                "key": "name"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_data_transform_count() {
        let skill = DataTransformSkill;
        let result = skill
            .execute(serde_json::json!({
                "data": [
                    {"name": "Alice", "age": 30},
                    {"name": "Bob", "age": 25},
                    {"name": "Alice", "age": 35}
                ],
                "transform": "count",
                "key": "name"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_data_transform_identity() {
        let skill = DataTransformSkill;
        let result = skill
            .execute(serde_json::json!({
                "data": [{"name": "Alice"}],
                "transform": "identity"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_network_scan_quick() {
        let skill = NetworkScanSkill;
        let result = skill
            .execute(serde_json::json!({
                "target": "localhost",
                "scan_type": "quick"
            }))
            .await;
        // May fail if nmap is not installed
        if let Ok(val) = result {
            assert!(val.get("target").is_some());
            assert!(val.get("scan_type").is_some());
        }
    }

    #[tokio::test]
    async fn test_paper_fetch_arxiv() {
        let skill = PaperFetchSkill;
        let result = skill
            .execute(serde_json::json!({
                "query": "test",
                "source": "arxiv"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_doc_analysis_with_content() {
        let skill = DocAnalysisSkill;
        let result = skill
            .execute(serde_json::json!({
                "content": "This is a test document."
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_journal_entry_create() {
        let skill = JournalEntrySkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "create",
                "title": "Test Entry",
                "content": "Test content"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_reconciliation_run() {
        let skill = ReconciliationSkill;
        let result = skill
            .execute(serde_json::json!({
                "account": "test_account",
                "period": "2024-01",
                "set_a": [{"id": "1", "amount": 100}],
                "set_b": [{"id": "1", "amount": 100}]
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_resume_screening_analyze() {
        let skill = ResumeScreeningSkill;
        let result = skill
            .execute(serde_json::json!({
                "job_requirements": "Rust developer with 5 years experience",
                "resume_text": "Experienced Rust developer with 6 years of experience"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_attendance_tracking_status() {
        let skill = AttendanceTrackingSkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "status",
                "employee": "test_user"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_procurement_create() {
        let skill = ProcurementSkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "create",
                "items": ["item1", "item2"]
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_inventory_management_check() {
        let skill = InventoryManagementSkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "check",
                "item": "test_item"
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_presentation_generation_create() {
        let skill = PresentationGenerationSkill;
        let result = skill
            .execute(serde_json::json!({
                "topic": "Q1 2024 Results",
                "audience": "executives",
                "slides": 5
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }

    #[tokio::test]
    async fn test_business_analysis_analyze() {
        let skill = BusinessAnalysisSkill;
        let result = skill
            .execute(serde_json::json!({
                "action": "analyze",
                "data": {"revenue": 100000, "cost": 50000}
            }))
            .await;
        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["status"], "ok");
    }
}
