//! 合规扫描引擎

use crate::error::{AutomationError, Result};
use crate::models::*;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

/// 合规扫描器
pub struct ComplianceScanner {
    conn: Mutex<Connection>,
    tool: ComplianceTool,
}

impl ComplianceScanner {
    /// 创建新的扫描器
    pub fn new(config: &LinuxAutomationConfig) -> Result<Self> {
        let conn = Connection::open(&config.database_path)?;

        // 创建表
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS compliance_reports (
                id TEXT PRIMARY KEY,
                host TEXT NOT NULL,
                scan_time TEXT NOT NULL,
                profile TEXT NOT NULL,
                score REAL NOT NULL,
                passed INTEGER NOT NULL,
                failed INTEGER NOT NULL,
                not_applicable INTEGER NOT NULL,
                findings TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_compliance_host ON compliance_reports(host);
            CREATE INDEX IF NOT EXISTS idx_compliance_scan_time ON compliance_reports(scan_time);
            ",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            tool: config.compliance_tool.clone(),
        })
    }

    /// 执行合规扫描
    pub async fn scan(&self, hosts: &[String], profile: &str) -> Result<AutomationResult> {
        let mut host_results = Vec::new();

        for host in hosts {
            let result = self.scan_host(host, profile).await?;
            host_results.push(result);
        }

        let all_success = host_results.iter().all(|r| r.status == TaskStatus::Success);

        Ok(AutomationResult {
            task_id: uuid::Uuid::new_v4(),
            task_type: "ComplianceScan".to_string(),
            status: if all_success {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            },
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            host_results,
            summary: format!("合规扫描完成: {} 个主机", hosts.len()),
            audit_trail: vec![],
        })
    }

    /// 扫描单个主机
    async fn scan_host(&self, host: &str, profile: &str) -> Result<HostResult> {
        // 根据工具类型生成扫描命令
        let command = match &self.tool {
            ComplianceTool::OpenScap => {
                format!(
                    "oscap xccdf eval --profile {} --results /tmp/results.xml /usr/share/xml/scap/ssg/content/ssg-rhel8-ds.xml",
                    profile
                )
            }
            ComplianceTool::Lynis => "lynis audit system --quick".to_string(),
            ComplianceTool::CisCat => {
                format!("cis-cat --profile {} --format json", profile)
            }
            ComplianceTool::Custom(cmd) => cmd.clone(),
        };

        // 执行扫描命令
        let output = tokio::process::Command::new("ssh")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(host)
            .arg(&command)
            .output()
            .await
            .map_err(|e| AutomationError::ComplianceScan(format!("扫描执行失败: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // 解析结果（简化实现）
        let findings = self.parse_findings(&stdout, &stderr)?;

        // 保存报告
        let report = ComplianceReport {
            host: host.to_string(),
            scan_time: Utc::now(),
            profile: profile.to_string(),
            score: self.calculate_score(&findings),
            passed: findings.iter().filter(|f| f.status == FindingStatus::Pass).count(),
            failed: findings.iter().filter(|f| f.status == FindingStatus::Fail).count(),
            not_applicable: findings
                .iter()
                .filter(|f| f.status == FindingStatus::NotApplicable)
                .count(),
            findings,
        };

        self.save_report(&report)?;

        Ok(HostResult {
            host: host.to_string(),
            status: if output.status.success() {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            },
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: 0,
        })
    }

    /// 解析扫描发现
    fn parse_findings(&self, stdout: &str, stderr: &str) -> Result<Vec<ComplianceFinding>> {
        let mut findings = Vec::new();

        // 简化实现：解析 OpenSCAP 输出
        for line in stdout.lines() {
            if line.contains("pass") || line.contains("fail") {
                let status = if line.contains("pass") {
                    FindingStatus::Pass
                } else {
                    FindingStatus::Fail
                };

                findings.push(ComplianceFinding {
                    rule_id: format!("RULE-{}", findings.len() + 1),
                    title: line.to_string(),
                    severity: Severity::Medium,
                    status,
                    description: line.to_string(),
                    remediation: None,
                });
            }
        }

        Ok(findings)
    }

    /// 计算合规分数
    fn calculate_score(&self, findings: &[ComplianceFinding]) -> f64 {
        if findings.is_empty() {
            return 0.0;
        }

        let passed = findings.iter().filter(|f| f.status == FindingStatus::Pass).count();
        let total = findings
            .iter()
            .filter(|f| f.status != FindingStatus::NotApplicable)
            .count();

        if total == 0 {
            return 0.0;
        }

        (passed as f64 / total as f64) * 100.0
    }

    /// 保存报告
    fn save_report(&self, report: &ComplianceReport) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let findings_json = serde_json::to_string(&report.findings)?;

        conn.execute(
            "INSERT INTO compliance_reports (id, host, scan_time, profile, score, passed, failed, not_applicable, findings)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                report.host,
                report.scan_time.to_rfc3339(),
                report.profile,
                report.score,
                report.passed,
                report.failed,
                report.not_applicable,
                findings_json,
            ],
        )?;

        Ok(())
    }

    /// 获取报告
    pub fn get_report(&self, host: &str) -> Result<ComplianceReport> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare(
            "SELECT id, host, scan_time, profile, score, passed, failed, not_applicable, findings
             FROM compliance_reports WHERE host = ?1 ORDER BY scan_time DESC LIMIT 1",
        )?;

        let report = stmt.query_row(params![host], |row| {
            let findings_str: String = row.get(8)?;
            let findings: Vec<ComplianceFinding> =
                serde_json::from_str(&findings_str).unwrap_or_default();

            Ok(ComplianceReport {
                host: row.get(1)?,
                scan_time: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                profile: row.get(3)?,
                score: row.get(4)?,
                passed: row.get(5)?,
                failed: row.get(6)?,
                not_applicable: row.get(7)?,
                findings,
            })
        })?;

        Ok(report)
    }

    /// 获取平均分数
    pub fn get_average_score(&self) -> Result<f64> {
        let conn = self.conn.lock().unwrap();

        let avg: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(score), 0.0) FROM compliance_reports",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        Ok(avg)
    }

    /// 获取最后扫描时间
    pub fn get_last_scan_time(&self) -> Result<Option<chrono::DateTime<Utc>>> {
        let conn = self.conn.lock().unwrap();

        let time: Option<String> = conn
            .query_row(
                "SELECT MAX(scan_time) FROM compliance_reports",
                [],
                |row| row.get(0),
            )
            .unwrap_or(None);

        match time {
            Some(t) => Ok(Some(
                chrono::DateTime::parse_from_rfc3339(&t)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            )),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_scanner() -> (ComplianceScanner, TempDir) {
        let tmp = TempDir::new().unwrap();
        let config = LinuxAutomationConfig {
            database_path: tmp.path().join("test.db"),
            playbook_dir: tmp.path().join("playbooks"),
            ssh_key_path: None,
            log_dir: tmp.path().join("logs"),
            target_hosts: vec!["localhost".to_string()],
            compliance_tool: ComplianceTool::OpenScap,
        };
        let scanner = ComplianceScanner::new(&config).unwrap();
        (scanner, tmp)
    }

    #[test]
    fn test_create_scanner() {
        let (scanner, _tmp) = create_test_scanner();
        assert_eq!(scanner.tool, ComplianceTool::OpenScap);
    }

    #[test]
    fn test_calculate_score_empty() {
        let (scanner, _tmp) = create_test_scanner();
        let score = scanner.calculate_score(&[]);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_calculate_score_with_findings() {
        let (scanner, _tmp) = create_test_scanner();

        let findings = vec![
            ComplianceFinding {
                rule_id: "1".to_string(),
                title: "Test".to_string(),
                severity: Severity::Medium,
                status: FindingStatus::Pass,
                description: "Test".to_string(),
                remediation: None,
            },
            ComplianceFinding {
                rule_id: "2".to_string(),
                title: "Test".to_string(),
                severity: Severity::Medium,
                status: FindingStatus::Fail,
                description: "Test".to_string(),
                remediation: None,
            },
        ];

        let score = scanner.calculate_score(&findings);
        assert_eq!(score, 50.0);
    }

    #[test]
    fn test_get_average_score_empty() {
        let (scanner, _tmp) = create_test_scanner();
        let score = scanner.get_average_score().unwrap();
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_get_last_scan_time_empty() {
        let (scanner, _tmp) = create_test_scanner();
        let time = scanner.get_last_scan_time().unwrap();
        assert!(time.is_none());
    }
}
