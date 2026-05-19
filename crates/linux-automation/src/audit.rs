//! 审计日志 - 不可变存储

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use tracing::debug;
use uuid::Uuid;

/// 审计统计
pub struct AuditStatistics {
    pub total_entries: usize,
}

/// 审计日志管理器
pub struct AuditLog {
    conn: Mutex<Connection>,
}

impl AuditLog {
    /// 创建新的审计日志管理器
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // 创建审计表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                user_name TEXT NOT NULL,
                action TEXT NOT NULL,
                target TEXT NOT NULL,
                result TEXT NOT NULL,
                details TEXT,
                signature TEXT,
                task_id TEXT,
                hash TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_audit_user ON audit_log(user_name);
            CREATE INDEX IF NOT EXISTS idx_audit_task ON audit_log(task_id);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// 记录任务执行审计
    pub fn record_task_execution(
        &self,
        task: &AutomationTask,
        result: &AutomationResult,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;

        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            user: task.created_by.clone(),
            action: format!("Execute {:?}", task.task_type),
            target: format!("{:?}", task.task_type),
            result: format!("{:?}", result.status),
            details: Some(result.summary.clone()),
            signature: None,
        };

        // 计算哈希（简单实现，生产环境应使用 SHA-256）
        let hash = format!("{:?}", entry);

