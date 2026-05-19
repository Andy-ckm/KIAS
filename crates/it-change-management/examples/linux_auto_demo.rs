use kias_it_change_management::linux_auto::*;
use std::path::PathBuf;

fn main() {
    println!("=== AgentGuard Linux自动化维护演示 ===\n");

    let config = LinuxAutomationConfig {
        playbook_dir: PathBuf::from("/etc/ansible/playbooks"),
        compliance_tool: ComplianceTool::OpenScap,
        target_hosts: vec!["192.168.1.10".to_string(), "192.168.1.11".to_string()],
        ssh_key_path: Some(PathBuf::from("/root/.ssh/id_rsa")),
        log_dir: PathBuf::from("/var/log/compliance"),
    };

    let mut manager = LinuxAutomationManager::new(config);

    // 1. 合规扫描
    println!("【1】合规扫描（CIS Benchmark）");
    let task = AutomationTask::ComplianceScan {
        profile: "xccdf_org.ssgproject.content_profile_cis_level2".to_string(),
        hosts: vec!["192.168.1.10".to_string()],
    };
    let cmd = manager.generate_ansible_command(&task);
    println!("  命令: {}", cmd);
    let result = manager.execute_task(task);
    println!("  状态: {:?}", result.status);
    println!("  主机数: {}\n", result.host_results.len());

    // 2. 合规扫描报告
    println!("【2】合规扫描报告");
    let report = manager.execute_compliance_scan("192.168.1.10", "cis_level2");
    println!("  主机: {}", report.host);
    println!("  评分: {:.1}%", report.score);
    println!(
        "  总规则: {}, 通过: {}, 失败: {}",
        report.total_rules, report.passed, report.failed
    );
    println!("  发现:");
    for finding in &report.findings {
        println!(
            "    [{}] {} - {:?}",
            finding.rule_id, finding.title, finding.status
        );
    }
    println!();

    // 3. 补丁检查
    println!("【3】安全补丁检查");
    let patches = manager.check_patch_status("192.168.1.10");
    println!("  主机: {}", patches.host);
    println!("  安全补丁: {} 个", patches.security_patches_available);
    println!(
        "  非安全补丁: {} 个",
        patches.non_security_patches_available
    );
    for p in &patches.patches {
        println!("    {} {} - {:?}", p.name, p.version, p.severity);
    }
    println!();

    // 4. 磁盘检查
    println!("【4】磁盘使用检查");
    let disk = manager.check_disk_usage("192.168.1.10");
    println!("  主机: {}", disk.host);
    for fs in &disk.filesystems {
        println!(
            "    {} : {:.1}GB / {:.1}GB ({:.0}%)",
            fs.mount_point, fs.used_gb, fs.total_gb, fs.use_percent
        );
    }
    if !disk.warnings.is_empty() {
        println!("  ⚠️ 警告:");
        for w in &disk.warnings {
            println!("    {}", w);
        }
    }
    println!();

    // 5. 安全更新
    println!("【5】安全更新");
    let task = AutomationTask::SecurityUpdate {
        hosts: vec!["192.168.1.10".to_string(), "192.168.1.11".to_string()],
    };
    let cmd = manager.generate_ansible_command(&task);
    println!("  命令: {}", cmd);
    let result = manager.execute_task(task);
    println!("  状态: {:?}", result.status);
    println!();

    // 6. 日志收集
    println!("【6】日志收集");
    let task = AutomationTask::LogCollection {
        hosts: vec!["192.168.1.10".to_string()],
        log_paths: vec![
            "/var/log/audit/".to_string(),
            "/var/log/messages".to_string(),
        ],
    };
    let cmd = manager.generate_ansible_command(&task);
    println!("  命令: {}", cmd);
    println!();

    // 7. OpenSCAP命令
    println!("【7】OpenSCAP扫描命令");
    let cmd = manager.generate_openscap_command("192.168.1.10", "cis_level2");
    println!("  {}", cmd);
    println!();

    // 8. Lynis审计命令
    println!("【8】Lynis安全审计命令");
    let cmd = manager.generate_lynis_command("192.168.1.10");
    println!("  {}", cmd);
    println!();

    // 9. 任务统计
    println!("【9】任务统计");
    let stats = manager.get_statistics();
    println!("  总任务: {}", stats.total);
    println!("  成功: {}", stats.success);
    println!("  失败: {}", stats.failed);
    println!("  成功率: {:.1}%", stats.success_rate);

    println!("\n演示完成！");
}
