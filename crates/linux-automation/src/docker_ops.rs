//! R025: Docker 容器运维模块
//!
//! 容器生命周期 / 镜像管理 / 资源监控 / 清理
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)

use crate::audit::AuditLog;
use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::models::*;

/// Docker 运维引擎
pub struct DockerOps;

impl DockerOps {
    /// 执行 Docker 操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &DockerAction,
        audit: &AuditLog,
    ) -> Result<DockerOpsResult> {
        let (cmd, action_desc) = match action {
            DockerAction::ListContainers { all } => {
                let flag = if *all { "-a" } else { "" };
                (
                    format!("docker ps {} --format '{{{{.ID}}}}|{{{{.Names}}}}|{{{{.Image}}}}|{{{{.Status}}}}|{{{{.Ports}}}}|{{{{.CreatedAt}}}}'", flag),
                    format!("列出容器(all={})", all),
                )
            }
            DockerAction::ContainerStatus { container } => (
                format!(
                    "docker inspect --format '{{{{.State.Status}}}}|{{{{.State.StartedAt}}}}|{{{{.Config.Image}}}}' {}",
                    container
                ),
                format!("查看容器状态: {}", container),
            ),
            DockerAction::Start { container } => (
                format!("docker start {}", container),
                format!("启动容器: {}", container),
            ),
            DockerAction::Stop { container } => (
                format!("docker stop {}", container),
                format!("停止容器: {}", container),
            ),
            DockerAction::Restart { container } => (
                format!("docker restart {}", container),
                format!("重启容器: {}", container),
            ),
            DockerAction::Remove { container, force } => {
                let flag = if *force { "-f" } else { "" };
                (
                    format!("docker rm {} {}", flag, container),
                    format!("删除容器: {} (force={})", container, force),
                )
            }
            DockerAction::Logs { container, tail } => (
                format!("docker logs --tail {} {}", tail, container),
                format!("查看日志: {} (tail={})", container, tail),
            ),
            DockerAction::Stats => (
                "docker stats --no-stream --format '{{.Name}}|{{.CPUPerc}}|{{.MemUsage}}|{{.NetIO}}|{{.BlockIO}}'".to_string(),
                "查看资源监控".to_string(),
            ),
            DockerAction::Prune {
                images,
                containers,
                volumes,
            } => {
                let mut cmds = Vec::new();
                if *containers {
                    cmds.push("docker container prune -f");
                }
                if *images {
                    cmds.push("docker image prune -f");
                }
                if *volumes {
                    cmds.push("docker volume prune -f");
                }
                (cmds.join(" && "), "清理Docker资源".to_string())
            }
            DockerAction::ListImages => (
                "docker images --format '{{.Repository}}:{{.Tag}}|{{.Size}}|{{.CreatedSince}}'".to_string(),
                "列出镜像".to_string(),
            ),
            DockerAction::Pull { image } => (
                format!("docker pull {}", image),
                format!("拉取镜像: {}", image),
            ),
        };

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let hr = result.host_results.first();

        let status = match hr {
            Some(h) if h.exit_code == 0 => TaskStatus::Success,
            _ => TaskStatus::Failed,
        };

        let stdout = hr.map(|h| h.stdout.clone()).unwrap_or_default();
        let stderr = hr.map(|h| h.stderr.clone()).unwrap_or_default();

        // 解析容器列表
        let containers = if matches!(action, DockerAction::ListContainers { .. }) {
            Self::parse_container_list(&stdout)
        } else {
            Vec::new()
        };

        // 可追溯: 记录审计
        audit.log_action(
            "system",
            "DockerOps",
            host,
            &format!("{}: {:?}", action_desc, status),
        )?;

        let message = match &status {
            TaskStatus::Success => format!("{} 成功", action_desc),
            _ => format!("{} 失败: {}", action_desc, stderr),
        };

        Ok(DockerOpsResult {
            host: host.to_string(),
            action: action.clone(),
            status,
            containers,
            message,
            audit_trail: vec![],
        })
    }

    /// 解析 docker ps 输出
    fn parse_container_list(output: &str) -> Vec<DockerContainer> {
        output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                if parts.len() >= 5 {
                    Some(DockerContainer {
                        id: parts[0].to_string(),
                        name: parts[1].to_string(),
                        image: parts[2].to_string(),
                        status: parts[3].to_string(),
                        state: String::new(),
                        ports: parts[4].to_string(),
                        created: parts.get(5).unwrap_or(&"").to_string(),
                        cpu_percent: None,
                        mem_usage: None,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_action_variants() {
        let actions = vec![
            DockerAction::ListContainers { all: true },
            DockerAction::ContainerStatus {
                container: "web".to_string(),
            },
            DockerAction::Start {
                container: "web".to_string(),
            },
            DockerAction::Stop {
                container: "web".to_string(),
            },
            DockerAction::Restart {
                container: "web".to_string(),
            },
            DockerAction::Remove {
                container: "web".to_string(),
                force: true,
            },
            DockerAction::Logs {
                container: "web".to_string(),
                tail: 100,
            },
            DockerAction::Stats,
            DockerAction::Prune {
                images: true,
                containers: true,
                volumes: false,
            },
            DockerAction::ListImages,
            DockerAction::Pull {
                image: "nginx:latest".to_string(),
            },
        ];
        assert_eq!(actions.len(), 11);
    }

    #[test]
    fn test_parse_container_list() {
        let output = "abc123|web|nginx:latest|Up 2 days|0.0.0.0:80->80/tcp|2024-01-01\ndef456|db|mysql:8|Exited (0) 3 hours ago||2024-01-02";
        let containers = DockerOps::parse_container_list(output);
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].name, "web");
        assert_eq!(containers[0].image, "nginx:latest");
        assert_eq!(containers[1].name, "db");
    }

    #[test]
    fn test_parse_container_list_empty() {
        let containers = DockerOps::parse_container_list("");
        assert!(containers.is_empty());
    }

    #[test]
    fn test_docker_container_creation() {
        let c = DockerContainer {
            id: "abc123".to_string(),
            name: "web".to_string(),
            image: "nginx:latest".to_string(),
            status: "Up 2 days".to_string(),
            state: "running".to_string(),
            ports: "80/tcp".to_string(),
            created: "2024-01-01".to_string(),
            cpu_percent: Some(5.2),
            mem_usage: Some("128MB".to_string()),
        };
        assert_eq!(c.name, "web");
        assert!(c.cpu_percent.is_some());
    }

    #[test]
    fn test_docker_ops_result_creation() {
        let result = DockerOpsResult {
            host: "server1".to_string(),
            action: DockerAction::ListContainers { all: false },
            status: TaskStatus::Success,
            containers: vec![],
            message: "ok".to_string(),
            audit_trail: vec![],
        };
        assert_eq!(result.host, "server1");
        assert_eq!(result.status, TaskStatus::Success);
    }

    #[test]
    fn test_docker_action_serialization() {
        let actions = vec![
            DockerAction::ListContainers { all: true },
            DockerAction::Stats,
            DockerAction::Prune {
                images: false,
                containers: true,
                volumes: false,
            },
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let d: DockerAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, d);
        }
    }

    #[test]
    fn test_parse_container_partial_fields() {
        let output = "abc123|web|nginx|Up";
        let containers = DockerOps::parse_container_list(output);
        assert!(containers.is_empty()); // 不够5个字段
    }

    #[test]
    fn test_docker_action_partial_eq() {
        assert_eq!(
            DockerAction::ListContainers { all: true },
            DockerAction::ListContainers { all: true }
        );
        assert_ne!(
            DockerAction::ListContainers { all: true },
            DockerAction::ListContainers { all: false }
        );
    }
}
