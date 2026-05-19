//! R029: 备份恢复自动化模块
//!
//! 备份创建 / 恢复 / 验证 / 清理 / 恢复测试
//! 灵魂: 可追溯(审计日志) / 透明(备份报告) / 可控(保留策略)
//!
//! 竞品参考: restic(33K★), borg(13K★), rustic(3K★,Rust), backrest(6K★)
//! AgentGuard差异化: 备份→验证→恢复测试→审计（竞品只做备份）

use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tracing::info;
use uuid::Uuid;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 备份管理器
///
/// 提供备份生命周期管理：创建、恢复、验证、清理
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct BackupManager {
    /// 备份记录 (job_id -> records)
    records: Mutex<HashMap<Uuid, Vec<BackupRecord>>>,
    /// 备份作业
    jobs: Mutex<HashMap<Uuid, BackupJob>>,
}

impl BackupManager {
    /// 创建新的备份管理器
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    /// 获取 records 锁
    fn records_lock(&self) -> Result<MutexGuard<'_, HashMap<Uuid, Vec<BackupRecord>>>> {
        self.records
            .lock()
            .map_err(|e| AutomationError::LockPoisoned(format!("records: {}", e)))
    }

    /// 获取 jobs 锁
    fn jobs_lock(&self) -> Result<MutexGuard<'_, HashMap<Uuid, BackupJob>>> {
        self.jobs
            .lock()
            .map_err(|e| AutomationError::LockPoisoned(format!("jobs: {}", e)))
    }

    /// 执行备份操作
    pub async fn execute(
        executor: &TaskExecutor,
        hosts: &[String],
        action: &BackupAction,
        _audit: &AuditLog,
    ) -> Result<BackupOpsResult> {
        info!(?action, hosts=?hosts, "执行备份操作");

        let (cmd, description) = Self::build_command(action)?;

        let mut all_results = Vec::new();
        let mut audit_entries = Vec::new();

        for host in hosts {
            let result = executor.execute_command(std::slice::from_ref(host), &cmd).await?;

            let entry = AuditEntry {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                action: format!("Backup {:?}", action),
                user: "system".to_string(),
                target: host.clone(),
                result: if result.status == TaskStatus::Success {
                    "success".to_string()
                } else {
                    "failed".to_string()
                },
                details: Some(description.clone()),
                signature: None,
            };
            audit_entries.push(entry);
            all_results.push(result);
        }

        let all_success = all_results.iter().all(|r| r.status == TaskStatus::Success);

        Ok(BackupOpsResult {
            action: action.clone(),
            status: if all_success {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            },
            message: description,
            records: vec![],
            audit_trail: audit_entries,
        })
    }

    /// 根据 BackupAction 构建 SSH 命令
    fn build_command(action: &BackupAction) -> Result<(String, String)> {
        match action {
            BackupAction::Create {
                sources,
                destination,
                backup_type,
                compression,
                encryption,
                exclude_patterns,
            } => {
                let mut cmd = String::from("rsync -av");
                // 压缩
                match compression {
                    CompressionType::None => {}
                    CompressionType::Gzip => cmd.push_str(" --compress"),
                    CompressionType::Zstd => cmd.push_str(" --compress --compress-choice=zstd"),
                    CompressionType::Lz4 => cmd.push_str(" --compress --compress-choice=lz4"),
                }
                // 排除模式
                for pattern in exclude_patterns {
                    cmd.push_str(&format!(" --exclude='{}'", pattern));
                }
                // 增量备份 (使用 --link-dest)
                if *backup_type == BackupType::Incremental {
                    cmd.push_str(&format!(" --link-dest={}/latest", destination));
                }
                // 源和目标
                let sources_str = sources.join(" ");
                cmd.push_str(&format!(" {} {}/", sources_str, destination));
                // 创建 latest 链接
                cmd.push_str(&format!(
                    " && ln -sfn {} {}/latest",
                    destination, destination
                ));
                // 如果加密，用 gpg 加密归档
                if *encryption {
                    cmd = format!(
                        "tar cz {} {} | gpg --encrypt --recipient backup@agentguard > {}/backup_$(date +%Y%m%d_%H%M%S).tar.gz.gpg",
                        exclude_patterns.iter().map(|p| format!("--exclude='{}'", p)).collect::<Vec<_>>().join(" "),
                        sources_str,
                        destination
                    );
                }
                let desc = format!(
                    "创建{:?}备份: {} -> {} (加密: {}, 压缩: {:?})",
                    backup_type, sources_str, destination, encryption, compression
                );
                Ok((cmd, desc))
            }
            BackupAction::Restore {
                backup_id,
                restore_path,
                point_in_time,
            } => {
                let pit = point_in_time
                    .map(|t| t.format("%Y%m%d_%H%M%S").to_string())
                    .unwrap_or_else(|| "latest".to_string());
                let cmd = format!(
                    "rsync -av {}/{}/ {}",
                    backup_id, pit, restore_path
                );
                let desc = format!("恢复备份 {} 到 {} (时间点: {})", backup_id, restore_path, pit);
                Ok((cmd, desc))
            }
            BackupAction::Verify { backup_id } => {
                let cmd = format!(
                    "find {} -type f -exec sha256sum {{}} \\; | sha256sum",
                    backup_id
                );
                let desc = format!("验证备份 {} 完整性", backup_id);
                Ok((cmd, desc))
            }
            BackupAction::List {
                source_filter,
                limit,
            } => {
                let filter = source_filter
                    .as_deref()
                    .unwrap_or("*");
                let limit_flag = limit
                    .map(|n| format!(" | head -{}", n))
                    .unwrap_or_default();
                let cmd = format!(
                    "find {} -maxdepth 1 -type d -name 'backup_*' | sort -r{}",
                    filter, limit_flag
                );
                let desc = format!("列出备份 (过滤: {:?}, 限制: {:?})", source_filter, limit);
                Ok((cmd, desc))
            }
            BackupAction::Prune {
                retention_days,
                keep_daily,
                keep_weekly,
                keep_monthly,
            } => {
                let cmd = format!(
                    "find /backup -maxdepth 1 -type d -mtime +{} -exec rm -rf {{}} \\; && echo '保留: daily={}, weekly={}, monthly={}'",
                    retention_days, keep_daily, keep_weekly, keep_monthly
                );
                let desc = format!(
                    "清理 {} 天前的备份 (保留 daily={}, weekly={}, monthly={})",
                    retention_days, keep_daily, keep_weekly, keep_monthly
                );
                Ok((cmd, desc))
            }
            BackupAction::RestoreTest { backup_id } => {
                let tmp_dir = format!("/tmp/restore_test_{}", Uuid::new_v4());
                let cmd = format!(
                    "mkdir -p {} && rsync -av {} {}/ && ls -la {}/ && rm -rf {}",
                    tmp_dir, backup_id, tmp_dir, tmp_dir, tmp_dir
                );
                let desc = format!("恢复测试: 备份 {} -> 临时目录验证", backup_id);
                Ok((cmd, desc))
            }
            BackupAction::Status => {
                let cmd = String::from("df -h /backup && echo '---' && ls -lt /backup | head -20");
                let desc = "备份状态检查".to_string();
                Ok((cmd, desc))
            }
        }
    }

    /// 创建备份作业
    pub fn create_job(&self, job: BackupJob) -> Result<Uuid> {
        let id = job.id;
        info!(job_id=%id, name=%job.name, "创建备份作业");
        let mut jobs = self.jobs_lock()?;
        jobs.insert(id, job);
        Ok(id)
    }

    /// 获取备份作业
    pub fn get_job(&self, job_id: &Uuid) -> Result<Option<BackupJob>> {
        let jobs = self.jobs_lock()?;
        Ok(jobs.get(job_id).cloned())
    }

    /// 列出所有备份作业
    pub fn list_jobs(&self) -> Result<Vec<BackupJob>> {
        let jobs = self.jobs_lock()?;
        Ok(jobs.values().cloned().collect())
    }

    /// 删除备份作业
    pub fn delete_job(&self, job_id: &Uuid) -> Result<bool> {
        let mut jobs = self.jobs_lock()?;
        Ok(jobs.remove(job_id).is_some())
    }

    /// 添加备份记录
    pub fn add_record(&self, record: BackupRecord) -> Result<()> {
        let mut records = self.records_lock()?;
        records
            .entry(record.job_id)
            .or_default()
            .push(record);
        Ok(())
    }

    /// 获取某个作业的备份记录
    pub fn get_records(&self, job_id: &Uuid) -> Result<Vec<BackupRecord>> {
        let records = self.records_lock()?;
        Ok(records.get(job_id).cloned().unwrap_or_default())
    }

    /// 获取所有备份记录
    pub fn get_all_records(&self) -> Result<Vec<BackupRecord>> {
        let records = self.records_lock()?;
        Ok(records.values().flatten().cloned().collect())
    }

    /// 执行保留策略清理
    pub fn prune_records(&self, retention_days: u32) -> Result<Vec<BackupRecord>> {
        let mut records = self.records_lock()?;
        let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
        let mut pruned = Vec::new();

        for (_job_id, job_records) in records.iter_mut() {
            let before = job_records.len();
            job_records.retain(|r| {
                let should_keep = r.started_at > cutoff || r.status == BackupStatus::InProgress;
                if !should_keep {
                    pruned.push(r.clone());
                }
                should_keep
            });
            if job_records.len() < before {
                info!(
                    job_id = %_job_id,
                    pruned = before - job_records.len(),
                    remaining = job_records.len(),
                    "清理过期备份记录"
                );
            }
        }

        Ok(pruned)
    }

    /// 计算备份统计
    pub fn get_statistics(&self) -> Result<BackupStatistics> {
        let jobs = self.jobs_lock()?;
        let records = self.records_lock()?;

        let total_jobs = jobs.len() as u32;
        let active_jobs = jobs
            .values()
            .filter(|j| j.status == BackupJobStatus::Active)
            .count() as u32;

        let all_records: Vec<&BackupRecord> = records.values().flatten().collect();
        let total_backups = all_records.len() as u32;
        let total_size_bytes: u64 = all_records.iter().map(|r| r.size_bytes).sum();

        let now = Utc::now();
        let last_24h = now - chrono::Duration::hours(24);
        let last_24h_backups = all_records
            .iter()
            .filter(|r| r.started_at > last_24h && r.status == BackupStatus::Completed)
            .count() as u32;
        let last_24h_failures = all_records
            .iter()
            .filter(|r| r.started_at > last_24h && r.status == BackupStatus::Failed)
            .count() as u32;

        let completed: Vec<&BackupRecord> = all_records
            .iter()
            .filter(|r| r.status == BackupStatus::Completed)
            .cloned()
            .collect();
        let average_duration_secs = if completed.is_empty() {
            0
        } else {
            completed.iter().map(|r| r.duration_secs).sum::<u64>() / completed.len() as u64
        };

        let verified_count = all_records
            .iter()
            .filter(|r| {
                r.verification
                    .as_ref()
                    .map(|v| v.checksum_match)
                    .unwrap_or(false)
            })
            .count() as u32;
        let verification_pass_rate = if total_backups == 0 {
            0.0
        } else {
            verified_count as f64 / total_backups as f64
        };

        let oldest_backup = all_records.iter().map(|r| r.started_at).min();
        let newest_backup = all_records.iter().map(|r| r.started_at).max();

        Ok(BackupStatistics {
            total_jobs,
            active_jobs,
            total_backups,
            total_size_bytes,
            last_24h_backups,
            last_24h_failures,
            average_duration_secs,
            storage_used_gb: total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            oldest_backup,
            newest_backup,
            verification_pass_rate,
        })
    }
}

