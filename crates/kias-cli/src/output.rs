//! 输出格式化模块 - 超越 AgentRun CLI

use crate::OutputFormat;
use serde::Serialize;
use tabled::Tabled;

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
        OutputFormat::Json => match serde_json::to_string_pretty(result) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("JSON 序列化失败: {}", e),
        },
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
        OutputFormat::Yaml => match serde_yaml::to_string(result) {
            Ok(yaml) => println!("{}", yaml),
            Err(e) => eprintln!("YAML 序列化失败: {}", e),
        },
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
            eprintln!(
                "{}",
                serde_json::json!({
                    "status": "error",
                    "error": msg,
                    "exit_code": code as i32
                })
            );
        }
        _ => {
            eprintln!("Error: {}", msg);
        }
    }
    code as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_values() {
        assert_eq!(ExitCode::Success as i32, 0);
        assert_eq!(ExitCode::ArgumentError as i32, 1);
        assert_eq!(ExitCode::AuthError as i32, 2);
        assert_eq!(ExitCode::NotFound as i32, 3);
        assert_eq!(ExitCode::PermissionDenied as i32, 4);
        assert_eq!(ExitCode::ServerError as i32, 5);
        assert_eq!(ExitCode::Timeout as i32, 6);
        assert_eq!(ExitCode::CostExceeded as i32, 7);
    }

    #[test]
    fn test_print_error_json_format() {
        let code = print_error("test error", ExitCode::ServerError, &OutputFormat::Json);
        assert_eq!(code, 5);
    }

    #[test]
    fn test_print_error_table_format() {
        let code = print_error("test error", ExitCode::ArgumentError, &OutputFormat::Table);
        assert_eq!(code, 1);
    }

    #[test]
    fn test_print_error_yaml_format() {
        let code = print_error("test error", ExitCode::AuthError, &OutputFormat::Yaml);
        assert_eq!(code, 2);
    }

    #[test]
    fn test_print_error_quiet_format() {
        let code = print_error("test error", ExitCode::NotFound, &OutputFormat::Quiet);
        assert_eq!(code, 3);
    }

    #[test]
    fn test_print_message_json() {
        // Should not panic
        print_message("hello", &OutputFormat::Json);
        print_message("hello", &OutputFormat::Table);
        print_message("hello", &OutputFormat::Yaml);
        print_message("hello", &OutputFormat::Quiet);
    }

    #[test]
    fn test_command_result_serialization() {
        let result = CommandResult {
            status: "ok".to_string(),
            data: "test-data".to_string(),
            metadata: ResultMetadata {
                duration_ms: 100,
                tokens_used: Some(50),
                cost: Some(0.001),
                request_id: "req-001".to_string(),
            },
        };
        let json = serde_json::to_string(&result).expect("should serialize");
        assert!(json.contains("ok"));
        assert!(json.contains("req-001"));
    }
}