        conn.execute(
            "INSERT INTO audit_log (id, timestamp, user_name, action, target, result, details, signature, task_id, hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id.to_string(),
                entry.timestamp.to_rfc3339(),
                entry.user,
                entry.action,
                entry.target,
                entry.result,
                entry.details,
                entry.signature,
                task.id.to_string(),
                hash,
            ],
        )?;

        Ok(())
    }

    /// 获取审计日志
    pub fn get_audit_log(&self, limit: Option<usize>) -> Result<Vec<AuditEntry>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;
        let limit = limit.unwrap_or(100);

        let mut stmt = conn.prepare(
            "SELECT id, timestamp, user_name, action, target, result, details, signature
             FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
        )?;

        let entries = stmt
            .query_map(params![limit], |row| {
                let id_str: String = row.get(0)?;
                let timestamp_str: String = row.get(1)?;

                Ok(AuditEntry {
                    id: Uuid::parse_str(&id_str).unwrap_or_default(),
                    timestamp: chrono::DateTime::parse_from_rfc3339(&timestamp_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    user: row.get(2)?,
                    action: row.get(3)?,
                    target: row.get(4)?,
                    result: row.get(5)?,
                    details: row.get(6)?,
                    signature: row.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// 获取审计统计
    pub fn get_statistics(&self) -> Result<AuditStatistics> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;

        let total: usize =
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))?;

        Ok(AuditStatistics {
            total_entries: total,
        })
    }

    /// 记录单条操作审计 (简化接口, 供巡检/初始化/Docker/K8s模块使用)
    pub fn log_action(&self, user: &str, action: &str, target: &str, result: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| AutomationError::LockPoisoned("conn".to_string()))?;
        let entry = AuditEntry {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            user: user.to_string(),
            action: action.to_string(),
            target: target.to_string(),
            result: result.to_string(),
            details: None,
            signature: None,
        };
        debug!(user = %user, action = %action, target = %target, "审计记录");

        conn.execute(
            "INSERT INTO audit_log (id, timestamp, user_name, action, target, result, details, signature, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.id.to_string(),
                entry.timestamp.to_rfc3339(),
                entry.user,
                entry.action,
                entry.target,
                entry.result,
                entry.details,
                entry.signature,
                String::new(),
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_audit_log() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();
        let stats = audit.get_statistics().unwrap();
        assert_eq!(stats.total_entries, 0);
    }

    #[test]
    fn test_record_audit() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "ls -la".to_string(),
                hosts: vec!["localhost".to_string()],
            },
            created_at: Utc::now(),
            created_by: "test-user".to_string(),
            priority: TaskPriority::Normal,
        };

        let result = AutomationResult {
            task_id: task.id,
            task_type: "CustomCommand".to_string(),
            status: TaskStatus::Success,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "Test".to_string(),
            audit_trail: vec![],
        };

        audit.record_task_execution(&task, &result).unwrap();

        let stats = audit.get_statistics().unwrap();
        assert_eq!(stats.total_entries, 1);

        let log = audit.get_audit_log(Some(10)).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].user, "test-user");
    }

    #[test]
    fn test_multiple_audit_entries() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        for i in 0..3 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: format!("user-{}", i),
                priority: TaskPriority::Normal,
            };

            let result = AutomationResult {
                task_id: task.id,
                task_type: "CustomCommand".to_string(),
                status: TaskStatus::Success,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                host_results: vec![],
                summary: "Test".to_string(),
                audit_trail: vec![],
            };

            audit.record_task_execution(&task, &result).unwrap();
        }

        let stats = audit.get_statistics().unwrap();
        assert_eq!(stats.total_entries, 3);

        let log = audit.get_audit_log(Some(10)).unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_audit_log_limit() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        for i in 0..5 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: "test-user".to_string(),
                priority: TaskPriority::Normal,
            };

            let result = AutomationResult {
                task_id: task.id,
                task_type: "CustomCommand".to_string(),
                status: TaskStatus::Success,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                host_results: vec![],
                summary: "Test".to_string(),
                audit_trail: vec![],
            };

            audit.record_task_execution(&task, &result).unwrap();
        }

        let log = audit.get_audit_log(Some(3)).unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_audit_log_none_limit() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        for i in 0..3 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: "user".to_string(),
                priority: TaskPriority::Normal,
            };
            let result = AutomationResult {
                task_id: task.id,
                task_type: "CustomCommand".to_string(),
                status: TaskStatus::Success,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                host_results: vec![],
                summary: "Test".to_string(),
                audit_trail: vec![],
            };
            audit.record_task_execution(&task, &result).unwrap();
        }

        let log = audit.get_audit_log(None).unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_audit_entry_fields() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::CustomCommand {
                command: "whoami".to_string(),
                hosts: vec!["server1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "admin".to_string(),
            priority: TaskPriority::High,
        };
        let result = AutomationResult {
            task_id: task.id,
            task_type: "CustomCommand".to_string(),
            status: TaskStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "Failed".to_string(),
            audit_trail: vec![],
        };
        audit.record_task_execution(&task, &result).unwrap();

        let log = audit.get_audit_log(Some(1)).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].user, "admin");
        assert!(log[0].action.contains("CustomCommand"));
    }

    #[test]
    fn test_audit_statistics_after_multiple_records() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        for i in 0..10 {
            let task = AutomationTask {
                id: Uuid::new_v4(),
                task_type: TaskType::CustomCommand {
                    command: format!("cmd-{}", i),
                    hosts: vec!["localhost".to_string()],
                },
                created_at: Utc::now(),
                created_by: "user".to_string(),
                priority: TaskPriority::Normal,
            };
            let result = AutomationResult {
                task_id: task.id,
                task_type: "CustomCommand".to_string(),
                status: TaskStatus::Success,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                host_results: vec![],
                summary: "Test".to_string(),
                audit_trail: vec![],
            };
            audit.record_task_execution(&task, &result).unwrap();
        }

        let stats = audit.get_statistics().unwrap();
        assert_eq!(stats.total_entries, 10);
    }

    #[test]
    fn test_audit_log_empty() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();
        let log = audit.get_audit_log(Some(10)).unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn test_audit_record_failed_task() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test.db");
        let audit = AuditLog::new(&db_path).unwrap();

        let task = AutomationTask {
            id: Uuid::new_v4(),
            task_type: TaskType::ComplianceScan {
                profile: "cis".to_string(),
                hosts: vec!["server1".to_string()],
            },
            created_at: Utc::now(),
            created_by: "scanner".to_string(),
            priority: TaskPriority::Critical,
        };
        let result = AutomationResult {
            task_id: task.id,
            task_type: "ComplianceScan".to_string(),
            status: TaskStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results: vec![],
            summary: "Scan failed".to_string(),
            audit_trail: vec![],
        };
        audit.record_task_execution(&task, &result).unwrap();

        let log = audit.get_audit_log(Some(1)).unwrap();
        assert!(log[0].action.contains("ComplianceScan"));
    }
}
