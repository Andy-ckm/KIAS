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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "data".to_string(),
            "sql".to_string(),
            "database".to_string(),
        ])
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

        // TODO: 实际实现需要数据库连接池
        // 目前返回占位结果
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "SQL query skill requires database connection configuration",
            "query": query,
            "database": database,
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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "data".to_string(),
            "csv".to_string(),
            "file".to_string(),
        ])
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

        // TODO: 实际实现需要 CSV 库
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "CSV process skill requires csv crate integration",
            "file_path": file_path,
            "operation": operation,
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

        // TODO: 实际实现需要数据转换引擎
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Data transform skill requires transformation engine",
            "transform": transform,
            "input_rows": data.as_array().map(|a| a.len()).unwrap_or(0),
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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "ops".to_string(),
            "docker".to_string(),
            "container".to_string(),
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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "ops".to_string(),
            "systemd".to_string(),
            "service".to_string(),
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
        SkillConfig::new(self.name(), self.description()).with_tags(vec![
            "security".to_string(),
            "network".to_string(),
            "scan".to_string(),
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

        // TODO: 实际实现需要 LLM 调用
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Document analysis requires LLM integration",
            "analysis_type": analysis_type,
            "content_length": content.len(),
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

        // TODO: 实际实现需要会计系统集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Journal entry skill requires accounting system integration",
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

        // TODO: 实际实现需要会计系统集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Reconciliation skill requires accounting system integration",
            "account": account,
            "period": period,
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

        // TODO: 实际实现需要 LLM 集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Resume screening requires LLM integration",
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

        // TODO: 实际实现需要考勤系统集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Attendance tracking requires HR system integration",
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

        // TODO: 实际实现需要 ERP 系统集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Procurement skill requires ERP system integration",
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

        // TODO: 实际实现需要 WMS 系统集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Inventory management requires WMS system integration",
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

        // TODO: 实际实现需要 LLM + 图表库集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Presentation generation requires LLM + chart library integration",
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

        // TODO: 实际实现需要 LLM + 数据源集成
        Ok(serde_json::json!({
            "status": "not_implemented",
            "message": "Business analysis requires LLM + data source integration",
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
}
