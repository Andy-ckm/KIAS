//! # Kanban SQLite Store
//!
//! Persistent storage for Kanban boards and tasks using SQLite (rusqlite).

use kias_common::{KiasError, KiasResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use super::kanban::{
    KanbanBoard, KanbanColumn, KanbanTask, Priority, WipLimit,
};

/// SQLite-backed Kanban store
pub struct KanbanStore {
    conn: Mutex<Connection>,
}

impl KanbanStore {
    /// Create a new store with the given SQLite database path
    pub fn new(db_path: &Path) -> KiasResult<Self> {
        let conn = Connection::open(db_path)
            .map_err(|e| KiasError::Config(format!("Failed to open kanban database: {}", e)))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| KiasError::Config(format!("Failed to set pragmas: {}", e)))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        info!("KanbanStore initialized at {:?}", db_path);
        Ok(store)
    }

    /// Create an in-memory store (for testing)
    pub fn new_in_memory() -> KiasResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| KiasError::Config(format!("Failed to create in-memory DB: {}", e)))?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| KiasError::Config(format!("Failed to set pragmas: {}", e)))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_tables()?;
        Ok(store)
    }

    fn init_tables(&self) -> KiasResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kanban_boards (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                wip_limits TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS kanban_tasks (
                id TEXT PRIMARY KEY,
                board_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                column_name TEXT NOT NULL DEFAULT 'Triage',
                priority INTEGER NOT NULL DEFAULT 2,
                assigned_to TEXT,
                required_capabilities TEXT NOT NULL DEFAULT '[]',
                tags TEXT NOT NULL DEFAULT '[]',
                estimated_minutes INTEGER,
                blocked_by TEXT NOT NULL DEFAULT '[]',
                parents TEXT NOT NULL DEFAULT '[]',
                children TEXT NOT NULL DEFAULT '[]',
                workspace TEXT,
                failure_count INTEGER NOT NULL DEFAULT 0,
                block_reason TEXT,
                metadata TEXT NOT NULL DEFAULT '{}',
                history TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                column_entered_at TEXT NOT NULL,
                FOREIGN KEY (board_id) REFERENCES kanban_boards(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_kanban_tasks_board ON kanban_tasks(board_id);
            CREATE INDEX IF NOT EXISTS idx_kanban_tasks_column ON kanban_tasks(board_id, column_name);",
        )
        .map_err(|e| KiasError::Config(format!("Failed to create kanban tables: {}", e)))?;
        debug!("Kanban tables initialized");
        Ok(())
    }

    // ─── Board operations ───

    pub fn save_board(&self, board: &KanbanBoard) -> KiasResult<()> {
        let conn = self.conn.lock().unwrap();
        let wip_json = serde_json::to_string(&board.wip_limits)
            .map_err(|e| KiasError::Config(format!("Serialize WIP: {}", e)))?;
        let now = st_to_str(SystemTime::now());
        conn.execute(
            "INSERT OR REPLACE INTO kanban_boards (id, name, description, wip_limits, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![board.id, board.name, board.description, wip_json, now],
        )
        .map_err(|e| KiasError::Config(format!("Failed to save board: {}", e)))?;
        Ok(())
    }

    pub fn load_board(&self, board_id: &str) -> KiasResult<Option<KanbanBoard>> {
        let conn = self.conn.lock().unwrap();
        let row: Option<(String, String, String, String, String)> = conn
            .query_row(
                "SELECT id, name, description, wip_limits, created_at FROM kanban_boards WHERE id=?1",
                params![board_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()
            .map_err(|e| KiasError::Config(format!("Load board: {}", e)))?;
        let Some((id, name, desc, wip_json, _)) = row else {
            return Ok(None);
        };
        let wip: Vec<WipLimit> = serde_json::from_str(&wip_json)
            .map_err(|e| KiasError::Config(format!("Parse WIP: {}", e)))?;
        let tasks = self.load_tasks_inner(&conn, board_id)?;
        Ok(Some(KanbanBoard {
            id,
            name,
            description: desc,
            tasks,
            wip_limits: wip,
            created_at: SystemTime::now(),
        }))
    }

    pub fn list_boards(&self) -> KiasResult<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name FROM kanban_boards ORDER BY created_at DESC")
            .map_err(|e| KiasError::Config(format!("Prepare: {}", e)))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| KiasError::Config(format!("List boards: {}", e)))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| KiasError::Config(format!("Row: {}", e)))?);
        }
        Ok(out)
    }

    pub fn delete_board(&self, board_id: &str) -> KiasResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM kanban_tasks WHERE board_id=?1", params![board_id])
            .map_err(|e| KiasError::Config(format!("Delete tasks: {}", e)))?;
        conn.execute("DELETE FROM kanban_boards WHERE id=?1", params![board_id])
            .map_err(|e| KiasError::Config(format!("Delete board: {}", e)))?;
        Ok(())
    }

    // ─── Task operations ───

    pub fn save_task(&self, task: &KanbanTask) -> KiasResult<()> {
        let conn = self.conn.lock().unwrap();
        self.save_task_inner(&conn, task)
    }

    fn save_task_inner(&self, conn: &Connection, t: &KanbanTask) -> KiasResult<()> {
        let se = |e: serde_json::Error| KiasError::Config(format!("Serialize: {}", e));
        conn.execute(
            "INSERT OR REPLACE INTO kanban_tasks (
                id, board_id, title, description, column_name, priority,
                assigned_to, required_capabilities, tags, estimated_minutes,
                blocked_by, parents, children, workspace, failure_count,
                block_reason, metadata, history, created_at, updated_at, column_entered_at
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
            params![
                t.id,
                t.board_id,
                t.title,
                t.description,
                t.column.to_string(),
                t.priority.to_int(),
                t.assigned_to,
                serde_json::to_string(&t.required_capabilities).map_err(&se)?,
                serde_json::to_string(&t.tags).map_err(&se)?,
                t.estimated_minutes.map(|v| v as i64),
                serde_json::to_string(&t.blocked_by).map_err(&se)?,
                serde_json::to_string(&t.parents).map_err(&se)?,
                serde_json::to_string(&t.children).map_err(&se)?,
                t.workspace,
                t.failure_count as i32,
                t.block_reason,
                serde_json::to_string(&t.metadata).map_err(&se)?,
                serde_json::to_string(&t.history).map_err(&se)?,
                st_to_str(t.created_at),
                st_to_str(t.updated_at),
                st_to_str(t.column_entered_at),
            ],
        )
        .map_err(|e| KiasError::Config(format!("Save task: {}", e)))?;
        Ok(())
    }

    pub fn load_task(&self, task_id: &str) -> KiasResult<Option<KanbanTask>> {
        let conn = self.conn.lock().unwrap();
        let r = conn
            .query_row(
                "SELECT id,board_id,title,description,column_name,priority,assigned_to,
                        required_capabilities,tags,estimated_minutes,blocked_by,parents,
                        children,workspace,failure_count,block_reason,metadata,history,
                        created_at,updated_at,column_entered_at
                 FROM kanban_tasks WHERE id=?1",
                params![task_id],
                row_to_raw,
            )
            .optional()
            .map_err(|e| KiasError::Config(format!("Load task: {}", e)))?;
        r.map(raw_to_task).transpose()
    }

    pub fn load_tasks_for_board(&self, board_id: &str) -> KiasResult<Vec<KanbanTask>> {
        let conn = self.conn.lock().unwrap();
        self.load_tasks_inner(&conn, board_id)
    }

    fn load_tasks_inner(&self, conn: &Connection, board_id: &str) -> KiasResult<Vec<KanbanTask>> {
        let mut stmt = conn
            .prepare(
                "SELECT id,board_id,title,description,column_name,priority,assigned_to,
                        required_capabilities,tags,estimated_minutes,blocked_by,parents,
                        children,workspace,failure_count,block_reason,metadata,history,
                        created_at,updated_at,column_entered_at
                 FROM kanban_tasks WHERE board_id=?1 ORDER BY priority ASC, created_at ASC",
            )
            .map_err(|e| KiasError::Config(format!("Prepare: {}", e)))?;
        let rows = stmt
            .query_map(params![board_id], row_to_raw)
            .map_err(|e| KiasError::Config(format!("Query: {}", e)))?;
        let mut tasks = Vec::new();
        for r in rows {
            let raw = r.map_err(|e| KiasError::Config(format!("Row: {}", e)))?;
            tasks.push(raw_to_task(raw)?);
        }
        Ok(tasks)
    }

    pub fn tasks_in_column(
        &self,
        board_id: &str,
        column: KanbanColumn,
    ) -> KiasResult<Vec<KanbanTask>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id,board_id,title,description,column_name,priority,assigned_to,
                        required_capabilities,tags,estimated_minutes,blocked_by,parents,
                        children,workspace,failure_count,block_reason,metadata,history,
                        created_at,updated_at,column_entered_at
                 FROM kanban_tasks WHERE board_id=?1 AND column_name=?2
                 ORDER BY priority ASC, created_at ASC",
            )
            .map_err(|e| KiasError::Config(format!("Prepare: {}", e)))?;
        let rows = stmt
            .query_map(params![board_id, column.to_string()], row_to_raw)
            .map_err(|e| KiasError::Config(format!("Query: {}", e)))?;
        let mut tasks = Vec::new();
        for r in rows {
            tasks.push(raw_to_task(r.map_err(|e| KiasError::Config(format!("Row: {}", e)))?)?);
        }
        Ok(tasks)
    }

    pub fn count_in_column(&self, board_id: &str, column: KanbanColumn) -> KiasResult<usize> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kanban_tasks WHERE board_id=?1 AND column_name=?2",
                params![board_id, column.to_string()],
                |r| r.get(0),
            )
            .map_err(|e| KiasError::Config(format!("Count: {}", e)))?;
        Ok(count as usize)
    }

    pub fn delete_task(&self, task_id: &str) -> KiasResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM kanban_tasks WHERE id=?1", params![task_id])
            .map_err(|e| KiasError::Config(format!("Delete: {}", e)))?;
        Ok(())
    }
}

