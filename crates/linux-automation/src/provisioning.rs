//! R024: 新服务器初始化模块
//!
//! 软件安装 + 用户配置 + 安全加固 + 服务管理
//! 灵魂: 可追溯(每步审计) / 透明(进度推送) / 可控(模板可定制)

use chrono::Utc;
use tracing::info;

use crate::audit::AuditLog;
use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::models::*;

/// 初始化引擎
pub struct Provisioner;

impl Provisioner {
    /// 执行服务器初始化
    pub async fn provision(
        executor: &TaskExecutor,
        host: &str,
        template: &ProvisionTemplate,
        audit: &AuditLog,
    ) -> Result<ProvisionReport> {
        let started_at = Utc::now();
        let mut step_results = Vec::new();

        for step in &template.steps {
            let step_start = std::time::Instant::now();
            let (status, stdout, stderr) =
                Self::execute_step(executor, host, &step.step_type).await?;
            let duration_ms = step_start.elapsed().as_millis() as u64;

            step_results.push(ProvisionStepResult {
                step_name: step.name.clone(),
                status: status.clone(),
                stdout,
                stderr,
                duration_ms,
            });

            // 可追溯: 每步记录审计
            audit.log_action(
                "system",
                "Provision",
                host,
                &format!("步骤[{}]: {:?}", step.name, status),
            )?;

            info!(host = %host, step = %step.name, status = ?status, "初始化步骤完成");

            // 必需步骤失败则中止
            if step.required && status == TaskStatus::Failed {
                return Ok(ProvisionReport {
                    host: host.to_string(),
                    template_name: template.name.clone(),
                    started_at,
                    completed_at: Some(Utc::now()),
                    step_results,
                    overall_status: TaskStatus::Failed,
                });
            }
        }

        let overall_status = if step_results.iter().all(|r| r.status == TaskStatus::Success) {
            TaskStatus::Success
        } else {
            TaskStatus::PartialSuccess
        };

        Ok(ProvisionReport {
            host: host.to_string(),
            template_name: template.name.clone(),
            started_at,
            completed_at: Some(Utc::now()),
            step_results,
            overall_status,
        })
    }

