//! 变更影响分析器
//!
//! 分析代码变更的影响范围，输出受影响的服务/租户/策略。

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// 影响范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactScope {
    pub affected_services: Vec<String>,
    pub affected_tenants: Vec<String>,
    pub affected_policies: Vec<String>,
    pub affected_workflows: Vec<String>,
    pub risk_level: RiskLevel,
}

impl ImpactScope {
    pub fn is_empty(&self) -> bool {
        self.affected_services.is_empty()
            && self.affected_tenants.is_empty()
            && self.affected_policies.is_empty()
            && self.affected_workflows.is_empty()
    }

    pub fn total_affected(&self) -> usize {
        self.affected_services.len()
            + self.affected_tenants.len()
            + self.affected_policies.len()
            + self.affected_workflows.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "Low"),
            RiskLevel::Medium => write!(f, "Medium"),
            RiskLevel::High => write!(f, "High"),
            RiskLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// 影响报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactReport {
    pub change_id: String,
    pub file_path: String,
    pub change_type: ChangeType,
    pub scope: ImpactScope,
    pub description: String,
    pub rollback_plan: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Add,
    Modify,
    Delete,
    Refactor,
}

/// 代码依赖图
#[derive(Default)]
pub struct DependencyGraph {
    /// 文件 -> 依赖的文件
    edges: HashMap<String, HashSet<String>>,
    /// 文件 -> 提供的服务
    service_map: HashMap<String, String>,
    /// 文件 -> 影响的租户
    tenant_map: HashMap<String, HashSet<String>>,
    /// 文件 -> 影响的策略
    policy_map: HashMap<String, HashSet<String>>,
    /// 文件 -> 影响的 Workflow
    workflow_map: HashMap<String, HashSet<String>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加依赖关系
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
    }

    /// 映射文件到服务
    pub fn map_file_to_service(&mut self, file: &str, service: &str) {
        self.service_map
            .insert(file.to_string(), service.to_string());
    }

    /// 映射文件到租户
    pub fn map_file_to_tenant(&mut self, file: &str, tenant: &str) {
        self.tenant_map
            .entry(file.to_string())
            .or_default()
            .insert(tenant.to_string());
    }

    /// 映射文件到策略
    pub fn map_file_to_policy(&mut self, file: &str, policy: &str) {
        self.policy_map
            .entry(file.to_string())
            .or_default()
            .insert(policy.to_string());
    }

    /// 映射文件到 Workflow
    pub fn map_file_to_workflow(&mut self, file: &str, workflow: &str) {
        self.workflow_map
            .entry(file.to_string())
            .or_default()
            .insert(workflow.to_string());
    }

    /// 收集所有受影响的文件（递归）
    fn collect_affected(&self, file: &str, visited: &mut HashSet<String>) {
        if visited.contains(file) {
            return;
        }
        visited.insert(file.to_string());
        if let Some(deps) = self.edges.get(file) {
            for dep in deps {
                self.collect_affected(dep, visited);
            }
        }
    }

    /// 分析变更影响
    pub fn analyze(&self, changed_file: &str, change_type: ChangeType) -> ImpactReport {
        let mut visited = HashSet::new();
        self.collect_affected(changed_file, &mut visited);

        let mut affected_services = HashSet::new();
        let mut affected_tenants = HashSet::new();
        let mut affected_policies = HashSet::new();
        let mut affected_workflows = HashSet::new();

        for file in &visited {
            if let Some(service) = self.service_map.get(file) {
                affected_services.insert(service.clone());
            }
            if let Some(tenants) = self.tenant_map.get(file) {
                affected_tenants.extend(tenants.clone());
            }
            if let Some(policies) = self.policy_map.get(file) {
                affected_policies.extend(policies.clone());
            }
            if let Some(workflows) = self.workflow_map.get(file) {
                affected_workflows.extend(workflows.clone());
            }
        }

        let risk_level = self.calculate_risk(change_type, &visited);

        ImpactReport {
            change_id: uuid::Uuid::new_v4().to_string(),
            file_path: changed_file.to_string(),
            change_type,
            scope: ImpactScope {
                affected_services: affected_services.into_iter().collect(),
                affected_tenants: affected_tenants.into_iter().collect(),
                affected_policies: affected_policies.into_iter().collect(),
                affected_workflows: affected_workflows.into_iter().collect(),
                risk_level,
            },
            description: format!("Change to {} affects {} files", changed_file, visited.len()),
            rollback_plan: self.generate_rollback_plan(change_type),
            timestamp: chrono::Utc::now(),
        }
    }

    fn calculate_risk(&self, change_type: ChangeType, affected: &HashSet<String>) -> RiskLevel {
        if affected.is_empty() {
            return RiskLevel::Low;
        }
        let score = affected.len();
        let type_multiplier = match change_type {
            ChangeType::Delete => 3,
            ChangeType::Modify => 2,
            ChangeType::Refactor => 1,
            ChangeType::Add => 1,
        };
        let total = score * type_multiplier;
        if total >= 15 {
            RiskLevel::Critical
        } else if total >= 9 {
            RiskLevel::High
        } else if total >= 5 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    fn generate_rollback_plan(&self, change_type: ChangeType) -> Option<String> {
        match change_type {
            ChangeType::Delete => Some("Restore from git history".to_string()),
            ChangeType::Modify => Some("Revert to previous version from VCS".to_string()),
            ChangeType::Refactor => Some("Ensure API compatibility before deploying".to_string()),
            ChangeType::Add => Some("Remove new code and redeploy".to_string()),
        }
    }
}