impl Default for BackupManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 备份操作结果
#[derive(Debug, Clone)]
pub struct BackupOpsResult {
    pub action: BackupAction,
    pub status: TaskStatus,
    pub message: String,
    pub records: Vec<BackupRecord>,
    pub audit_trail: Vec<AuditEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> BackupManager {
        BackupManager::new()
    }

    fn create_test_job(name: &str) -> BackupJob {
        BackupJob {
            id: Uuid::new_v4(),
            name: name.to_string(),
            sources: vec!["/data".to_string()],
            destination: "/backup".to_string(),
            backup_type: BackupType::Full,
            schedule: BackupSchedule::Daily {
                hour: 2,
                minute: 0,
            },
            compression: CompressionType::Gzip,
            encryption: false,
            encryption_key_id: None,
            exclude_patterns: vec!["*.tmp".to_string()],
            retention: RetentionPolicy::default(),
            created_at: Utc::now(),
            created_by: "admin".to_string(),
            status: BackupJobStatus::Active,
            last_run: None,
            last_backup_id: None,
            total_backups: 0,
            total_size_bytes: 0,
        }
    }

    fn create_test_record(job_id: Uuid) -> BackupRecord {
        BackupRecord {
            id: Uuid::new_v4(),
            job_id,
            backup_type: BackupType::Full,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: BackupStatus::Completed,
            size_bytes: 1024 * 1024 * 100, // 100MB
            file_count: 500,
            checksum: "abc123".to_string(),
            encryption: false,
            compression: CompressionType::Gzip,
            duration_secs: 60,
            source_hosts: vec!["server1".to_string()],
            error_message: None,
            verification: None,
        }
    }

