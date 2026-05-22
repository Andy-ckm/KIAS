//! 调试时光机
//!
//! 记录请求从入口到每个节点的状态，支持回放调试会话。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 步骤记录 - 每步的输入/输出/状态/时间戳
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_id: String,
    pub step_name: String,
    pub input: HashMap<String, String>,
    pub output: HashMap<String, String>,
    pub state: ExecutionState,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionState {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// 回放会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySession {
    pub session_id: String,
    pub request_id: String,
    pub steps: Vec<StepRecord>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

impl ReplaySession {
    pub fn new(request_id: String) -> Self {
        Self {
            session_id: Uuid::new_v4().to_string(),
            request_id,
            steps: Vec::new(),
            start_time: Utc::now(),
            end_time: None,
            metadata: HashMap::new(),
        }
    }

    /// 添加步骤记录
    pub fn add_step(&mut self, record: StepRecord) {
        self.steps.push(record);
    }

    /// 完成会话
    pub fn finish(&mut self) {
        self.end_time = Some(Utc::now());
    }

    /// 获取总耗时
    pub fn total_duration_ms(&self) -> Option<u64> {
        self.end_time
            .map(|end| (end - self.start_time).num_milliseconds() as u64)
    }

    /// 获取步骤数量
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// 获取失败步骤
    pub fn failed_steps(&self) -> Vec<&StepRecord> {
        self.steps
            .iter()
            .filter(|s| s.state == ExecutionState::Failed)
            .collect()
    }

    /// 从头开始回放
    pub fn replay_from_start(&self) -> Vec<&StepRecord> {
        self.steps.iter().collect()
    }

    /// 从指定步骤回放
    pub fn replay_from_step(&self, step_id: &str) -> Option<Vec<&StepRecord>> {
        let start_idx = self.steps.iter().position(|s| s.step_id == step_id)?;
        Some(self.steps[start_idx..].iter().collect())
    }
}

/// 时间旅行调试器
#[derive(Default)]
pub struct TimeTravelDebugger {
    sessions: HashMap<String, ReplaySession>,
}

impl TimeTravelDebugger {
    pub fn new() -> Self {
        Self::default()
    }

    /// 开始新的调试会话
    pub fn start_session(&mut self, request_id: String) -> String {
        let session = ReplaySession::new(request_id);
        let session_id = session.session_id.clone();
        self.sessions.insert(session_id.clone(), session);
        session_id
    }

    /// 记录步骤
    pub fn record_step(&mut self, session_id: &str, record: StepRecord) -> Result<(), DebugError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| DebugError::SessionNotFound(session_id.to_string()))?;
        session.add_step(record);
        Ok(())
    }

    /// 结束会话
    pub fn end_session(&mut self, session_id: &str) -> Result<(), DebugError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| DebugError::SessionNotFound(session_id.to_string()))?;
        session.finish();
        Ok(())
    }

    /// 获取会话
    pub fn get_session(&self, session_id: &str) -> Option<&ReplaySession> {
        self.sessions.get(session_id)
    }

    /// 获取所有会话
    pub fn list_sessions(&self) -> Vec<&ReplaySession> {
        self.sessions.values().collect()
    }

    /// 删除会话
    pub fn delete_session(&mut self, session_id: &str) -> Result<(), DebugError> {
        if self.sessions.remove(session_id).is_none() {
            return Err(DebugError::SessionNotFound(session_id.to_string()));
        }
        Ok(())
    }

    /// 获取会话数量
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 查找失败的会话
    pub fn find_failed_sessions(&self) -> Vec<&ReplaySession> {
        self.sessions
            .values()
            .filter(|s| !s.failed_steps().is_empty())
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DebugError {
    #[error("Session `{0}` not found")]
    SessionNotFound(String),
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_step(step_id: &str, state: ExecutionState) -> StepRecord {
        StepRecord {
            step_id: step_id.to_string(),
            step_name: format!("Step {}", step_id),
            input: HashMap::from([("key".to_string(), "value".to_string())]),
            output: HashMap::from([("result".to_string(), "ok".to_string())]),
            state,
            timestamp: Utc::now(),
            duration_ms: 100,
        }
    }

    #[test]
    fn test_time_travel_debugger_start_session() {
        let mut debugger = TimeTravelDebugger::new();
        let session_id = debugger.start_session("req-1".to_string());
        assert!(!session_id.is_empty());
        assert_eq!(debugger.session_count(), 1);
    }

    #[test]
    fn test_time_travel_debugger_record_step() {
        let mut debugger = TimeTravelDebugger::new();
        let session_id = debugger.start_session("req-1".to_string());
        debugger
            .record_step(
                &session_id,
                create_test_step("step1", ExecutionState::Success),
            )
            .unwrap();
        let session = debugger.get_session(&session_id).unwrap();
        assert_eq!(session.step_count(), 1);
    }

    #[test]
    fn test_time_travel_debugger_record_step_not_found() {
        let mut debugger = TimeTravelDebugger::new();
        let result = debugger.record_step(
            "nonexistent",
            create_test_step("step1", ExecutionState::Success),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_replay_session_failed_steps() {
        let mut session = ReplaySession::new("req-1".to_string());
        session.add_step(create_test_step("step1", ExecutionState::Success));
        session.add_step(create_test_step("step2", ExecutionState::Failed));
        session.add_step(create_test_step("step3", ExecutionState::Success));
        let failed = session.failed_steps();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].step_id, "step2");
    }

    #[test]
    fn test_replay_session_replay_from_step() {
        let mut session = ReplaySession::new("req-1".to_string());
        session.add_step(create_test_step("step1", ExecutionState::Success));
        session.add_step(create_test_step("step2", ExecutionState::Success));
        session.add_step(create_test_step("step3", ExecutionState::Success));
        let replay = session.replay_from_step("step2").unwrap();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].step_id, "step2");
    }

    #[test]
    fn test_time_travel_debugger_find_failed_sessions() {
        let mut debugger = TimeTravelDebugger::new();
        let sid1 = debugger.start_session("req-1".to_string());
        debugger
            .record_step(&sid1, create_test_step("step1", ExecutionState::Failed))
            .unwrap();
        let sid2 = debugger.start_session("req-2".to_string());
        debugger
            .record_step(&sid2, create_test_step("step1", ExecutionState::Success))
            .unwrap();
        let failed = debugger.find_failed_sessions();
        assert_eq!(failed.len(), 1);
    }

    #[test]
    fn test_execution_state_serialization() {
        let state = ExecutionState::Running;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ExecutionState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, state);
    }
}