    /// 执行单个初始化步骤
    async fn execute_step(
        executor: &TaskExecutor,
        host: &str,
        step_type: &ProvisionStepType,
    ) -> Result<(TaskStatus, String, String)> {
        let cmd = match step_type {
            ProvisionStepType::SystemUpdate => {
                // 检测包管理器
                "if command -v apt-get >/dev/null; then apt-get update -qq && apt-get upgrade -y -qq; \
                 elif command -v yum >/dev/null; then yum update -y -q; \
                 elif command -v dnf >/dev/null; then dnf update -y -q; \
                 else echo 'Unknown package manager' && exit 1; fi"
                    .to_string()
            }
            ProvisionStepType::InstallPackages { packages } => {
                let pkg_list = packages.join(" ");
                format!(
                    "if command -v apt-get >/dev/null; then apt-get install -y -qq {pkgs}; \
                     elif command -v yum >/dev/null; then yum install -y -q {pkgs}; \
                     elif command -v dnf >/dev/null; then dnf install -y -q {pkgs}; \
                     else echo 'Unknown package manager' && exit 1; fi",
                    pkgs = pkg_list
                )
            }
            ProvisionStepType::CreateUser { username, ssh_key } => {
                let mut cmds = vec![format!(
                    "id -u {user} >/dev/null 2>&1 || useradd -m -s /bin/bash {user}",
                    user = username
                )];
                if let Some(key) = ssh_key {
                    cmds.push(format!(
                        "mkdir -p /home/{user}/.ssh && echo '{key}' >> /home/{user}/.ssh/authorized_keys && \
                         chmod 700 /home/{user}/.ssh && chmod 600 /home/{user}/.ssh/authorized_keys && \
                         chown -R {user}:{user} /home/{user}/.ssh",
                        user = username,
                        key = key
                    ));
                }
                cmds.join(" && ")
            }
            ProvisionStepType::SudoConfig { username, rules } => {
                let rules_content = rules.join("\n");
                format!(
                    "echo '{username} ALL=(ALL) NOPASSWD: {rules}' > /etc/sudoers.d/{username} && \
                     chmod 440 /etc/sudoers.d/{username}",
                    username = username,
                    rules = rules_content
                )
            }
            ProvisionStepType::SshHardening { config } => {
                let port = config.port;
                let root_login = if config.permit_root_login {
                    "yes"
                } else {
                    "no"
                };
                let passwd_auth = if config.password_auth { "yes" } else { "no" };
                let max_tries = config.max_auth_tries;
                format!(
                    "sed -i 's/^#*Port .*/Port {port}/' /etc/ssh/sshd_config && \
                     sed -i 's/^#*PermitRootLogin .*/PermitRootLogin {root}/' /etc/ssh/sshd_config && \
                     sed -i 's/^#*PasswordAuthentication .*/PasswordAuthentication {passwd}/' /etc/ssh/sshd_config && \
                     sed -i 's/^#*MaxAuthTries .*/MaxAuthTries {tries}/' /etc/ssh/sshd_config && \
                     systemctl restart sshd 2>/dev/null || systemctl restart ssh 2>/dev/null || true",
                    port = port,
                    root = root_login,
                    passwd = passwd_auth,
                    tries = max_tries
                )
            }
            ProvisionStepType::Firewall { rules } => {
                let mut cmds = vec!["command -v ufw >/dev/null && ufw --force enable".to_string()];
                for rule in rules {
                    let source = rule.source.as_deref().unwrap_or("any");
                    cmds.push(format!(
                        "ufw allow {}/{} from {} >/dev/null 2>&1 || true",
                        rule.port, rule.protocol, source
                    ));
                }
                cmds.join(" && ")
            }
            ProvisionStepType::Timezone { tz } => {
                format!("timedatectl set-timezone {} 2>/dev/null || ln -sf /usr/share/zoneinfo/{} /etc/localtime", tz, tz)
            }
            ProvisionStepType::NtpServer { server } => {
                format!(
                    "if command -v timedatectl >/dev/null; then \
                       timedatectl set-ntp true; \
                     fi; \
                     if [ -f /etc/chrony/chrony.conf ]; then \
                       echo 'server {} iburst' >> /etc/chrony/chrony.conf && systemctl restart chronyd; \
                     elif [ -f /etc/ntp.conf ]; then \
                       echo 'server {} iburst' >> /etc/ntp.conf && systemctl restart ntpd; \
                     fi",
                    server, server
                )
            }
            ProvisionStepType::KernelParams { params } => {
                let mut cmds = Vec::new();
                for (key, value) in params {
                    cmds.push(format!("sysctl -w {}={} >/dev/null 2>&1", key, value));
                    cmds.push(format!(
                        "grep -q '^{}' /etc/sysctl.conf || echo '{}={}' >> /etc/sysctl.conf",
                        key, key, value
                    ));
                }
                cmds.join(" && ")
            }
            ProvisionStepType::ServiceManagement { enable, disable } => {
                let mut cmds = Vec::new();
                for svc in enable {
                    cmds.push(format!("systemctl enable {} 2>/dev/null || true", svc));
                }
                for svc in disable {
                    cmds.push(format!("systemctl disable {} 2>/dev/null || true", svc));
                }
                cmds.join(" && ")
            }
            ProvisionStepType::CustomScript { script } => script.clone(),
        };

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        if let Some(hr) = result.host_results.first() {
            let status = if hr.exit_code == 0 {
                TaskStatus::Success
            } else {
                TaskStatus::Failed
            };
            Ok((status, hr.stdout.clone(), hr.stderr.clone()))
        } else {
            Ok((TaskStatus::Failed, String::new(), "No result".to_string()))
        }
    }