/// 变更影响分析器
pub struct ChangeImpactAnalyzer {
    dependency_graph: DependencyGraph,
}

impl Default for ChangeImpactAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ChangeImpactAnalyzer {
    pub fn new() -> Self {
        Self {
            dependency_graph: DependencyGraph::new(),
        }
    }

    /// 注册依赖关系
    pub fn register_dependency(&mut self, from: &str, to: &str) {
        self.dependency_graph.add_dependency(from, to);
    }

    /// 注册文件到服务的映射
    pub fn register_service_mapping(&mut self, file: &str, service: &str) {
        self.dependency_graph.map_file_to_service(file, service);
    }

    /// 注册文件到租户的映射
    pub fn register_tenant_mapping(&mut self, file: &str, tenant: &str) {
        self.dependency_graph.map_file_to_tenant(file, tenant);
    }

    /// 注册文件到策略的映射
    pub fn register_policy_mapping(&mut self, file: &str, policy: &str) {
        self.dependency_graph.map_file_to_policy(file, policy);
    }

    /// 注册文件到 Workflow 的映射
    pub fn register_workflow_mapping(&mut self, file: &str, workflow: &str) {
        self.dependency_graph.map_file_to_workflow(file, workflow);
    }

    /// 分析变更影响
    pub fn analyze(&self, file: &str, change_type: ChangeType) -> ImpactReport {
        self.dependency_graph.analyze(file, change_type)
    }
}

// ============== 测试 ==============

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_impact_analyzer_basic() {
        let mut analyzer = ChangeImpactAnalyzer::new();
        analyzer.register_dependency("src/main.rs", "src/lib.rs");
        analyzer.register_service_mapping("src/lib.rs", "core-service");
        let report = analyzer.analyze("src/main.rs", ChangeType::Modify);
        assert!(report
            .scope
            .affected_services
            .contains(&"core-service".to_string()));
    }

    #[test]
    fn test_change_impact_analyzer_delete_high_risk() {
        let mut analyzer = ChangeImpactAnalyzer::new();
        analyzer.register_dependency("src/main.rs", "src/lib.rs");
        analyzer.register_dependency("src/lib.rs", "src/core.rs");
        analyzer.register_service_mapping("src/core.rs", "core-service");
        analyzer.register_tenant_mapping("src/core.rs", "tenant-1");
        let report = analyzer.analyze("src/main.rs", ChangeType::Delete);
        assert_eq!(report.scope.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_change_impact_analyzer_add_low_risk() {
        let mut analyzer = ChangeImpactAnalyzer::new();
        let report = analyzer.analyze("src/new_file.rs", ChangeType::Add);
        assert_eq!(report.scope.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_impact_scope_is_empty() {
        let scope = ImpactScope {
            affected_services: vec![],
            affected_tenants: vec![],
            affected_policies: vec![],
            affected_workflows: vec![],
            risk_level: RiskLevel::Low,
        };
        assert!(scope.is_empty());
    }

    #[test]
    fn test_impact_scope_total_affected() {
        let scope = ImpactScope {
            affected_services: vec!["s1".to_string(), "s2".to_string()],
            affected_tenants: vec!["t1".to_string()],
            affected_policies: vec![],
            affected_workflows: vec!["w1".to_string()],
            risk_level: RiskLevel::Medium,
        };
        assert_eq!(scope.total_affected(), 4);
    }

    #[test]
    fn test_risk_level_display() {
        assert_eq!(RiskLevel::Low.to_string(), "Low");
        assert_eq!(RiskLevel::Medium.to_string(), "Medium");
        assert_eq!(RiskLevel::High.to_string(), "High");
        assert_eq!(RiskLevel::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_change_impact_analyzer_workflow_mapping() {
        let mut analyzer = ChangeImpactAnalyzer::new();
        analyzer.register_dependency("src/workflow.rs", "src/nodes.rs");
        analyzer.register_workflow_mapping("src/nodes.rs", "workflow-1");
        let report = analyzer.analyze("src/workflow.rs", ChangeType::Modify);
        assert!(report
            .scope
            .affected_workflows
            .contains(&"workflow-1".to_string()));
    }
}