// ─── Internal row type and conversions ───

type RawRow = (
    String,
    String,
    String,
    String,
    String,
    i32,
    Option<String>,
    String,
    String,
    Option<i64>,
    String,
    String,
    String,
    Option<String>,
    i32,
    Option<String>,
    String,
    String,
    String,
    String,
    String,
);

fn row_to_raw(row: &rusqlite::Row) -> rusqlite::Result<RawRow> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
        row.get(5)?,
        row.get(6)?,
        row.get(7)?,
        row.get(8)?,
        row.get(9)?,
        row.get(10)?,
        row.get(11)?,
        row.get(12)?,
        row.get(13)?,
        row.get(14)?,
        row.get(15)?,
        row.get(16)?,
        row.get(17)?,
        row.get(18)?,
        row.get(19)?,
        row.get(20)?,
    ))
}

fn raw_to_task(r: RawRow) -> KiasResult<KanbanTask> {
    let column = KanbanColumn::parse(&r.4)
        .ok_or_else(|| KiasError::Config(format!("Invalid column: {}", r.4)))?;
    Ok(KanbanTask {
        id: r.0,
        board_id: r.1,
        title: r.2,
        description: r.3,
        column,
        priority: Priority::from_int(r.5),
        assigned_to: r.6,
        required_capabilities: serde_json::from_str(&r.7)
            .map_err(|e| KiasError::Config(format!("Parse caps: {}", e)))?,
        tags: serde_json::from_str(&r.8)
            .map_err(|e| KiasError::Config(format!("Parse tags: {}", e)))?,
        estimated_minutes: r.9.map(|v| v as u64),
        blocked_by: serde_json::from_str(&r.10)
            .map_err(|e| KiasError::Config(format!("Parse blocked_by: {}", e)))?,
        parents: serde_json::from_str(&r.11)
            .map_err(|e| KiasError::Config(format!("Parse parents: {}", e)))?,
        children: serde_json::from_str(&r.12)
            .map_err(|e| KiasError::Config(format!("Parse children: {}", e)))?,
        workspace: r.13,
        failure_count: r.14 as u32,
        block_reason: r.15,
        metadata: serde_json::from_str(&r.16)
            .map_err(|e| KiasError::Config(format!("Parse metadata: {}", e)))?,
        history: serde_json::from_str(&r.17)
            .map_err(|e| KiasError::Config(format!("Parse history: {}", e)))?,
        created_at: str_to_st(&r.18),
        updated_at: str_to_st(&r.19),
        column_entered_at: str_to_st(&r.20),
    })
}