    /// 创建默认初始化模板
    pub fn default_template() -> ProvisionTemplate {
        ProvisionTemplate {
            name: "default".to_string(),
            steps: vec![
                ProvisionStep {
                    name: "系统更新".to_string(),
                    step_type: ProvisionStepType::SystemUpdate,
                    required: true,
                },
                ProvisionStep {
                    name: "安装基础软件".to_string(),
                    step_type: ProvisionStepType::InstallPackages {
                        packages: vec![
                            "vim".to_string(),
                            "curl".to_string(),
                            "wget".to_string(),
                            "git".to_string(),
                            "htop".to_string(),
                            "tmux".to_string(),
                            "unzip".to_string(),
                        ],
                    },
                    required: false,
                },
                ProvisionStep {
                    name: "设置时区".to_string(),
                    step_type: ProvisionStepType::Timezone {
                        tz: "Asia/Shanghai".to_string(),
                    },
                    required: false,
                },
                ProvisionStep {
                    name: "NTP同步".to_string(),
                    step_type: ProvisionStepType::NtpServer {
                        server: "ntp.aliyun.com".to_string(),
                    },
                    required: false,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_template() {
        let tmpl = Provisioner::default_template();
        assert_eq!(tmpl.name, "default");
        assert_eq!(tmpl.steps.len(), 4);
        assert!(tmpl.steps[0].required); // 系统更新是必需的
        assert!(!tmpl.steps[1].required); // 基础软件不是必需的
    }

    #[test]
    fn test_provision_step_type_variants() {
        let variants = vec![
            ProvisionStepType::SystemUpdate,
            ProvisionStepType::InstallPackages {
                packages: vec!["vim".to_string()],
            },
            ProvisionStepType::CreateUser {
                username: "test".to_string(),
                ssh_key: None,
            },
            ProvisionStepType::SudoConfig {
                username: "test".to_string(),
                rules: vec!["ALL".to_string()],
            },
            ProvisionStepType::SshHardening {
                config: SshConfig {
                    permit_root_login: false,
                    password_auth: false,
                    max_auth_tries: 3,
                    port: 22,
                },
            },
            ProvisionStepType::Firewall {
                rules: vec![FirewallRule {
                    port: 22,
                    protocol: "tcp".to_string(),
                    action: "allow".to_string(),
                    source: None,
                }],
            },
            ProvisionStepType::Timezone {
                tz: "UTC".to_string(),
            },
            ProvisionStepType::NtpServer {
                server: "pool.ntp.org".to_string(),
            },
            ProvisionStepType::KernelParams {
                params: vec![("vm.swappiness".to_string(), "10".to_string())],
            },
            ProvisionStepType::ServiceManagement {
                enable: vec!["sshd".to_string()],
                disable: vec!["cups".to_string()],
            },
            ProvisionStepType::CustomScript {
                script: "echo hello".to_string(),
            },
        ];
        assert_eq!(variants.len(), 11);
    }

    #[test]
    fn test_ssh_config_creation() {
        let cfg = SshConfig {
            permit_root_login: false,
            password_auth: false,
            max_auth_tries: 3,
            port: 2222,
        };
        assert!(!cfg.permit_root_login);
        assert_eq!(cfg.port, 2222);
    }

    #[test]
    fn test_firewall_rule_creation() {
        let rule = FirewallRule {
            port: 443,
            protocol: "tcp".to_string(),
            action: "allow".to_string(),
            source: Some("10.0.0.0/8".to_string()),
        };
        assert_eq!(rule.port, 443);
        assert!(rule.source.is_some());
    }

    #[test]
    fn test_provision_report_creation() {
        let report = ProvisionReport {
            host: "server1".to_string(),
            template_name: "default".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            step_results: vec![],
            overall_status: TaskStatus::Running,
        };
        assert_eq!(report.host, "server1");
        assert!(report.completed_at.is_none());
    }

    #[test]
    fn test_provision_step_result_creation() {
        let result = ProvisionStepResult {
            step_name: "系统更新".to_string(),
            status: TaskStatus::Success,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 1500,
        };
        assert_eq!(result.step_name, "系统更新");
        assert_eq!(result.status, TaskStatus::Success);
    }

    #[test]
    fn test_provision_template_custom() {
        let tmpl = ProvisionTemplate {
            name: "minimal".to_string(),
            steps: vec![ProvisionStep {
                name: "仅更新".to_string(),
                step_type: ProvisionStepType::SystemUpdate,
                required: true,
            }],
        };
        assert_eq!(tmpl.steps.len(), 1);
        assert!(tmpl.steps[0].required);
    }

    #[test]
    fn test_provision_step_type_serialization() {
        let step = ProvisionStepType::InstallPackages {
            packages: vec!["vim".to_string(), "git".to_string()],
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ProvisionStepType::InstallPackages { .. }
        ));
    }

    // ============================================================
    // SshConfig tests
    // ============================================================

    #[test]
    fn test_ssh_config_clone() {
        let cfg = SshConfig {
            permit_root_login: false,
            password_auth: true,
            max_auth_tries: 6,
            port: 22,
        };
        let cloned = cfg.clone();
        assert_eq!(cloned, cfg);
    }

    #[test]
    fn test_ssh_config_debug() {
        let cfg = SshConfig {
            permit_root_login: true,
            password_auth: true,
            max_auth_tries: 3,
            port: 22,
        };
        let debug = format!("{:?}", cfg);
        assert!(debug.contains("SshConfig"));
        assert!(debug.contains("22"));
    }

    #[test]
    fn test_ssh_config_partial_eq() {
        let a = SshConfig {
            permit_root_login: false,
            password_auth: false,
            max_auth_tries: 3,
            port: 2222,
        };
        let b = SshConfig {
            permit_root_login: false,
            password_auth: false,
            max_auth_tries: 3,
            port: 2222,
        };
        let c = SshConfig {
            permit_root_login: true,
            password_auth: false,
            max_auth_tries: 3,
            port: 2222,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_ssh_config_serialization_roundtrip() {
        let cfg = SshConfig {
            permit_root_login: false,
            password_auth: false,
            max_auth_tries: 3,
            port: 2222,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: SshConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, cfg);
    }

    // ============================================================
    // FirewallRule tests
    // ============================================================

    #[test]
    fn test_firewall_rule_clone() {
        let rule = FirewallRule {
            port: 443,
            protocol: "tcp".to_string(),
            action: "allow".to_string(),
            source: Some("10.0.0.0/8".to_string()),
        };
        let cloned = rule.clone();
        assert_eq!(cloned, rule);
    }

    #[test]
    fn test_firewall_rule_debug() {
        let rule = FirewallRule {
            port: 80,
            protocol: "tcp".to_string(),
            action: "allow".to_string(),
            source: None,
        };
        let debug = format!("{:?}", rule);
        assert!(debug.contains("FirewallRule"));
        assert!(debug.contains("80"));
    }

    #[test]
    fn test_firewall_rule_partial_eq() {
        let a = FirewallRule {
            port: 22,
            protocol: "tcp".to_string(),
            action: "allow".to_string(),
            source: None,
        };
        let b = a.clone();
        let c = FirewallRule {
            port: 22,
            protocol: "tcp".to_string(),
            action: "deny".to_string(),
            source: None,
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_firewall_rule_serialization_roundtrip() {
        let rule = FirewallRule {
            port: 443,
            protocol: "tcp".to_string(),
            action: "allow".to_string(),
            source: Some("192.168.1.0/24".to_string()),
        };
        let json = serde_json::to_string(&rule).unwrap();
        let deserialized: FirewallRule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, rule);
    }

    // ============================================================
    // ProvisionTemplate tests
    // ============================================================

    #[test]
    fn test_provision_template_clone() {
        let tmpl = Provisioner::default_template();
        let cloned = tmpl.clone();
        assert_eq!(cloned.name, tmpl.name);
        assert_eq!(cloned.steps.len(), tmpl.steps.len());
    }

    #[test]
    fn test_provision_template_debug() {
        let tmpl = Provisioner::default_template();
        let debug = format!("{:?}", tmpl);
        assert!(debug.contains("ProvisionTemplate"));
        assert!(debug.contains("default"));
    }

    #[test]
    fn test_provision_template_serialization() {
        let tmpl = Provisioner::default_template();
        let json = serde_json::to_string(&tmpl).unwrap();
        let deserialized: ProvisionTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, tmpl.name);
        assert_eq!(deserialized.steps.len(), tmpl.steps.len());
    }

    // ============================================================
    // ProvisionStep tests
    // ============================================================

    #[test]
    fn test_provision_step_clone() {
        let step = ProvisionStep {
            name: "test".to_string(),
            step_type: ProvisionStepType::SystemUpdate,
            required: true,
        };
        let cloned = step.clone();
        assert_eq!(cloned.name, step.name);
        assert_eq!(cloned.required, step.required);
    }

    #[test]
    fn test_provision_step_debug() {
        let step = ProvisionStep {
            name: "安装软件".to_string(),
            step_type: ProvisionStepType::InstallPackages {
                packages: vec!["vim".to_string()],
            },
            required: false,
        };
        let debug = format!("{:?}", step);
        assert!(debug.contains("ProvisionStep"));
        assert!(debug.contains("安装软件"));
    }

    // ============================================================
    // ProvisionStepType all variant serialization
    // ============================================================

    #[test]
    fn test_provision_step_type_system_update_serialization() {
        let step = ProvisionStepType::SystemUpdate;
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProvisionStepType::SystemUpdate);
    }

    #[test]
    fn test_provision_step_type_create_user_serialization() {
        let step = ProvisionStepType::CreateUser {
            username: "deploy".to_string(),
            ssh_key: Some("ssh-rsa AAAA...".to_string()),
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProvisionStepType::CreateUser { .. }));
    }

    #[test]
    fn test_provision_step_type_sudo_config_serialization() {
        let step = ProvisionStepType::SudoConfig {
            username: "deploy".to_string(),
            rules: vec!["ALL".to_string(), "!root".to_string()],
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProvisionStepType::SudoConfig { .. }));
    }

    #[test]
    fn test_provision_step_type_ssh_hardening_serialization() {
        let step = ProvisionStepType::SshHardening {
            config: SshConfig {
                permit_root_login: false,
                password_auth: false,
                max_auth_tries: 3,
                port: 2222,
            },
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ProvisionStepType::SshHardening { .. }
        ));
    }

    #[test]
    fn test_provision_step_type_firewall_serialization() {
        let step = ProvisionStepType::Firewall {
            rules: vec![
                FirewallRule {
                    port: 22,
                    protocol: "tcp".to_string(),
                    action: "allow".to_string(),
                    source: None,
                },
                FirewallRule {
                    port: 443,
                    protocol: "tcp".to_string(),
                    action: "allow".to_string(),
                    source: Some("10.0.0.0/8".to_string()),
                },
            ],
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProvisionStepType::Firewall { .. }));
    }

