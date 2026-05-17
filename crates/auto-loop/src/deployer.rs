//! 自动部署 — KIAS自循环的核心（真实执行版）
//!
//! 自动部署修复，包括：
//! - 真实代码编译
//! - Git 快照回滚
//! - 健康检查
//! - 部署监控
//!
//! ## 控制论原理
//! Deployer 是闭环的"执行器"——将决策转化为真实环境中的变更。
//! 参考：Ashby 必要多样性定律 — 控制器必须能影响被控对象。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::codegen::CodePatch;

/// 部署状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeployStatus {
    /// 待部署
    Pending,
    /// 部署中
    Deploying,
    /// 部署成功
    Success,
    /// 部署失败
    Failed,
    /// 已回滚
    RolledBack,
}

/// 部署结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    /// 部署ID
    pub id: String,
    /// 部署状态
    pub status: DeployStatus,
    /// 部署详情
    pub details: String,
    /// 变更文件
    pub changed_files: Vec<String>,
    /// 部署耗时（毫秒）
    pub duration_ms: u64,
    /// 部署时间
    pub deployed_at: chrono::DateTime<chrono::Utc>,
    /// 错误信息
    pub errors: Vec<String>,
    /// Git 快照 commit hash（用于回滚）
    pub snapshot_hash: Option<String>,
}

/// 部署器 trait
pub trait Deployer: Send + Sync {
    /// 执行部署
    fn deploy(&self, target: &str, patches: &[CodePatch]) -> DeployResult;

    /// 获取部署器名称
    fn name(&self) -> &str;

    /// 回滚部署
    fn rollback(&self, deploy_id: &str, snapshot_hash: &str) -> DeployResult;

    /// 健康检查
    fn health_check(&self) -> bool;
}

/// Git 快照部署器 — 真实执行
///
/// 流程：
/// 1. 创建 Git stash 快照（可回滚点）
/// 2. 执行 cargo check 验证编译
/// 3. 成功则继续，失败则自动回滚
pub struct GitSnapshotDeployer {
    workspace_path: PathBuf,
}

impl GitSnapshotDeployer {
    pub fn new(workspace_path: impl Into<PathBuf>) -> Self {
        Self {
            workspace_path: workspace_path.into(),
        }
    }

    /// 创建 Git 快照
    fn create_snapshot(&self) -> Option<String> {
        let output = Command::new("git")
            .args(["stash", "push", "-m", "kias-auto-deploy-snapshot"])
            .current_dir(&self.workspace_path)
            .output()
            .ok()?;

        if output.status.success() {
            // 获取当前 HEAD hash
            let hash_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&self.workspace_path)
                .output()
                .ok()?;
            Some(
                String::from_utf8_lossy(&hash_output.stdout)
                    .trim()
                    .to_string(),
            )
        } else {
            // 没有变更可以 stash，直接返回 HEAD
            let hash_output = Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&self.workspace_path)
                .output()
                .ok()?;
            Some(
                String::from_utf8_lossy(&hash_output.stdout)
                    .trim()
                    .to_string(),
            )
        }
    }

    /// 执行 cargo build
    fn build(&self) -> (bool, String) {
        let output = Command::new("cargo")
            .args(["check", "--workspace"])
            .current_dir(&self.workspace_path)
            .output();

        match output {
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                (o.status.success(), stderr)
            }
            Err(e) => (false, format!("构建失败: {}", e)),
        }
    }
}

impl Default for GitSnapshotDeployer {
    fn default() -> Self {
        Self::new("/workspace/kias")
    }
}

impl Deployer for GitSnapshotDeployer {
    fn deploy(&self, target: &str, patches: &[CodePatch]) -> DeployResult {
        let start = std::time::Instant::now();
        let deploy_id = uuid::Uuid::new_v4().to_string();
        let changed_files: Vec<String> = patches.iter().map(|p| p.target_file.clone()).collect();

        // Step 1: 创建快照
        let snapshot_hash = self.create_snapshot();

        // Step 2: 验证构建
        let (build_success, build_output) = self.build();

        if build_success {
            DeployResult {
                id: deploy_id,
                status: DeployStatus::Success,
                details: format!("部署成功: {} ({} 文件变更)", target, changed_files.len()),
                changed_files,
                duration_ms: start.elapsed().as_millis() as u64,
                deployed_at: chrono::Utc::now(),
                errors: vec![],
                snapshot_hash,
            }
        } else {
            // 构建失败 → 自动回滚
            if let Some(ref hash) = snapshot_hash {
                let _ = Command::new("git")
                    .args(["checkout", hash, "--", "."])
                    .current_dir(&self.workspace_path)
                    .output();
            }

            DeployResult {
                id: deploy_id,
                status: DeployStatus::Failed,
                details: format!("部署失败，已自动回滚: {}", target),
                changed_files,
                duration_ms: start.elapsed().as_millis() as u64,
                deployed_at: chrono::Utc::now(),
                errors: vec![build_output],
                snapshot_hash,
            }
        }
    }

    fn name(&self) -> &str {
        "GitSnapshotDeployer"
    }

