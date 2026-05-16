//! 工具执行框架
//!
//! 参考 Codex CLI 的 3 工具模型:
//! - file_read: 读取文件
//! - file_write: 写入文件
//! - shell: 执行 shell 命令
//!
//! 加上扩展工具:
//! - search: 搜索文件内容
//! - http: HTTP 请求
//! - python: Python 脚本执行

pub mod builtin;
pub mod registry;
pub mod sandbox;

pub use builtin::*;
pub use registry::ToolRegistry;