    #[test]
    fn test_provision_step_type_timezone_serialization() {
        let step = ProvisionStepType::Timezone {
            tz: "Asia/Shanghai".to_string(),
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProvisionStepType::Timezone { .. }));
    }

    #[test]
    fn test_provision_step_type_ntp_serialization() {
        let step = ProvisionStepType::NtpServer {
            server: "ntp.aliyun.com".to_string(),
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProvisionStepType::NtpServer { .. }));
    }

    #[test]
    fn test_provision_step_type_kernel_params_serialization() {
        let step = ProvisionStepType::KernelParams {
            params: vec![
                ("vm.swappiness".to_string(), "10".to_string()),
                ("net.ipv4.ip_forward".to_string(), "1".to_string()),
            ],
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ProvisionStepType::KernelParams { .. }
        ));
    }

    #[test]
    fn test_provision_step_type_service_mgmt_serialization() {
        let step = ProvisionStepType::ServiceManagement {
            enable: vec!["sshd".to_string(), "nginx".to_string()],
            disable: vec!["cups".to_string(), "bluetooth".to_string()],
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ProvisionStepType::ServiceManagement { .. }
        ));
    }

    #[test]
    fn test_provision_step_type_custom_script_serialization() {
        let step = ProvisionStepType::CustomScript {
            script: "echo hello && ls -la".to_string(),
        };
        let json = serde_json::to_string(&step).unwrap();
        let deserialized: ProvisionStepType = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            ProvisionStepType::CustomScript { .. }
        ));
    }

    // ============================================================
    // ProvisionReport & ProvisionStepResult tests
    // ============================================================

    #[test]
    fn test_provision_report_clone() {
        let report = ProvisionReport {
            host: "server1".to_string(),
            template_name: "default".to_string(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            step_results: vec![],
            overall_status: TaskStatus::Success,
        };
        let cloned = report.clone();
        assert_eq!(cloned.host, report.host);
        assert_eq!(cloned.overall_status, report.overall_status);
    }

    #[test]
    fn test_provision_report_debug() {
        let report = ProvisionReport {
            host: "server1".to_string(),
            template_name: "default".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            step_results: vec![],
            overall_status: TaskStatus::Running,
        };
        let debug = format!("{:?}", report);
        assert!(debug.contains("ProvisionReport"));
        assert!(debug.contains("server1"));
    }

    #[test]
    fn test_provision_report_serialization() {
        let report = ProvisionReport {
            host: "server1".to_string(),
            template_name: "minimal".to_string(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            step_results: vec![ProvisionStepResult {
                step_name: "update".to_string(),
                status: TaskStatus::Success,
                stdout: "ok".to_string(),
                stderr: String::new(),
                duration_ms: 500,
            }],
            overall_status: TaskStatus::Success,
        };
        let json = serde_json::to_string(&report).unwrap();
        let deserialized: ProvisionReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.host, "server1");
        assert_eq!(deserialized.step_results.len(), 1);
    }

    #[test]
    fn test_provision_step_result_clone() {
        let result = ProvisionStepResult {
            step_name: "update".to_string(),
            status: TaskStatus::Success,
            stdout: "ok".to_string(),
            stderr: String::new(),
            duration_ms: 500,
        };
        let cloned = result.clone();
        assert_eq!(cloned.step_name, result.step_name);
        assert_eq!(cloned.duration_ms, result.duration_ms);
    }

    #[test]
    fn test_provision_step_result_debug() {
        let result = ProvisionStepResult {
            step_name: "install".to_string(),
            status: TaskStatus::Failed,
            stdout: String::new(),
            stderr: "error".to_string(),
            duration_ms: 1000,
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("ProvisionStepResult"));
        assert!(debug.contains("install"));
    }

    #[test]
    fn test_provision_step_result_serialization() {
        let result = ProvisionStepResult {
            step_name: "config".to_string(),
            status: TaskStatus::PartialSuccess,
            stdout: "partial".to_string(),
            stderr: "warning".to_string(),
            duration_ms: 2000,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ProvisionStepResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.step_name, "config");
        assert_eq!(deserialized.status, TaskStatus::PartialSuccess);
    }

    // ============================================================
    // Default template details
    // ============================================================

    #[test]
    fn test_default_template_step_names() {
        let tmpl = Provisioner::default_template();
        assert_eq!(tmpl.steps[0].name, "系统更新");
        assert_eq!(tmpl.steps[1].name, "安装基础软件");
        assert_eq!(tmpl.steps[2].name, "设置时区");
        assert_eq!(tmpl.steps[3].name, "NTP同步");
    }

    #[test]
    fn test_default_template_install_packages() {
        let tmpl = Provisioner::default_template();
        if let ProvisionStepType::InstallPackages { packages } = &tmpl.steps[1].step_type {
            assert!(packages.contains(&"vim".to_string()));
            assert!(packages.contains(&"curl".to_string()));
            assert!(packages.contains(&"git".to_string()));
            assert_eq!(packages.len(), 7);
        } else {
            panic!("Expected InstallPackages");
        }
    }

    #[test]
    fn test_default_template_timezone() {
        let tmpl = Provisioner::default_template();
        if let ProvisionStepType::Timezone { tz } = &tmpl.steps[2].step_type {
            assert_eq!(tz, "Asia/Shanghai");
        } else {
            panic!("Expected Timezone");
        }
    }

    #[test]
    fn test_default_template_ntp_server() {
        let tmpl = Provisioner::default_template();
        if let ProvisionStepType::NtpServer { server } = &tmpl.steps[3].step_type {
            assert_eq!(server, "ntp.aliyun.com");
        } else {
            panic!("Expected NtpServer");
        }
    }
}
