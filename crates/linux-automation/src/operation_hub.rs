//! OperationHub — SAP Agent Hub 架构启发的操作管理中心
//!
//! 三大组件:
//! - OperationRegistry: 中央注册表 (登记/治理/监控)
//! - OperationGraph: 知识图谱 (操作依赖/关系)
//! - OperationMemory: 组织记忆 (历史学习/模式提取)
//!
//! 灵魂: 可追溯(注册即审计) / 透明(状态实时可见) / 可控(预审+策略)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::audit::AuditLog;
use crate::error::Result;
use crate::models::*;

// ============================================================
// OperationRegistry: 中央注册表
// ============================================================

/// 操作注册表 — 每个操作必须注册后才能执行
pub struct OperationRegistry {
    /// 已注册的操作 (id -> OperationRecord)
    operations: HashMap<Uuid, OperationRecord>,
    /// 按主机索引
    by_host: HashMap<String, Vec<Uuid>>,
    /// 按类型索引
    by_type: HashMap<String, Vec<Uuid>>,
}

/// 操作记录 — 注册时创建，全生命周期追踪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRecord {
    pub id: Uuid,
    pub op_type: String,
    pub target_host: String,
    pub description: String,
    pub status: OperationStatus,
    pub registered_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub registered_by: String,
    pub result_summary: Option<String>,
    pub tags: Vec<String>,
}

/// 操作状态机: Registered → Approved → Running → Completed/Failed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    /// 已注册，等待审批
    Registered,
    /// 已审批，等待执行
    Approved,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed,
    /// 已取消
    Cancelled,
    /// 已回滚
    RolledBack,
}

impl OperationRegistry {
    pub fn new() -> Self {
        Self {
            operations: HashMap::new(),
            by_host: HashMap::new(),
            by_type: HashMap::new(),
        }
    }

    /// 注册操作 (执行前必须调用)
    pub fn register(
        &mut self,
        op_type: &str,
        target_host: &str,
        description: &str,
        registered_by: &str,
        tags: Vec<String>,
        audit: &AuditLog,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let record = OperationRecord {
            id,
            op_type: op_type.to_string(),
            target_host: target_host.to_string(),
            description: description.to_string(),
            status: OperationStatus::Registered,
            registered_at: Utc::now(),
            started_at: None,
            completed_at: None,
            registered_by: registered_by.to_string(),
            result_summary: None,
            tags,
        };

        // 索引
        self.by_host
            .entry(target_host.to_string())
            .or_default()
            .push(id);
        self.by_type
            .entry(op_type.to_string())
            .or_default()
            .push(id);
        self.operations.insert(id, record);

        // 可追溯: 注册即审计
        audit.log_action(
            registered_by,
            "Register",
            target_host,
            &format!("{}: {}", op_type, description),
        )?;

        Ok(id)
    }

    /// 审批操作
    pub fn approve(&mut self, id: &Uuid, approver: &str, audit: &AuditLog) -> Result<()> {
        if let Some(record) = self.operations.get_mut(id) {
            if record.status == OperationStatus::Registered {
                record.status = OperationStatus::Approved;
                audit.log_action(
                    approver,
                    "Approve",
                    &record.target_host,
                    &format!("操作 {} 已审批", id),
                )?;
            }
        }
        Ok(())
    }

    /// 标记开始执行
    pub fn start(&mut self, id: &Uuid) -> Result<()> {
        if let Some(record) = self.operations.get_mut(id) {
            record.status = OperationStatus::Running;
            record.started_at = Some(Utc::now());
        }
        Ok(())
    }

    /// 标记完成
    pub fn complete(
        &mut self,
        id: &Uuid,
        success: bool,
        summary: &str,
        audit: &AuditLog,
    ) -> Result<()> {
        if let Some(record) = self.operations.get_mut(id) {
            record.status = if success {
                OperationStatus::Completed
            } else {
                OperationStatus::Failed
            };
            record.completed_at = Some(Utc::now());
            record.result_summary = Some(summary.to_string());

            audit.log_action(
                &record.registered_by,
                if success { "Complete" } else { "Fail" },
                &record.target_host,
                summary,
            )?;
        }
        Ok(())
    }