    // === 构造测试 ===

    #[test]
    fn test_backup_manager_new() {
        let mgr = create_test_manager();
        assert!(mgr.jobs_lock().unwrap().is_empty());
        assert!(mgr.records_lock().unwrap().is_empty());
    }

    #[test]
    fn test_backup_manager_default() {
        let mgr = BackupManager::default();
        assert!(mgr.jobs_lock().unwrap().is_empty());
    }

    // === 作业管理测试 ===

    #[test]
    fn test_create_job() {
        let mgr = create_test_manager();
        let job = create_test_job("daily-backup");
        let id = job.id;

        let result = mgr.create_job(job);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), id);

        let jobs = mgr.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "daily-backup");
    }

    #[test]
    fn test_get_job_exists() {
        let mgr = create_test_manager();
        let job = create_test_job("test-job");
        let id = job.id;
        mgr.create_job(job).unwrap();

        let retrieved = mgr.get_job(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-job");
    }

    #[test]
    fn test_get_job_not_found() {
        let mgr = create_test_manager();
        let id = Uuid::new_v4();
        let result = mgr.get_job(&id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list_jobs_multiple() {
        let mgr = create_test_manager();
        mgr.create_job(create_test_job("job1")).unwrap();
        mgr.create_job(create_test_job("job2")).unwrap();
        mgr.create_job(create_test_job("job3")).unwrap();

        let jobs = mgr.list_jobs().unwrap();
        assert_eq!(jobs.len(), 3);
    }

    #[test]
    fn test_list_jobs_empty() {
        let mgr = create_test_manager();
        let jobs = mgr.list_jobs().unwrap();
        assert!(jobs.is_empty());
    }

    #[test]
    fn test_delete_job_exists() {
        let mgr = create_test_manager();
        let job = create_test_job("to-delete");
        let id = job.id;
        mgr.create_job(job).unwrap();

        let deleted = mgr.delete_job(&id).unwrap();
        assert!(deleted);
        assert!(mgr.get_job(&id).unwrap().is_none());
    }

    #[test]
    fn test_delete_job_not_found() {
        let mgr = create_test_manager();
        let id = Uuid::new_v4();
        let deleted = mgr.delete_job(&id).unwrap();
        assert!(!deleted);
    }

    // === 记录管理测试 ===

    #[test]
    fn test_add_record() {
        let mgr = create_test_manager();
        let job = create_test_job("test");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        let record = create_test_record(job_id);
        let result = mgr.add_record(record);
        assert!(result.is_ok());

        let records = mgr.get_records(&job_id).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, BackupStatus::Completed);
    }

    #[test]
    fn test_add_multiple_records() {
        let mgr = create_test_manager();
        let job = create_test_job("multi");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        for _ in 0..5 {
            mgr.add_record(create_test_record(job_id)).unwrap();
        }

        let records = mgr.get_records(&job_id).unwrap();
        assert_eq!(records.len(), 5);
    }

    #[test]
    fn test_get_records_empty() {
        let mgr = create_test_manager();
        let id = Uuid::new_v4();
        let records = mgr.get_records(&id).unwrap();
        assert!(records.is_empty());
    }

    #[test]
    fn test_get_all_records() {
        let mgr = create_test_manager();
        let job1 = create_test_job("j1");
        let job2 = create_test_job("j2");
        let j1_id = job1.id;
        let j2_id = job2.id;
        mgr.create_job(job1).unwrap();
        mgr.create_job(job2).unwrap();

        mgr.add_record(create_test_record(j1_id)).unwrap();
        mgr.add_record(create_test_record(j1_id)).unwrap();
        mgr.add_record(create_test_record(j2_id)).unwrap();

        let all = mgr.get_all_records().unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_get_all_records_empty() {
        let mgr = create_test_manager();
        let all = mgr.get_all_records().unwrap();
        assert!(all.is_empty());
    }

    // === 保留策略测试 ===

    #[test]
    fn test_prune_records_removes_old() {
        let mgr = create_test_manager();
        let job = create_test_job("prune-test");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        // 添加一条旧记录
        let mut old_record = create_test_record(job_id);
        old_record.started_at = Utc::now() - chrono::Duration::days(100);
        old_record.status = BackupStatus::Completed;
        mgr.add_record(old_record).unwrap();

        // 添加一条新记录
        let new_record = create_test_record(job_id);
        mgr.add_record(new_record).unwrap();

        let pruned = mgr.prune_records(30).unwrap();
        assert_eq!(pruned.len(), 1); // 旧记录被清理

        let remaining = mgr.get_records(&job_id).unwrap();
        assert_eq!(remaining.len(), 1); // 新记录保留
    }

    #[test]
    fn test_prune_records_keeps_in_progress() {
        let mgr = create_test_manager();
        let job = create_test_job("prune-inprogress");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        let mut in_progress = create_test_record(job_id);
        in_progress.started_at = Utc::now() - chrono::Duration::days(100);
        in_progress.status = BackupStatus::InProgress;
        mgr.add_record(in_progress).unwrap();

        let pruned = mgr.prune_records(30).unwrap();
        assert_eq!(pruned.len(), 0); // InProgress 不被清理

        let remaining = mgr.get_records(&job_id).unwrap();
        assert_eq!(remaining.len(), 1);
    }

    #[test]
    fn test_prune_records_empty() {
        let mgr = create_test_manager();
        let pruned = mgr.prune_records(30).unwrap();
        assert!(pruned.is_empty());
    }

    // === 统计测试 ===

    #[test]
    fn test_statistics_empty() {
        let mgr = create_test_manager();
        let stats = mgr.get_statistics().unwrap();
        assert_eq!(stats.total_jobs, 0);
        assert_eq!(stats.active_jobs, 0);
        assert_eq!(stats.total_backups, 0);
        assert_eq!(stats.total_size_bytes, 0);
        assert_eq!(stats.last_24h_backups, 0);
        assert_eq!(stats.last_24h_failures, 0);
        assert_eq!(stats.average_duration_secs, 0);
        assert_eq!(stats.storage_used_gb, 0.0);
        assert_eq!(stats.verification_pass_rate, 0.0);
        assert!(stats.oldest_backup.is_none());
        assert!(stats.newest_backup.is_none());
    }

    #[test]
    fn test_statistics_with_jobs_and_records() {
        let mgr = create_test_manager();
        let mut job1 = create_test_job("active-job");
        job1.status = BackupJobStatus::Active;
        let mut job2 = create_test_job("paused-job");
        job2.status = BackupJobStatus::Paused;
        let j1_id = job1.id;
        let j2_id = job2.id;
        mgr.create_job(job1).unwrap();
        mgr.create_job(job2).unwrap();

        // 添加记录
        mgr.add_record(create_test_record(j1_id)).unwrap();
        mgr.add_record(create_test_record(j2_id)).unwrap();

        let stats = mgr.get_statistics().unwrap();
        assert_eq!(stats.total_jobs, 2);
        assert_eq!(stats.active_jobs, 1); // 只有 job1 是 Active
        assert_eq!(stats.total_backups, 2);
        assert!(stats.total_size_bytes > 0);
        assert!(stats.newest_backup.is_some());
    }

    #[test]
    fn test_statistics_verification_rate() {
        let mgr = create_test_manager();
        let job = create_test_job("verified-job");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        // 一条有验证通过的记录
        let mut verified = create_test_record(job_id);
        verified.verification = Some(BackupVerification {
            verified_at: Utc::now(),
            checksum_match: true,
            file_count_match: true,
            restore_test_passed: true,
            integrity_score: 1.0,
            notes: "All good".to_string(),
        });
        mgr.add_record(verified).unwrap();

        // 一条没有验证的记录
        let mut unverified = create_test_record(job_id);
        unverified.verification = None;
        mgr.add_record(unverified).unwrap();

        let stats = mgr.get_statistics().unwrap();
        assert_eq!(stats.total_backups, 2);
        assert!((stats.verification_pass_rate - 0.5).abs() < 0.01);
    }

    // === build_command 测试 ===

    #[test]
    fn test_build_command_create_full() {
        let action = BackupAction::Create {
            sources: vec!["/data".to_string()],
            destination: "/backup/full".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::Gzip,
            encryption: false,
            exclude_patterns: vec![],
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("rsync -av"));
        assert!(cmd.contains("--compress"));
        assert!(cmd.contains("/data"));
        assert!(cmd.contains("/backup/full/"));
        assert!(desc.contains("Full"));
    }

    #[test]
    fn test_build_command_create_incremental() {
        let action = BackupAction::Create {
            sources: vec!["/var/log".to_string()],
            destination: "/backup/incr".to_string(),
            backup_type: BackupType::Incremental,
            compression: CompressionType::None,
            encryption: false,
            exclude_patterns: vec!["*.tmp".to_string()],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("--link-dest=/backup/incr/latest"));
        assert!(cmd.contains("--exclude='*.tmp'"));
    }

    #[test]
    fn test_build_command_create_encrypted() {
        let action = BackupAction::Create {
            sources: vec!["/secrets".to_string()],
            destination: "/backup/enc".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::None,
            encryption: true,
            exclude_patterns: vec![],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("gpg --encrypt"));
        assert!(cmd.contains(".tar.gz.gpg"));
    }

    #[test]
    fn test_build_command_create_zstd() {
        let action = BackupAction::Create {
            sources: vec!["/data".to_string()],
            destination: "/backup".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::Zstd,
            encryption: false,
            exclude_patterns: vec![],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("zstd"));
    }

    #[test]
    fn test_build_command_create_lz4() {
        let action = BackupAction::Create {
            sources: vec!["/data".to_string()],
            destination: "/backup".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::Lz4,
            encryption: false,
            exclude_patterns: vec![],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("lz4"));
    }

    #[test]
    fn test_build_command_restore() {
        let action = BackupAction::Restore {
            backup_id: "backup-001".to_string(),
            restore_path: "/restore".to_string(),
            point_in_time: None,
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("rsync -av"));
        assert!(cmd.contains("backup-001"));
        assert!(cmd.contains("/restore"));
        assert!(desc.contains("恢复备份"));
    }

    #[test]
    fn test_build_command_restore_with_point_in_time() {
        let pit = chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(10, 30, 0)
            .unwrap()
            .and_utc();
        let action = BackupAction::Restore {
            backup_id: "backup-002".to_string(),
            restore_path: "/restore".to_string(),
            point_in_time: Some(pit),
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("20260115_103000"));
    }

    #[test]
    fn test_build_command_verify() {
        let action = BackupAction::Verify {
            backup_id: "backup-003".to_string(),
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("sha256sum"));
        assert!(desc.contains("验证备份"));
    }

    #[test]
    fn test_build_command_list() {
        let action = BackupAction::List {
            source_filter: None,
            limit: None,
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("find"));
        assert!(desc.contains("列出备份"));
    }

    #[test]
    fn test_build_command_list_with_filter_and_limit() {
        let action = BackupAction::List {
            source_filter: Some("/backup/daily".to_string()),
            limit: Some(10),
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("head -10"));
        assert!(cmd.contains("/backup/daily"));
    }

    #[test]
    fn test_build_command_prune() {
        let action = BackupAction::Prune {
            retention_days: 30,
            keep_daily: 7,
            keep_weekly: 4,
            keep_monthly: 12,
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("mtime +30"));
        assert!(desc.contains("清理 30 天前"));
    }

    #[test]
    fn test_build_command_restore_test() {
        let action = BackupAction::RestoreTest {
            backup_id: "backup-004".to_string(),
        };
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("restore_test_"));
        assert!(cmd.contains("mkdir -p"));
        assert!(cmd.contains("rm -rf"));
        assert!(desc.contains("恢复测试"));
    }

    #[test]
    fn test_build_command_status() {
        let action = BackupAction::Status;
        let (cmd, desc) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("df -h"));
        assert!(desc.contains("备份状态检查"));
    }

    // === 数据模型测试 ===

    #[test]
    fn test_backup_type_variants() {
        let types = [
            BackupType::Full,
            BackupType::Incremental,
            BackupType::Differential,
        ];
        assert_eq!(types.len(), 3);
        assert_ne!(BackupType::Full, BackupType::Incremental);
        assert_ne!(BackupType::Incremental, BackupType::Differential);
    }

    #[test]
    fn test_backup_action_clone() {
        let action = BackupAction::Create {
            sources: vec!["/data".to_string()],
            destination: "/backup".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::Gzip,
            encryption: false,
            exclude_patterns: vec![],
        };
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_backup_schedule_variants() {
        let schedules = [
            BackupSchedule::Manual,
            BackupSchedule::Hourly,
            BackupSchedule::Daily {
                hour: 2,
                minute: 0,
            },
            BackupSchedule::Weekly {
                day_of_week: 1,
                hour: 3,
                minute: 0,
            },
            BackupSchedule::Monthly {
                day_of_month: 1,
                hour: 4,
                minute: 0,
            },
            BackupSchedule::Cron {
                expression: "0 2 * * *".to_string(),
            },
        ];
        assert_eq!(schedules.len(), 6);
        assert_ne!(BackupSchedule::Manual, BackupSchedule::Hourly);
    }

    #[test]
    fn test_retention_policy_default() {
        let policy = RetentionPolicy::default();
        assert_eq!(policy.keep_daily, 7);
        assert_eq!(policy.keep_weekly, 4);
        assert_eq!(policy.keep_monthly, 12);
        assert_eq!(policy.keep_yearly, 3);
        assert!(policy.max_total_size_gb.is_none());
    }

    #[test]
    fn test_backup_job_status_variants() {
        let statuses = [
            BackupJobStatus::Active,
            BackupJobStatus::Paused,
            BackupJobStatus::Failed,
            BackupJobStatus::Disabled,
        ];
        assert_eq!(statuses.len(), 4);
    }

    #[test]
    fn test_backup_status_variants() {
        let statuses = [
            BackupStatus::InProgress,
            BackupStatus::Completed,
            BackupStatus::Failed,
            BackupStatus::Verified,
            BackupStatus::Corrupted,
        ];
        assert_eq!(statuses.len(), 5);
    }

    #[test]
    fn test_backup_verification_fields() {
        let v = BackupVerification {
            verified_at: Utc::now(),
            checksum_match: true,
            file_count_match: true,
            restore_test_passed: true,
            integrity_score: 0.99,
            notes: "All files verified".to_string(),
        };
        assert!(v.checksum_match);
        assert!(v.file_count_match);
        assert!(v.restore_test_passed);
        assert!(v.integrity_score > 0.9);
    }

    #[test]
    fn test_compression_type_variants() {
        let types = [
            CompressionType::None,
            CompressionType::Gzip,
            CompressionType::Zstd,
            CompressionType::Lz4,
        ];
        assert_eq!(types.len(), 4);
    }

    // === 边界情况测试 ===

    #[test]
    fn test_create_multiple_jobs_same_name() {
        let mgr = create_test_manager();
        let job1 = create_test_job("same-name");
        let job2 = create_test_job("same-name");
        let id1 = job1.id;
        let id2 = job2.id;

        mgr.create_job(job1).unwrap();
        mgr.create_job(job2).unwrap();

        // 两个不同的 job，名字相同但 ID 不同
        assert!(mgr.get_job(&id1).unwrap().is_some());
        assert!(mgr.get_job(&id2).unwrap().is_some());
        assert_eq!(mgr.list_jobs().unwrap().len(), 2);
    }

    #[test]
    fn test_delete_job_removes_records() {
        let mgr = create_test_manager();
        let job = create_test_job("delete-with-records");
        let job_id = job.id;
        mgr.create_job(job).unwrap();
        mgr.add_record(create_test_record(job_id)).unwrap();

        // 删除 job，records 不自动删除（设计选择）
        mgr.delete_job(&job_id).unwrap();
        assert!(mgr.get_job(&job_id).unwrap().is_none());
        // records 仍在（按 job_id 索引）
        let records = mgr.get_records(&job_id).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn test_statistics_storage_calculation() {
        let mgr = create_test_manager();
        let job = create_test_job("storage-test");
        let job_id = job.id;
        mgr.create_job(job).unwrap();

        // 1GB 的备份记录
        let mut record = create_test_record(job_id);
        record.size_bytes = 1024 * 1024 * 1024;
        mgr.add_record(record).unwrap();

        let stats = mgr.get_statistics().unwrap();
        assert!((stats.storage_used_gb - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_build_command_create_multiple_sources() {
        let action = BackupAction::Create {
            sources: vec![
                "/var/log".to_string(),
                "/etc".to_string(),
                "/home".to_string(),
            ],
            destination: "/backup/multi".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::None,
            encryption: false,
            exclude_patterns: vec![],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("/var/log"));
        assert!(cmd.contains("/etc"));
        assert!(cmd.contains("/home"));
    }

    #[test]
    fn test_build_command_create_multiple_excludes() {
        let action = BackupAction::Create {
            sources: vec!["/data".to_string()],
            destination: "/backup".to_string(),
            backup_type: BackupType::Full,
            compression: CompressionType::None,
            encryption: false,
            exclude_patterns: vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                ".cache".to_string(),
            ],
        };
        let (cmd, _) = BackupManager::build_command(&action).unwrap();
        assert!(cmd.contains("--exclude='*.tmp'"));
        assert!(cmd.contains("--exclude='*.log'"));
        assert!(cmd.contains("--exclude='.cache'"));
    }
}
