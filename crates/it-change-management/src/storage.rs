//! # SQLite 持久化层
//!
//! 为 IT 变更管理提供 SQLite 持久化存储，支持：
//! - 变更请求 CRUD
//! - 审计日志（不可篡改，哈希链）
//! - 审批记录
//! - CAPA 记录
//! - 附件元数据

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, Result as SqlResult};
use serde_json;
use std::path::Path;
use std::sync::Mutex;

use crate::*;

/// SQLite 存储错误
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("未找到: {0}")]
    NotFound(String),
}

/// SQLite 持久化存储
pub struct ChangeStorage {
    conn: Mutex<Connection>,
}

impl ChangeStorage {
    /// 创建新的存储实例
    pub fn new(db_path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open(db_path)?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    /// 创建内存数据库（用于测试）
    pub fn new_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Mutex::new(conn),
        };
        storage.init_tables()?;
        Ok(storage)
    }

    /// 初始化数据库表
    fn init_tables(&self) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS change_requests (
                id TEXT PRIMARY KEY,
                change_number TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                change_type TEXT NOT NULL,
                change_category TEXT NOT NULL,
                risk_level TEXT NOT NULL,
                status TEXT NOT NULL,
                requester TEXT NOT NULL,
                requester_department TEXT NOT NULL,
                impact_assessment TEXT NOT NULL,
                rollback_plan TEXT NOT NULL,
                implementation_plan TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                submitted_at TEXT,
                approved_at TEXT,
                implemented_at TEXT,
                verified_at TEXT,
                closed_at TEXT,
                sla_deadline TEXT,
                emergency_approval_deadline TEXT
            );

            CREATE TABLE IF NOT EXISTS approvers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                change_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                role TEXT NOT NULL,
                decision TEXT,
                signed_at TEXT,
                signature TEXT,
                FOREIGN KEY (change_id) REFERENCES change_requests(id)
            );

            CREATE TABLE IF NOT EXISTS audit_log (
                id TEXT PRIMARY KEY,
                change_id TEXT NOT NULL,
                actor TEXT NOT NULL,
                action TEXT NOT NULL,
                detail TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                previous_hash TEXT NOT NULL,
                hash TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                FOREIGN KEY (change_id) REFERENCES change_requests(id)
            );

            CREATE TABLE IF NOT EXISTS capa_records (
                id TEXT PRIMARY KEY,
                change_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                root_cause TEXT,
                corrective_action TEXT,
                preventive_action TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                closed_at TEXT,
                FOREIGN KEY (change_id) REFERENCES change_requests(id)
            );

            CREATE TABLE IF NOT EXISTS attachments (
                id TEXT PRIMARY KEY,
                change_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                file_type TEXT NOT NULL,
                file_size_bytes INTEGER NOT NULL,
                storage_path TEXT NOT NULL,
                uploaded_by TEXT NOT NULL,
                uploaded_at TEXT NOT NULL,
                hash_sha256 TEXT NOT NULL,
                FOREIGN KEY (change_id) REFERENCES change_requests(id)
            );

            CREATE TABLE IF NOT EXISTS comments (
                id TEXT PRIMARY KEY,
                change_id TEXT NOT NULL,
                author TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                is_internal INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (change_id) REFERENCES change_requests(id)
            );

            CREATE INDEX IF NOT EXISTS idx_change_status ON change_requests(status);
            CREATE INDEX IF NOT EXISTS idx_change_risk ON change_requests(risk_level);
            CREATE INDEX IF NOT EXISTS idx_audit_change ON audit_log(change_id);
            CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
            CREATE INDEX IF NOT EXISTS idx_capa_change ON capa_records(change_id);
            ",
        )?;

        Ok(())
    }

    /// 保存变更请求
    pub fn save_change(&self, change: &ItChangeRequest) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO change_requests (
                id, change_number, title, description, change_type, change_category,
                risk_level, status, requester, requester_department,
                impact_assessment, rollback_plan, implementation_plan,
                created_at, updated_at, submitted_at, approved_at,
                implemented_at, verified_at, closed_at,
                sla_deadline, emergency_approval_deadline
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                change.id,
                change.change_number,
                change.title,
                change.description,
                serde_json::to_string(&change.change_type)?,
                serde_json::to_string(&change.change_category)?,
                serde_json::to_string(&change.risk_level)?,
                serde_json::to_string(&change.status)?,
                change.requester,
                change.requester_department,
                serde_json::to_string(&change.impact_assessment)?,
                change.rollback_plan,
                change.implementation_plan,
                change.created_at.to_rfc3339(),
                change.updated_at.to_rfc3339(),
                change.submitted_at.map(|t| t.to_rfc3339()),
                change.approved_at.map(|t| t.to_rfc3339()),
                change.implemented_at.map(|t| t.to_rfc3339()),
                change.verified_at.map(|t| t.to_rfc3339()),
                change.closed_at.map(|t| t.to_rfc3339()),
                change.sla_deadline.map(|t| t.to_rfc3339()),
                change.emergency_approval_deadline.map(|t| t.to_rfc3339()),
            ],
        )?;

        // 保存审批人
        for approver in &change.approvers {
            conn.execute(
                "INSERT INTO approvers (change_id, user_id, name, role, decision, signed_at, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    change.id,
                    approver.user_id,
                    approver.name,
                    approver.role,
                    approver.decision.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()),
                    approver.signed_at.map(|t| t.to_rfc3339()),
                    approver.signature.as_ref().map(|s| serde_json::to_string(s).unwrap_or_default()),
                ],
            )?;
        }

        // 保存 CAPA 记录
        for capa in &change.capa_records {
            conn.execute(
                "INSERT OR IGNORE INTO capa_records (id, change_id, title, description, root_cause, corrective_action, preventive_action, status, created_at, closed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    capa.id,
                    capa.change_id,
                    capa.title,
                    capa.description,
                    capa.root_cause,
                    capa.corrective_action,
                    capa.preventive_action,
                    serde_json::to_string(&capa.status)?,
                    capa.created_at.to_rfc3339(),
                    capa.closed_at.map(|t| t.to_rfc3339()),
                ],
            )?;
        }

        // 保存附件
        for attachment in &change.attachments {
            conn.execute(
                "INSERT OR IGNORE INTO attachments (id, change_id, filename, file_type, file_size_bytes, storage_path, uploaded_by, uploaded_at, hash_sha256)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    attachment.id,
                    change.id,
                    attachment.filename,
                    attachment.file_type,
                    attachment.file_size_bytes,
                    attachment.storage_path,
                    attachment.uploaded_by,
                    attachment.uploaded_at.to_rfc3339(),
                    attachment.hash_sha256,
                ],
            )?;
        }

        // 保存评论
        for comment in &change.comments {
            conn.execute(
                "INSERT OR IGNORE INTO comments (id, change_id, author, content, created_at, is_internal)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    comment.id,
                    change.id,
                    comment.author,
                    comment.content,
                    comment.created_at.to_rfc3339(),
                    comment.is_internal as i32,
                ],
            )?;
        }

        Ok(())
    }

    /// 保存审计日志条目
    pub fn save_audit_entry(&self, entry: &AuditEntry) -> Result<(), StorageError> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT INTO audit_log (id, change_id, actor, action, detail, timestamp, previous_hash, hash, ip_address, user_agent)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id,
                entry.change_id,
                entry.actor,
                serde_json::to_string(&entry.action)?,
                entry.detail,
                entry.timestamp.to_rfc3339(),
                entry.previous_hash,
                entry.hash,
                entry.ip_address,
                entry.user_agent,
            ],
        )?;

        Ok(())
    }

    /// 获取变更请求
    pub fn get_change(&self, change_id: &str) -> Result<Option<ItChangeRequest>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, change_number, title, description, change_type, change_category,
                    risk_level, status, requester, requester_department,
                    impact_assessment, rollback_plan, implementation_plan,
                    created_at, updated_at, submitted_at, approved_at,
                    implemented_at, verified_at, closed_at,
                    sla_deadline, emergency_approval_deadline
             FROM change_requests WHERE id = ?1",
        )?;

        let result = stmt.query_row(params![change_id], |row| {
            Ok(ItChangeRequest {
                id: row.get(0)?,
                change_number: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                change_type: serde_json::from_str(&row.get::<_, String>(4)?)
                    .unwrap_or(ChangeType::Configuration),
                change_category: serde_json::from_str(&row.get::<_, String>(5)?)
                    .unwrap_or(ChangeCategory::Normal),
                risk_level: serde_json::from_str(&row.get::<_, String>(6)?)
                    .unwrap_or(RiskLevel::Low),
                status: serde_json::from_str(&row.get::<_, String>(7)?)
                    .unwrap_or(ChangeStatus::Draft),
                requester: row.get(8)?,
                requester_department: row.get(9)?,
                impact_assessment: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or(
                    ImpactAssessment {
                        affected_systems: vec![],
                        affected_users: vec![],
                        downtime_estimate_minutes: 0,
                        risk_mitigation: vec![],
                        testing_requirements: vec![],
                        gxp_impact: GxpImpact::None,
                        requires_csv_validation: false,
                        affects_data_integrity: false,
                    },
                ),
                rollback_plan: row.get(11)?,
                implementation_plan: row.get(12)?,
                approvers: Vec::new(),
                verification_steps: Vec::new(),
                validation_plan: None,
                capa_records: Vec::new(),
                attachments: Vec::new(),
                comments: Vec::new(),
                created_at: parse_datetime(&row.get::<_, String>(13)?).unwrap_or_default(),
                updated_at: parse_datetime(&row.get::<_, String>(14)?).unwrap_or_default(),
                submitted_at: row
                    .get::<_, Option<String>>(15)?
                    .and_then(|s| parse_datetime(&s)),
                approved_at: row
                    .get::<_, Option<String>>(16)?
                    .and_then(|s| parse_datetime(&s)),
                implemented_at: row
                    .get::<_, Option<String>>(17)?
                    .and_then(|s| parse_datetime(&s)),
                verified_at: row
                    .get::<_, Option<String>>(18)?
                    .and_then(|s| parse_datetime(&s)),
                closed_at: row
                    .get::<_, Option<String>>(19)?
                    .and_then(|s| parse_datetime(&s)),
                sla_deadline: row
                    .get::<_, Option<String>>(20)?
                    .and_then(|s| parse_datetime(&s)),
                emergency_approval_deadline: row
                    .get::<_, Option<String>>(21)?
                    .and_then(|s| parse_datetime(&s)),
            })
        });

        match result {
            Ok(mut change) => {
                // 加载关联数据
                change.approvers = self.get_approvers_for_change(&conn, change_id)?;
                change.capa_records = self.get_capa_records_for_change(&conn, change_id)?;
                change.attachments = self.get_attachments_for_change(&conn, change_id)?;
                change.comments = self.get_comments_for_change(&conn, change_id)?;
                Ok(Some(change))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(StorageError::Database(e)),
        }
    }

    /// 列出所有变更请求
    pub fn list_changes(&self) -> Result<Vec<ItChangeRequest>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare("SELECT id FROM change_requests ORDER BY created_at DESC")?;

        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        drop(stmt);
        drop(conn);

        let mut changes = Vec::new();
        for id in ids {
            if let Some(change) = self.get_change(&id)? {
                changes.push(change);
            }
        }

        Ok(changes)
    }

    /// 获取审计日志
    pub fn get_audit_log(&self, change_id: &str) -> Result<Vec<AuditEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, change_id, actor, action, detail, timestamp, previous_hash, hash, ip_address, user_agent
             FROM audit_log WHERE change_id = ?1 ORDER BY timestamp ASC",
        )?;

        let entries = stmt
            .query_map(params![change_id], |row| {
                Ok(AuditEntry {
                    id: row.get(0)?,
                    change_id: row.get(1)?,
                    actor: row.get(2)?,
                    action: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(AuditAction::Created),
                    detail: row.get(4)?,
                    timestamp: parse_datetime(&row.get::<_, String>(5)?).unwrap_or_default(),
                    previous_hash: row.get(6)?,
                    hash: row.get(7)?,
                    ip_address: row.get(8)?,
                    user_agent: row.get(9)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(entries)
    }

    /// 获取全部审计日志
    pub fn get_all_audit_log(&self) -> Result<Vec<AuditEntry>, StorageError> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, change_id, actor, action, detail, timestamp, previous_hash, hash, ip_address, user_agent
             FROM audit_log ORDER BY timestamp ASC",
        )?;

        let entries = stmt
            .query_map([], |row| {
                Ok(AuditEntry {
                    id: row.get(0)?,
                    change_id: row.get(1)?,
                    actor: row.get(2)?,
                    action: serde_json::from_str(&row.get::<_, String>(3)?)
                        .unwrap_or(AuditAction::Created),
                    detail: row.get(4)?,
                    timestamp: parse_datetime(&row.get::<_, String>(5)?).unwrap_or_default(),
                    previous_hash: row.get(6)?,
                    hash: row.get(7)?,
                    ip_address: row.get(8)?,
                    user_agent: row.get(9)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(entries)
    }

    /// 按状态筛选变更
    pub fn list_changes_by_status(
        &self,
        status: &ChangeStatus,
    ) -> Result<Vec<ItChangeRequest>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let status_str = serde_json::to_string(status).unwrap_or_default();

        let mut stmt = conn
            .prepare("SELECT id FROM change_requests WHERE status = ?1 ORDER BY created_at DESC")?;

        let ids: Vec<String> = stmt
            .query_map(params![status_str], |row| row.get(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        drop(stmt);
        drop(conn);

        let mut changes = Vec::new();
        for id in ids {
            if let Some(change) = self.get_change(&id)? {
                changes.push(change);
            }
        }

        Ok(changes)
    }

    /// 获取 SLA 超期的变更
    pub fn get_sla_violations(&self) -> Result<Vec<ItChangeRequest>, StorageError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let mut stmt = conn.prepare(
            "SELECT id FROM change_requests
             WHERE sla_deadline < ?1
             AND status NOT IN ('Closed', 'Rejected', 'RolledBack')
             ORDER BY sla_deadline ASC",
        )?;

        let ids: Vec<String> = stmt
            .query_map(params![now], |row| row.get(0))?
            .collect::<SqlResult<Vec<_>>>()?;

        drop(stmt);
        drop(conn);

        let mut changes = Vec::new();
        for id in ids {
            if let Some(change) = self.get_change(&id)? {
                changes.push(change);
            }
        }

        Ok(changes)
    }

    /// 验证审计日志哈希链完整性
    pub fn verify_audit_chain_integrity(&self) -> Result<bool, StorageError> {
        let entries = self.get_all_audit_log()?;

        for i in 1..entries.len() {
            if entries[i].previous_hash != entries[i - 1].hash {
                return Ok(false);
            }
        }

        Ok(true)
    }

    // 内部辅助方法

    fn get_approvers_for_change(
        &self,
        conn: &Connection,
        change_id: &str,
    ) -> Result<Vec<Approver>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT user_id, name, role, decision, signed_at, signature
             FROM approvers WHERE change_id = ?1",
        )?;

        let approvers = stmt
            .query_map(params![change_id], |row| {
                Ok(Approver {
                    user_id: row.get(0)?,
                    name: row.get(1)?,
                    role: row.get(2)?,
                    decision: row
                        .get::<_, Option<String>>(3)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                    signed_at: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| parse_datetime(&s)),
                    signature: row
                        .get::<_, Option<String>>(5)?
                        .and_then(|s| serde_json::from_str(&s).ok()),
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(approvers)
    }

    fn get_capa_records_for_change(
        &self,
        conn: &Connection,
        change_id: &str,
    ) -> Result<Vec<CapaRecord>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, change_id, title, description, root_cause, corrective_action, preventive_action, status, created_at, closed_at
             FROM capa_records WHERE change_id = ?1",
        )?;

        let records = stmt
            .query_map(params![change_id], |row| {
                Ok(CapaRecord {
                    id: row.get(0)?,
                    change_id: row.get(1)?,
                    title: row.get(2)?,
                    description: row.get(3)?,
                    root_cause: row.get(4)?,
                    corrective_action: row.get(5)?,
                    preventive_action: row.get(6)?,
                    status: serde_json::from_str(&row.get::<_, String>(7)?)
                        .unwrap_or(CapaStatus::Open),
                    created_at: parse_datetime(&row.get::<_, String>(8)?).unwrap_or_default(),
                    closed_at: row
                        .get::<_, Option<String>>(9)?
                        .and_then(|s| parse_datetime(&s)),
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(records)
    }

    fn get_attachments_for_change(
        &self,
        conn: &Connection,
        change_id: &str,
    ) -> Result<Vec<Attachment>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, filename, file_type, file_size_bytes, storage_path, uploaded_by, uploaded_at, hash_sha256
             FROM attachments WHERE change_id = ?1",
        )?;

        let attachments = stmt
            .query_map(params![change_id], |row| {
                Ok(Attachment {
                    id: row.get(0)?,
                    filename: row.get(1)?,
                    file_type: row.get(2)?,
                    file_size_bytes: row.get(3)?,
                    storage_path: row.get(4)?,
                    uploaded_by: row.get(5)?,
                    uploaded_at: parse_datetime(&row.get::<_, String>(6)?).unwrap_or_default(),
                    hash_sha256: row.get(7)?,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(attachments)
    }

    fn get_comments_for_change(
        &self,
        conn: &Connection,
        change_id: &str,
    ) -> Result<Vec<Comment>, StorageError> {
        let mut stmt = conn.prepare(
            "SELECT id, author, content, created_at, is_internal
             FROM comments WHERE change_id = ?1",
        )?;

        let comments = stmt
            .query_map(params![change_id], |row| {
                Ok(Comment {
                    id: row.get(0)?,
                    author: row.get(1)?,
                    content: row.get(2)?,
                    created_at: parse_datetime(&row.get::<_, String>(3)?).unwrap_or_default(),
                    is_internal: row.get::<_, i32>(4)? != 0,
                })
            })?
            .collect::<SqlResult<Vec<_>>>()?;

        Ok(comments)
    }
}

/// 解析 ISO 8601 日期时间
fn parse_datetime(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_change() -> ItChangeRequest {
        let mut manager = ItChangeManager::new();
        manager.create_change_request(
            "测试变更".to_string(),
            "测试描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "test.user".to_string(),
            "IT部门".to_string(),
            "回滚计划".to_string(),
            "实施计划".to_string(),
            ImpactAssessment {
                affected_systems: vec!["LIMS".to_string()],
                affected_users: vec!["QC".to_string()],
                downtime_estimate_minutes: 30,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::Direct,
                requires_csv_validation: true,
                affects_data_integrity: false,
            },
        )
    }

    #[test]
    fn test_save_and_get_change() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();

        storage.save_change(&change).unwrap();
        let loaded = storage.get_change(&change.id).unwrap().unwrap();

        assert_eq!(loaded.id, change.id);
        assert_eq!(loaded.title, change.title);
        assert_eq!(loaded.change_number, change.change_number);
    }

    #[test]
    fn test_list_changes() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let mut manager = ItChangeManager::new();

        let change1 = manager.create_change_request(
            "第一个变更".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user1".to_string(),
            "IT部门".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        );

        let change2 = manager.create_change_request(
            "第二个变更".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::High,
            "user2".to_string(),
            "QA部门".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        );

        storage.save_change(&change1).unwrap();
        storage.save_change(&change2).unwrap();

        let changes = storage.list_changes().unwrap();
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn test_save_and_get_audit_log() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();
        storage.save_change(&change).unwrap();

        let entry = AuditEntry {
            id: "audit-1".to_string(),
            change_id: change.id.clone(),
            actor: "test.user".to_string(),
            action: AuditAction::Created,
            detail: "变更已创建".to_string(),
            timestamp: Utc::now(),
            previous_hash: "0".repeat(64),
            hash: "abc123".to_string(),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
        };

        storage.save_audit_entry(&entry).unwrap();

        let log = storage.get_audit_log(&change.id).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].actor, "test.user");
    }

    #[test]
    fn test_audit_chain_integrity() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();
        storage.save_change(&change).unwrap();

        let entry1 = AuditEntry {
            id: "audit-1".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Created,
            detail: "创建".to_string(),
            timestamp: Utc::now(),
            previous_hash: "0".repeat(64),
            hash: "hash1".to_string(),
            ip_address: None,
            user_agent: None,
        };

        let entry2 = AuditEntry {
            id: "audit-2".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Submitted,
            detail: "提交".to_string(),
            timestamp: Utc::now(),
            previous_hash: "hash1".to_string(),
            hash: "hash2".to_string(),
            ip_address: None,
            user_agent: None,
        };

        storage.save_audit_entry(&entry1).unwrap();
        storage.save_audit_entry(&entry2).unwrap();

        assert!(storage.verify_audit_chain_integrity().unwrap());
    }

    #[test]
    fn test_verify_audit_chain_corrupted() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();
        storage.save_change(&change).unwrap();

        let entry1 = AuditEntry {
            id: "audit-1".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Created,
            detail: "创建".to_string(),
            timestamp: Utc::now(),
            previous_hash: "0".repeat(64),
            hash: "hash1".to_string(),
            ip_address: None,
            user_agent: None,
        };

        // Corrupted: previous_hash doesn't match entry1.hash
        let entry2 = AuditEntry {
            id: "audit-2".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Submitted,
            detail: "提交".to_string(),
            timestamp: Utc::now(),
            previous_hash: "WRONG_HASH".to_string(),
            hash: "hash2".to_string(),
            ip_address: None,
            user_agent: None,
        };

        storage.save_audit_entry(&entry1).unwrap();
        storage.save_audit_entry(&entry2).unwrap();

        assert!(!storage.verify_audit_chain_integrity().unwrap());
    }

    #[test]
    fn test_get_change_not_found() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let result = storage.get_change("nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_changes_by_status() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let mut manager = ItChangeManager::new();

        let change1 = manager.create_change_request(
            "变更1".to_string(),
            "描述".to_string(),
            ChangeType::Configuration,
            ChangeCategory::Normal,
            RiskLevel::Low,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        );

        let change2 = manager.create_change_request(
            "变更2".to_string(),
            "描述".to_string(),
            ChangeType::Application,
            ChangeCategory::Normal,
            RiskLevel::High,
            "user".to_string(),
            "IT".to_string(),
            "回滚".to_string(),
            "实施".to_string(),
            ImpactAssessment {
                affected_systems: vec![],
                affected_users: vec![],
                downtime_estimate_minutes: 0,
                risk_mitigation: vec![],
                testing_requirements: vec![],
                gxp_impact: GxpImpact::None,
                requires_csv_validation: false,
                affects_data_integrity: false,
            },
        );

        storage.save_change(&change1).unwrap();
        storage.save_change(&change2).unwrap();

        let drafts = storage
            .list_changes_by_status(&ChangeStatus::Draft)
            .unwrap();
        assert_eq!(drafts.len(), 2);

        let submitted = storage
            .list_changes_by_status(&ChangeStatus::Submitted)
            .unwrap();
        assert_eq!(submitted.len(), 0);
    }

    #[test]
    fn test_get_all_audit_log() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();
        storage.save_change(&change).unwrap();

        let entry = AuditEntry {
            id: "audit-1".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Created,
            detail: "创建".to_string(),
            timestamp: Utc::now(),
            previous_hash: "0".repeat(64),
            hash: "hash1".to_string(),
            ip_address: None,
            user_agent: None,
        };

        storage.save_audit_entry(&entry).unwrap();

        let all = storage.get_all_audit_log().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].actor, "user");
    }

    #[test]
    fn test_save_change_updates_existing() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let mut change = create_test_change();
        storage.save_change(&change).unwrap();

        // Update the change
        change.title = "更新后的标题".to_string();
        change.status = ChangeStatus::Submitted;
        storage.save_change(&change).unwrap();

        let loaded = storage.get_change(&change.id).unwrap().unwrap();
        assert_eq!(loaded.title, "更新后的标题");
        assert_eq!(loaded.status, ChangeStatus::Submitted);
    }

    #[test]
    fn test_audit_entry_with_ip_and_user_agent() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let change = create_test_change();
        storage.save_change(&change).unwrap();

        let entry = AuditEntry {
            id: "audit-1".to_string(),
            change_id: change.id.clone(),
            actor: "user".to_string(),
            action: AuditAction::Created,
            detail: "创建".to_string(),
            timestamp: Utc::now(),
            previous_hash: "0".repeat(64),
            hash: "hash1".to_string(),
            ip_address: Some("10.0.0.1".to_string()),
            user_agent: Some("TestAgent/1.0".to_string()),
        };

        storage.save_audit_entry(&entry).unwrap();

        let log = storage.get_audit_log(&change.id).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].ip_address, Some("10.0.0.1".to_string()));
        assert_eq!(log[0].user_agent, Some("TestAgent/1.0".to_string()));
    }

    #[test]
    fn test_empty_audit_log() {
        let storage = ChangeStorage::new_in_memory().unwrap();
        let log = storage.get_audit_log("nonexistent").unwrap();
        assert!(log.is_empty());

        let all = storage.get_all_audit_log().unwrap();
        assert!(all.is_empty());
    }
}
