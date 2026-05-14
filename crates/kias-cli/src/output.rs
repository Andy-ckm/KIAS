//! 输出格式化模块 - 超越 AgentRun CLI

use crate::OutputFormat;
use serde::Serialize;
use tabled::{Table, Tabled};

/// 命令执行结果
#[derive(Debug, Serialize)]
pub struct CommandResult<T: Serialize> {
    pub status: String,
    pub data: T,
    pub metadata: ResultMetadata,
}

#[derive(Debug, Serialize)]
pub struct ResultMetadata {
    pub duration_ms: u64,
    pub tokens_used: Option<u64>,
    pub cost: Option<f64>,
    pub request_id: String,
}

/// 退出码
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    ArgumentError = 1,
    AuthError = 2,
    NotFound = 3,
    PermissionDenied = 4,
    ServerError = 5,
    Timeout = 6,
    CostExceeded = 7,
}

/// 格式化输出
pub fn print_result<T: Serialize + Tabled>(result: &CommandResult<T>, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(result).unwrap_or_default());
        }
        OutputFormat::Table => {
            println!("Status: {}", result.status);
            println!("Duration: {}ms", result.metadata.duration_ms);
            if let Some(tokens) = result.metadata.tokens_used {
                println!("Tokens: {}", tokens);
            }
            if let Some(cost) = result.metadata.cost {
                println!("Cost: ${:.4}", cost);
            }
            println!("Request ID: {}", result.metadata.request_id);
        }
        OutputFormat::Yaml => {
            println!("{}", serde_yaml::to_string(result).unwrap_or_default());
        }
        OutputFormat::Quiet => {
            // Quiet 模式只输出核心标识符
            println!("{}", result.status);
        }
    }
}

/// 打印简单消息
pub fn print_message(msg: &str, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::json!({"message": msg}));
        }
        OutputFormat::Table | OutputFormat::Yaml => {
            println!("{}", msg);
        }
        OutputFormat::Quiet => {
            println!("{}", msg);
        }
    }
}

/// 打印错误并返回退出码
pub fn print_error(msg: &str, code: ExitCode, format: &OutputFormat) -> i32 {
    match format {
        OutputFormat::Json => {
            eprintln!("{}", serde_json::json!({
                "status": "error",
                "error": msg,
                "exit_code": code as i32
            }));
        }
        _ => {
            eprintln!("Error: {}", msg);
        }
    }
    code as i32
}