    /// 取消操作
    pub fn cancel(&mut self, id: &Uuid, reason: &str, audit: &AuditLog) -> Result<()> {
        if let Some(record) = self.operations.get_mut(id) {
            record.status = OperationStatus::Cancelled;
            record.completed_at = Some(Utc::now());
            record.result_summary = Some(format!("取消: {}", reason));
            audit.log_action("system", "Cancel", &record.target_host, reason)?;
        }
        Ok(())
    }

    /// 查询操作
    pub fn get(&self, id: &Uuid) -> Option<&OperationRecord> {
        self.operations.get(id)
    }

    /// 按主机查询
    pub fn by_host(&self, host: &str) -> Vec<&OperationRecord> {
        self.by_host
            .get(host)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.operations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按类型查询
    pub fn by_type(&self, op_type: &str) -> Vec<&OperationRecord> {
        self.by_type
            .get(op_type)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.operations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 按状态查询
    pub fn by_status(&self, status: &OperationStatus) -> Vec<&OperationRecord> {
        self.operations
            .values()
            .filter(|r| r.status == *status)
            .collect()
    }

    /// 统计
    pub fn statistics(&self) -> RegistryStatistics {
        let total = self.operations.len();
        let mut by_status = HashMap::new();
        let mut by_type = HashMap::new();

        for record in self.operations.values() {
            *by_status
                .entry(format!("{:?}", record.status))
                .or_insert(0usize) += 1;
            *by_type.entry(record.op_type.clone()).or_insert(0usize) += 1;
        }

        RegistryStatistics {
            total,
            by_status,
            by_type,
        }
    }

    /// 总操作数
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStatistics {
    pub total: usize,
    pub by_status: HashMap<String, usize>,
    pub by_type: HashMap<String, usize>,
}

// ============================================================
// OperationGraph: 知识图谱
// ============================================================

/// 操作关系类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationType {
    /// A 依赖 B (B必须先完成)
    DependsOn,
    /// A 触发 B (A完成后自动执行B)
    Triggers,
    /// A 是 B 的回滚操作
    RollbackOf,
    /// A 和 B 相关 (同一变更)
    RelatedTo,
    /// A 替代 B (B被废弃)
    Replaces,
}

/// 操作关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationRelation {
    pub from: Uuid,
    pub to: Uuid,
    pub relation: RelationType,
    pub description: String,
}

/// 操作知识图谱
pub struct OperationGraph {
    /// 关系列表
    relations: Vec<OperationRelation>,
    /// 按源操作索引
    by_source: HashMap<Uuid, Vec<usize>>,
    /// 按目标操作索引
    by_target: HashMap<Uuid, Vec<usize>>,
}

impl OperationGraph {
    pub fn new() -> Self {
        Self {
            relations: Vec::new(),
            by_source: HashMap::new(),
            by_target: HashMap::new(),
        }
    }

    /// 添加关系
    pub fn add_relation(
        &mut self,
        from: Uuid,
        to: Uuid,
        relation: RelationType,
        description: &str,
    ) {
        let idx = self.relations.len();
        self.relations.push(OperationRelation {
            from,
            to,
            relation,
            description: description.to_string(),
        });
        self.by_source.entry(from).or_default().push(idx);
        self.by_target.entry(to).or_default().push(idx);
    }