fn st_to_str(t: SystemTime) -> String {
    let dur = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

fn str_to_st(s: &str) -> SystemTime {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| {
            let d = dt.signed_duration_since(chrono::DateTime::UNIX_EPOCH);
            UNIX_EPOCH + Duration::from_secs(d.num_seconds().max(0) as u64)
        })
        .unwrap_or_else(|_| SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;

    fn make_task(id: &str, board_id: &str, title: &str) -> KanbanTask {
        KanbanTask {
            id: id.to_string(),
            board_id: board_id.to_string(),
            title: title.to_string(),
            description: "desc".to_string(),
            column: KanbanColumn::Triage,
            priority: Priority::Medium,
            required_capabilities: vec![],
            assigned_to: None,
            created_at: SystemTime::now(),
            column_entered_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            history: vec![],
            tags: vec!["test".to_string()],
            estimated_minutes: Some(30),
            blocked_by: vec![],
            parents: vec![],
            children: vec![],
            workspace: None,
            failure_count: 0,
            block_reason: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_save_and_load_board() {
        let store = KanbanStore::new_in_memory().unwrap();
        let board = KanbanBoard::new("b1", "Test Board");
        store.save_board(&board).unwrap();
        let loaded = store.load_board("b1").unwrap().unwrap();
        assert_eq!(loaded.id, "b1");
        assert_eq!(loaded.name, "Test Board");
    }

    #[test]
    fn test_list_boards() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board 1")).unwrap();
        store.save_board(&KanbanBoard::new("b2", "Board 2")).unwrap();
        let boards = store.list_boards().unwrap();
        assert_eq!(boards.len(), 2);
    }

    #[test]
    fn test_delete_board() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board")).unwrap();
        store.save_task(&make_task("t1", "b1", "Task")).unwrap();
        store.delete_board("b1").unwrap();
        assert!(store.load_board("b1").unwrap().is_none());
    }

    #[test]
    fn test_save_and_load_task() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board")).unwrap();
        let mut task = make_task("t1", "b1", "My Task");
        task.assigned_to = Some("agent-1".to_string());
        task.column = KanbanColumn::InProgress;
        task.blocked_by = vec!["t0".to_string()];
        task.parents = vec!["t0".to_string()];
        task.children = vec!["t2".to_string()];
        store.save_task(&task).unwrap();

        let loaded = store.load_task("t1").unwrap().unwrap();
        assert_eq!(loaded.id, "t1");
        assert_eq!(loaded.column, KanbanColumn::InProgress);
        assert_eq!(loaded.assigned_to, Some("agent-1".to_string()));
        assert_eq!(loaded.blocked_by, vec!["t0".to_string()]);
        assert_eq!(loaded.parents, vec!["t0".to_string()]);
        assert_eq!(loaded.children, vec!["t2".to_string()]);
    }

    #[test]
    fn test_load_tasks_for_board() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board")).unwrap();
        store.save_board(&KanbanBoard::new("b2", "Board2")).unwrap();
        store.save_task(&make_task("t1", "b1", "Task 1")).unwrap();
        store.save_task(&make_task("t2", "b1", "Task 2")).unwrap();
        store.save_task(&make_task("t3", "b2", "Task 3")).unwrap();
        let tasks = store.load_tasks_for_board("b1").unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_tasks_in_column() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board")).unwrap();
        let mut t1 = make_task("t1", "b1", "A");
        t1.column = KanbanColumn::Todo;
        let mut t2 = make_task("t2", "b1", "B");
        t2.column = KanbanColumn::InProgress;
        let mut t3 = make_task("t3", "b1", "C");
        t3.column = KanbanColumn::Todo;
        store.save_task(&t1).unwrap();
        store.save_task(&t2).unwrap();
        store.save_task(&t3).unwrap();

        let todo = store.tasks_in_column("b1", KanbanColumn::Todo).unwrap();
        assert_eq!(todo.len(), 2);
        assert_eq!(store.count_in_column("b1", KanbanColumn::Todo).unwrap(), 2);
    }

    #[test]
    fn test_upsert() {
        let store = KanbanStore::new_in_memory().unwrap();
        store.save_board(&KanbanBoard::new("b1", "Board")).unwrap();
        let mut task = make_task("t1", "b1", "Original");
        store.save_task(&task).unwrap();
        task.title = "Updated".to_string();
        task.column = KanbanColumn::Done;
        store.save_task(&task).unwrap();
        let loaded = store.load_task("t1").unwrap().unwrap();
        assert_eq!(loaded.title, "Updated");
        assert_eq!(loaded.column, KanbanColumn::Done);
    }

    #[test]
    fn test_nonexistent() {
        let store = KanbanStore::new_in_memory().unwrap();
        assert!(store.load_board("nope").unwrap().is_none());
        assert!(store.load_task("nope").unwrap().is_none());
    }
}