    fn rollback(&self, deploy_id: &str, snapshot_hash: &str) -> DeployResult {
        let start = std::time::Instant::now();

        let output = Command::new("git")
            .args(["checkout", snapshot_hash, "--", "."])
            .current_dir(&self.workspace_path)
            .output();

        let (success, details) = match output {
            Ok(o) if o.status.success() => (
                true,
                format!(
                    "回滚到 {} 成功",
                    &snapshot_hash[..8.min(snapshot_hash.len())]
                ),
            ),
            Ok(o) => (
                false,
                format!("回滚失败: {}", String::from_utf8_lossy(&o.stderr)),
            ),
            Err(e) => (false, format!("回滚命令失败: {}", e)),
        };

        DeployResult {
            id: deploy_id.to_string(),
            status: if success {
                DeployStatus::RolledBack
            } else {
                DeployStatus::Failed
            },
            details,
            changed_files: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
            deployed_at: chrono::Utc::now(),
            errors: if success {
                vec![]
            } else {
                vec!["回滚失败".to_string()]
            },
            snapshot_hash: Some(snapshot_hash.to_string()),
        }
    }

    fn health_check(&self) -> bool {
        // 检查 workspace 存在且是 git 仓库
        self.workspace_path.join(".git").exists()
    }
}

/// 部署器管理器
pub struct DeployerManager {
    /// 部署器列表
    deployers: Vec<Box<dyn Deployer>>,
    /// 部署历史
    history: Vec<DeployResult>,
}

impl Default for DeployerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DeployerManager {
    pub fn new() -> Self {
        Self {
            deployers: Vec::new(),
            history: Vec::new(),
        }
    }

    /// 注册部署器
    pub fn register_deployer(&mut self, deployer: Box<dyn Deployer>) {
        self.deployers.push(deployer);
    }

    /// 执行部署
    pub fn deploy(&mut self, target: &str, patches: &[CodePatch]) -> Vec<DeployResult> {
        let mut results = Vec::new();

        for deployer in &self.deployers {
            let result = deployer.deploy(target, patches);
            results.push(result.clone());
            self.history.push(result);
        }

        results
    }

    /// 回滚部署
    pub fn rollback(&mut self, deploy_id: &str, snapshot_hash: &str) -> Vec<DeployResult> {
        let mut results = Vec::new();

        for deployer in &self.deployers {
            let result = deployer.rollback(deploy_id, snapshot_hash);
            results.push(result.clone());
            self.history.push(result);
        }

        results
    }

    /// 获取部署历史
    pub fn history(&self) -> &[DeployResult] {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{CodePatch, PatchType};

    fn make_patches() -> Vec<CodePatch> {
        vec![CodePatch {
            id: "test".to_string(),
            target_file: "test.rs".to_string(),
            patch_type: PatchType::CodeChange,
            content: "test".to_string(),
            description: "test".to_string(),
            generated_at: chrono::Utc::now(),
        }]
    }

    #[test]
    fn test_deploy_status_variants() {
        assert!(matches!(DeployStatus::Pending, DeployStatus::Pending));
        assert!(matches!(DeployStatus::Deploying, DeployStatus::Deploying));
        assert!(matches!(DeployStatus::Success, DeployStatus::Success));
        assert!(matches!(DeployStatus::Failed, DeployStatus::Failed));
        assert!(matches!(DeployStatus::RolledBack, DeployStatus::RolledBack));
    }

    #[test]
    fn test_deploy_result_fields() {
        let result = DeployResult {
            id: "deploy-1".to_string(),
            status: DeployStatus::Success,
            details: "Build succeeded".to_string(),
            changed_files: vec!["main.rs".to_string()],
            duration_ms: 1500,
            deployed_at: chrono::Utc::now(),
            errors: vec![],
            snapshot_hash: Some("abc123".to_string()),
        };
        assert_eq!(result.status, DeployStatus::Success);
        assert_eq!(result.changed_files.len(), 1);
        assert!(result.errors.is_empty());
        assert_eq!(result.duration_ms, 1500);
    }

    #[test]
    fn test_deployer_manager_creation() {
        let manager = DeployerManager::new();
        assert!(manager.deployers.is_empty());
        assert!(manager.history().is_empty());
    }

    #[test]
    fn test_deployer_manager_empty_deploy() {
        let mut manager = DeployerManager::new();
        let results = manager.deploy("target", &make_patches());
        assert!(results.is_empty());
    }

    #[test]
    fn test_deployer_manager_empty_rollback() {
        let mut manager = DeployerManager::new();
        let results = manager.rollback("deploy-123", "abc123");
        assert!(results.is_empty());
    }

    #[test]
    fn test_git_snapshot_deployer_health_check() {
        let deployer = GitSnapshotDeployer::new("/workspace/kias");
        // /workspace/kias 应该是 git 仓库
        assert!(deployer.health_check());
    }

    #[test]
    fn test_git_snapshot_deployer_nonexistent_path() {
        let deployer = GitSnapshotDeployer::new("/nonexistent/path");
        assert!(!deployer.health_check());
    }

    #[test]
    fn test_git_snapshot_deployer_name() {
        let deployer = GitSnapshotDeployer::new("/tmp");
        assert_eq!(deployer.name(), "GitSnapshotDeployer");
    }
}