    /// 查询操作的所有依赖
    pub fn dependencies(&self, id: &Uuid) -> Vec<&OperationRelation> {
        self.by_source
            .get(id)
            .map(|idxs| {
                idxs.iter()
                    .filter_map(|i| self.relations.get(*i))
                    .filter(|r| r.relation == RelationType::DependsOn)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 查询操作触发的后续操作
    pub fn triggers(&self, id: &Uuid) -> Vec<&OperationRelation> {
        self.by_source
            .get(id)
            .map(|idxs| {
                idxs.iter()
                    .filter_map(|i| self.relations.get(*i))
                    .filter(|r| r.relation == RelationType::Triggers)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 查询操作的所有关系
    pub fn all_relations(&self, id: &Uuid) -> Vec<&OperationRelation> {
        let mut result = Vec::new();
        if let Some(idxs) = self.by_source.get(id) {
            result.extend(idxs.iter().filter_map(|i| self.relations.get(*i)));
        }
        if let Some(idxs) = self.by_target.get(id) {
            result.extend(idxs.iter().filter_map(|i| self.relations.get(*i)));
        }
        result
    }

    /// 关系总数
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }
}

// ============================================================
// OperationMemory: 组织记忆
// ============================================================

/// 操作统计 (按类型)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStats {
    pub op_type: String,
    pub total_count: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub success_rate: f64,
    pub avg_duration_ms: f64,
    pub common_failures: Vec<(String, usize)>,
}

/// 组织记忆 — 从历史操作中学习
pub struct OperationMemory {
    /// 按类型统计
    stats: HashMap<String, MemoryAccumulator>,
}

/// 累加器
#[derive(Debug, Clone)]
struct MemoryAccumulator {
    total: usize,
    success: usize,
    failure: usize,
    total_duration_ms: u64,
    failure_reasons: HashMap<String, usize>,
}

impl MemoryAccumulator {
    fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            failure: 0,
            total_duration_ms: 0,
            failure_reasons: HashMap::new(),
        }
    }

    fn record(&mut self, success: bool, duration_ms: u64, failure_reason: Option<&str>) {
        self.total += 1;
        if success {
            self.success += 1;
        } else {
            self.failure += 1;
            if let Some(reason) = failure_reason {
                *self.failure_reasons.entry(reason.to_string()).or_insert(0) += 1;
            }
        }
        self.total_duration_ms += duration_ms;
    }

    fn to_stats(&self, op_type: &str) -> OperationStats {
        let success_rate = if self.total > 0 {
            self.success as f64 / self.total as f64
        } else {
            0.0
        };
        let avg_duration_ms = if self.total > 0 {
            self.total_duration_ms as f64 / self.total as f64
        } else {
            0.0
        };
        let mut common_failures: Vec<(String, usize)> = self
            .failure_reasons
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        common_failures.sort_by(|a, b| b.1.cmp(&a.1));

        OperationStats {
            op_type: op_type.to_string(),
            total_count: self.total,
            success_count: self.success,
            failure_count: self.failure,
            success_rate,
            avg_duration_ms,
            common_failures,
        }
    }
}

impl OperationMemory {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    /// 记录操作结果
    pub fn record(
        &mut self,
        op_type: &str,
        success: bool,
        duration_ms: u64,
        failure_reason: Option<&str>,
    ) {
        let acc = self
            .stats
            .entry(op_type.to_string())
            .or_insert_with(MemoryAccumulator::new);
        acc.record(success, duration_ms, failure_reason);
    }

    /// 从 OperationRecord 批量学习
    pub fn learn_from_records(&mut self, records: &[&OperationRecord]) {
        for record in records {
            if let Some(completed) = record.completed_at {
                let duration_ms = if let Some(started) = record.started_at {
                    (completed - started).num_milliseconds().max(0) as u64
                } else {
                    0
                };
                let success = record.status == OperationStatus::Completed;
                let failure_reason = if !success {
                    record.result_summary.as_deref()
                } else {
                    None
                };
                self.record(&record.op_type, success, duration_ms, failure_reason);
            }
        }
    }

    /// 查询操作统计
    pub fn get_stats(&self, op_type: &str) -> Option<OperationStats> {
        self.stats.get(op_type).map(|acc| acc.to_stats(op_type))
    }

    /// 所有操作统计
    pub fn all_stats(&self) -> Vec<OperationStats> {
        self.stats
            .iter()
            .map(|(op_type, acc)| acc.to_stats(op_type))
            .collect()
    }

    /// 查询成功率
    pub fn success_rate(&self, op_type: &str) -> f64 {
        self.stats
            .get(op_type)
            .map(|acc| {
                if acc.total > 0 {
                    acc.success as f64 / acc.total as f64
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0)
    }

    /// 查询常见失败原因
    pub fn common_failures(&self, op_type: &str) -> Vec<(String, usize)> {
        self.stats
            .get(op_type)
            .map(|acc| {
                let mut v: Vec<(String, usize)> = acc
                    .failure_reasons
                    .iter()
                    .map(|(k, v)| (k.clone(), *v))
                    .collect();
                v.sort_by(|a, b| b.1.cmp(&a.1));
                v
            })
            .unwrap_or_default()
    }
}

// ============================================================
// OperationHub: 统一入口 (整体性)
// ============================================================

/// 操作管理中心 — 统一入口
pub struct OperationHub {
    pub registry: OperationRegistry,
    pub graph: OperationGraph,
    pub memory: OperationMemory,
}

impl OperationHub {
    pub fn new() -> Self {
        Self {
            registry: OperationRegistry::new(),
            graph: OperationGraph::new(),
            memory: OperationMemory::new(),
        }
    }

    /// 注册并执行操作 (一键调用)
    pub fn register_and_execute(
        &mut self,
        op_type: &str,
        target_host: &str,
        description: &str,
        registered_by: &str,
        tags: Vec<String>,
        audit: &AuditLog,
    ) -> Result<Uuid> {
        // 1. 注册
        let id = self.registry.register(
            op_type,
            target_host,
            description,
            registered_by,
            tags,
            audit,
        )?;

        // 2. 自动审批 (可扩展为人工审批)
        self.registry.approve(&id, "system", audit)?;

        // 3. 标记开始
        self.registry.start(&id)?;

        Ok(id)
    }

    /// 完成操作并学习
    pub fn complete_operation(
        &mut self,
        id: &Uuid,
        success: bool,
        summary: &str,
        audit: &AuditLog,
    ) -> Result<()> {
        // 1. 标记完成
        self.registry.complete(id, success, summary, audit)?;

        // 2. 学习: 记录到组织记忆
        if let Some(record) = self.registry.get(id) {
            let duration_ms =
                if let (Some(start), Some(end)) = (record.started_at, record.completed_at) {
                    (end - start).num_milliseconds().max(0) as u64
                } else {
                    0
                };
            let failure_reason = if !success { Some(summary) } else { None };
            self.memory
                .record(&record.op_type, success, duration_ms, failure_reason);
        }

        Ok(())
    }

    /// 添加操作关系
    pub fn relate(&mut self, from: Uuid, to: Uuid, relation: RelationType, description: &str) {
        self.graph.add_relation(from, to, relation, description);
    }

    /// 查询操作的完整上下文 (注册信息 + 关系 + 历史统计)
    pub fn operation_context(&self, id: &Uuid) -> Option<OperationContext> {
        let record = self.registry.get(id)?;
        let relations = self.graph.all_relations(id);
        let stats = self.memory.get_stats(&record.op_type);

        Some(OperationContext {
            record: record.clone(),
            relations: relations.into_iter().cloned().collect(),
            stats,
        })
    }
}

/// 操作上下文 (综合视图)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationContext {
    pub record: OperationRecord,
    pub relations: Vec<OperationRelation>,
    pub stats: Option<OperationStats>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_hub() -> (OperationHub, AuditLog, TempDir) {
        let tmp = TempDir::new().unwrap();
        let audit = AuditLog::new(&tmp.path().join("test.db")).unwrap();
        let hub = OperationHub::new();
        (hub, audit, tmp)
    }

    // === OperationRegistry 测试 ===

    #[test]
    fn test_registry_register() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .registry
            .register(
                "HealthCheck",
                "server1",
                "CPU巡检",
                "admin",
                vec!["daily".to_string()],
                &audit,
            )
            .unwrap();
        assert!(hub.registry.get(&id).is_some());
        assert_eq!(hub.registry.len(), 1);
    }

    #[test]
    fn test_registry_lifecycle() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .registry
            .register("DockerOps", "server1", "重启nginx", "admin", vec![], &audit)
            .unwrap();

        // Registered → Approved → Running → Completed
        assert_eq!(
            hub.registry.get(&id).unwrap().status,
            OperationStatus::Registered
        );
        hub.registry.approve(&id, "manager", &audit).unwrap();
        assert_eq!(
            hub.registry.get(&id).unwrap().status,
            OperationStatus::Approved
        );
        hub.registry.start(&id).unwrap();
        assert_eq!(
            hub.registry.get(&id).unwrap().status,
            OperationStatus::Running
        );
        hub.registry.complete(&id, true, "成功", &audit).unwrap();
        assert_eq!(
            hub.registry.get(&id).unwrap().status,
            OperationStatus::Completed
        );
    }

    #[test]
    fn test_registry_cancel() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .registry
            .register("K8sOps", "k8s", "删除Pod", "admin", vec![], &audit)
            .unwrap();
        hub.registry.cancel(&id, "用户取消", &audit).unwrap();
        assert_eq!(
            hub.registry.get(&id).unwrap().status,
            OperationStatus::Cancelled
        );
    }

