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
}
