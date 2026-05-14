use super::state::Task;
use async_trait::async_trait;
use kias_common::KiasResult;

/// Worker - 执行面（借鉴 MiniMax 设计）
///
/// 职责：
/// 1. 具体执行任务
/// 2. 不同 Worker 有不同工具、上下文、输出要求
/// 3. 专业化角色：检索、写代码、生成文档、处理表格
#[async_trait]
pub trait Worker: Send + Sync {
    /// 执行任务
    async fn execute(&self, task: &Task) -> KiasResult<String>;

    /// 获取 Worker 名称
    fn name(&self) -> &str;

    /// 获取 Worker 能力描述
    fn capabilities(&self) -> Vec<String>;
}

pub struct CodeWorker {
    name: String,
    workdir: Option<String>,
}

impl CodeWorker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            workdir: None,
        }
    }

    pub fn with_workdir(mut self, workdir: &str) -> Self {
        self.workdir = Some(workdir.to_string());
        self
    }
}

#[async_trait]
impl Worker for CodeWorker {
    async fn execute(&self, task: &Task) -> KiasResult<String> {
        tracing::info!(worker = %self.name, task_id = %task.id, "Executing code task");

        // Extract command from task context
        let command = task
            .context
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("echo 'no command specified'");

        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command);

        if let Some(ref workdir) = self.workdir {
            cmd.current_dir(workdir);
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let exit_code = out.status.code().unwrap_or(-1);

                if exit_code == 0 {
                    Ok(format!("{}\n{}", stdout, stderr).trim().to_string())
                } else {
                    Ok(format!(
                        "EXIT_CODE={}\nSTDOUT:\n{}\nSTDERR:\n{}",
                        exit_code, stdout, stderr
                    ))
                }
            }
            Err(e) => Ok(format!("EXECUTION_ERROR: {}", e)),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "code_generation".to_string(),
            "code_review".to_string(),
            "shell_execution".to_string(),
        ]
    }
}

pub struct ResearchWorker {
    name: String,
}

impl ResearchWorker {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Worker for ResearchWorker {
    async fn execute(&self, task: &Task) -> KiasResult<String> {
        tracing::info!(worker = %self.name, task_id = %task.id, "Executing research task");

        // Extract query from task context
        let query = task
            .context
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or(&task.description);

        // For now, return structured research output
        // In production, this would call search engines and LLM APIs
        Ok(format!(
            "Research results for: {}\n\n\
             Query: {}\n\
             Task: {}\n\n\
             Note: This is a placeholder. Integrate with search API for real results.",
            task.name, query, task.description
        ))
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["web_search".to_string(), "document_analysis".to_string()]
    }
}

/// LLM-backed worker that calls an external LLM API
pub struct LlmWorker {
    name: String,
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl LlmWorker {
    pub fn new(name: &str, api_url: &str, api_key: &str, model: &str) -> Self {
        Self {
            name: name.to_string(),
            client: reqwest::Client::new(),
            api_url: api_url.to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl Worker for LlmWorker {
    async fn execute(&self, task: &Task) -> KiasResult<String> {
        tracing::info!(worker = %self.name, task_id = %task.id, model = %self.model, "Executing LLM task");

        let prompt = task
            .context
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or(&task.description);

        let request_body = serde_json::json!({
            "model": self.model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 2048,
        });

        let response = self
            .client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();

                if (200..300).contains(&status) {
                    let parsed: serde_json::Result<serde_json::Value> = serde_json::from_str(&body);
                    let content = parsed.ok().and_then(|v| {
                        v["choices"]
                            .as_array()?
                            .first()?
                            .get("message")?
                            .get("content")?
                            .as_str()
                            .map(|s| s.to_string())
                    });

                    Ok(content.unwrap_or(body))
                } else {
                    Ok(format!(
                        "LLM_ERROR (status {}): {}",
                        status,
                        &body[..body.len().min(200)]
                    ))
                }
            }
            Err(e) => Ok(format!("LLM_REQUEST_FAILED: {}", e)),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "text_generation".to_string(),
            "code_generation".to_string(),
            "analysis".to_string(),
            "summarization".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            name: "test".to_string(),
            description: "test task".to_string(),
            assigned_to: None,
            verified_by: None,
            status: crate::state::TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            retry_count: 0,
            max_retries: 3,
            context: serde_json::json!({}),
        }
    }

    fn make_task_with_command(id: &str, command: &str) -> Task {
        Task {
            id: id.to_string(),
            name: "test".to_string(),
            description: "test task".to_string(),
            assigned_to: None,
            verified_by: None,
            status: crate::state::TaskStatus::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            retry_count: 0,
            max_retries: 3,
            context: serde_json::json!({"command": command}),
        }
    }

    #[tokio::test]
    async fn test_code_worker_executes_command() {
        let worker = CodeWorker::new("coder");
        let task = make_task_with_command("t1", "echo hello");
        let result = worker.execute(&task).await.unwrap();
        assert!(result.contains("hello"));
    }

    #[tokio::test]
    async fn test_code_worker_failing_command() {
        let worker = CodeWorker::new("coder");
        let task = make_task_with_command("t1", "exit 1");
        let result = worker.execute(&task).await.unwrap();
        assert!(result.contains("EXIT_CODE=1"));
    }

    #[tokio::test]
    async fn test_code_worker_no_command() {
        let worker = CodeWorker::new("coder");
        let task = make_task("t1");
        let result = worker.execute(&task).await.unwrap();
        assert!(result.contains("no command specified"));
    }

    #[tokio::test]
    async fn test_code_worker_capabilities() {
        let worker = CodeWorker::new("coder");
        let caps = worker.capabilities();
        assert!(caps.contains(&"code_generation".to_string()));
        assert!(caps.contains(&"shell_execution".to_string()));
    }

    #[tokio::test]
    async fn test_research_worker() {
        let worker = ResearchWorker::new("researcher");
        let task = make_task("t1");
        let result = worker.execute(&task).await.unwrap();
        assert!(result.contains("Research results"));
    }

    #[tokio::test]
    async fn test_research_worker_with_query() {
        let mut task = make_task("t1");
        task.context = serde_json::json!({"query": "What is KIAS?"});
        let worker = ResearchWorker::new("researcher");
        let result = worker.execute(&task).await.unwrap();
        assert!(result.contains("What is KIAS?"));
    }

    #[tokio::test]
    async fn test_research_worker_capabilities() {
        let worker = ResearchWorker::new("researcher");
        let caps = worker.capabilities();
        assert!(caps.contains(&"web_search".to_string()));
    }
}