    #[test]
    fn test_registry_by_host() {
        let (mut hub, audit, _tmp) = create_test_hub();
        hub.registry
            .register("HealthCheck", "server1", "巡检1", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("HealthCheck", "server1", "巡检2", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("DockerOps", "server2", "Docker", "admin", vec![], &audit)
            .unwrap();

        assert_eq!(hub.registry.by_host("server1").len(), 2);
        assert_eq!(hub.registry.by_host("server2").len(), 1);
        assert_eq!(hub.registry.by_host("server3").len(), 0);
    }

    #[test]
    fn test_registry_by_type() {
        let (mut hub, audit, _tmp) = create_test_hub();
        hub.registry
            .register("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("DockerOps", "s1", "Docker", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("HealthCheck", "s2", "巡检", "admin", vec![], &audit)
            .unwrap();

        assert_eq!(hub.registry.by_type("HealthCheck").len(), 2);
        assert_eq!(hub.registry.by_type("DockerOps").len(), 1);
    }

    #[test]
    fn test_registry_by_status() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .registry
            .register("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("DockerOps", "s1", "Docker", "admin", vec![], &audit)
            .unwrap();

        assert_eq!(
            hub.registry.by_status(&OperationStatus::Registered).len(),
            2
        );
        hub.registry.approve(&id, "mgr", &audit).unwrap();
        assert_eq!(
            hub.registry.by_status(&OperationStatus::Registered).len(),
            1
        );
        assert_eq!(hub.registry.by_status(&OperationStatus::Approved).len(), 1);
    }

    #[test]
    fn test_registry_statistics() {
        let (mut hub, audit, _tmp) = create_test_hub();
        hub.registry
            .register("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        hub.registry
            .register("DockerOps", "s1", "Docker", "admin", vec![], &audit)
            .unwrap();

        let stats = hub.registry.statistics();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_status.get("Registered"), Some(&2));
    }

    // === OperationGraph 测试 ===

    #[test]
    fn test_graph_add_relation() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        hub.graph
            .add_relation(id1, id2, RelationType::DependsOn, "检查依赖重启");

        assert_eq!(hub.graph.len(), 1);
        assert_eq!(hub.graph.dependencies(&id1).len(), 1);
    }

    #[test]
    fn test_graph_triggers() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        hub.graph
            .add_relation(id1, id2, RelationType::Triggers, "巡检触发修复");

        assert_eq!(hub.graph.triggers(&id1).len(), 1);
        assert_eq!(hub.graph.dependencies(&id1).len(), 0);
    }

    #[test]
    fn test_graph_rollback() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        hub.graph
            .add_relation(id1, id2, RelationType::RollbackOf, "回滚部署");

        let rels = hub.graph.all_relations(&id1);
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].relation, RelationType::RollbackOf);
    }

    #[test]
    fn test_graph_all_relations() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();
        hub.graph
            .add_relation(id1, id2, RelationType::DependsOn, "A依赖B");
        hub.graph
            .add_relation(id3, id1, RelationType::Triggers, "C触发A");

        // id1 有2个关系: 作为源1个, 作为目标1个
        assert_eq!(hub.graph.all_relations(&id1).len(), 2);
    }

    // === OperationMemory 测试 ===

    #[test]
    fn test_memory_record() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        hub.memory.record("HealthCheck", true, 100, None);
        hub.memory.record("HealthCheck", true, 200, None);
        hub.memory.record("HealthCheck", false, 150, Some("超时"));

        let stats = hub.memory.get_stats("HealthCheck").unwrap();
        assert_eq!(stats.total_count, 3);
        assert_eq!(stats.success_count, 2);
        assert_eq!(stats.failure_count, 1);
        assert!((stats.success_rate - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_memory_success_rate() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        hub.memory.record("DockerOps", true, 50, None);
        hub.memory.record("DockerOps", true, 60, None);

        assert!((hub.memory.success_rate("DockerOps") - 1.0).abs() < 0.01);
        assert_eq!(hub.memory.success_rate("Unknown"), 0.0);
    }

    #[test]
    fn test_memory_common_failures() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        hub.memory.record("K8sOps", false, 100, Some("连接超时"));
        hub.memory.record("K8sOps", false, 100, Some("连接超时"));
        hub.memory.record("K8sOps", false, 100, Some("权限不足"));

        let failures = hub.memory.common_failures("K8sOps");
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].0, "连接超时");
        assert_eq!(failures[0].1, 2);
    }

    #[test]
    fn test_memory_learn_from_records() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .registry
            .register("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        hub.registry.start(&id).unwrap();
        hub.registry.complete(&id, true, "成功", &audit).unwrap();

        let records: Vec<&OperationRecord> = hub.registry.by_type("HealthCheck");
        hub.memory.learn_from_records(&records);

        let stats = hub.memory.get_stats("HealthCheck");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().total_count, 1);
    }

    #[test]
    fn test_memory_all_stats() {
        let (mut hub, _audit, _tmp) = create_test_hub();
        hub.memory.record("HealthCheck", true, 100, None);
        hub.memory.record("DockerOps", true, 50, None);

        let all = hub.memory.all_stats();
        assert_eq!(all.len(), 2);
    }

    // === OperationHub 集成测试 ===

    #[test]
    fn test_hub_register_and_execute() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .register_and_execute(
                "HealthCheck",
                "server1",
                "CPU巡检",
                "admin",
                vec!["daily".to_string()],
                &audit,
            )
            .unwrap();

        let record = hub.registry.get(&id).unwrap();
        assert_eq!(record.status, OperationStatus::Running); // 自动审批+开始
    }

    #[test]
    fn test_hub_complete_and_learn() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .register_and_execute("DockerOps", "server1", "重启nginx", "admin", vec![], &audit)
            .unwrap();

        hub.complete_operation(&id, true, "重启成功", &audit)
            .unwrap();

        // 验证记忆已学习
        let stats = hub.memory.get_stats("DockerOps");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().success_count, 1);
    }

    #[test]
    fn test_hub_relate() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id1 = hub
            .register_and_execute("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        let id2 = hub
            .register_and_execute("DockerOps", "s1", "修复", "admin", vec![], &audit)
            .unwrap();

        hub.relate(id1, id2, RelationType::Triggers, "巡检发现问题触发修复");

        let ctx = hub.operation_context(&id1).unwrap();
        assert_eq!(ctx.relations.len(), 1);
        assert_eq!(ctx.relations[0].relation, RelationType::Triggers);
    }

    #[test]
    fn test_hub_operation_context() {
        let (mut hub, audit, _tmp) = create_test_hub();
        let id = hub
            .register_and_execute("HealthCheck", "s1", "巡检", "admin", vec![], &audit)
            .unwrap();
        hub.complete_operation(&id, true, "成功", &audit).unwrap();

        let ctx = hub.operation_context(&id).unwrap();
        assert_eq!(ctx.record.op_type, "HealthCheck");
        assert!(ctx.stats.is_some());
        assert_eq!(ctx.stats.unwrap().total_count, 1);
    }

    // === 序列化测试 ===

    #[test]
    fn test_operation_status_serialization() {
        let statuses = vec![
            OperationStatus::Registered,
            OperationStatus::Approved,
            OperationStatus::Running,
            OperationStatus::Completed,
            OperationStatus::Failed,
            OperationStatus::Cancelled,
            OperationStatus::RolledBack,
        ];
        for s in statuses {
            let json = serde_json::to_string(&s).unwrap();
            let d: OperationStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(s, d);
        }
    }

    #[test]
    fn test_relation_type_serialization() {
        let types = vec![
            RelationType::DependsOn,
            RelationType::Triggers,
            RelationType::RollbackOf,
            RelationType::RelatedTo,
            RelationType::Replaces,
        ];
        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let d: RelationType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, d);
        }
    }

    #[test]
    fn test_operation_record_serialization() {
        let record = OperationRecord {
            id: Uuid::new_v4(),
            op_type: "HealthCheck".to_string(),
            target_host: "server1".to_string(),
            description: "CPU巡检".to_string(),
            status: OperationStatus::Completed,
            registered_at: Utc::now(),
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            registered_by: "admin".to_string(),
            result_summary: Some("正常".to_string()),
            tags: vec!["daily".to_string()],
        };
        let json = serde_json::to_string(&record).unwrap();
        let d: OperationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(d.op_type, "HealthCheck");
        assert_eq!(d.status, OperationStatus::Completed);
    }
}
