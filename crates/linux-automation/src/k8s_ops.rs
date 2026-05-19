//! R026: Kubernetes 集群运维模块
//!
//! 集群健康 / Pod 排查 / 资源管理 / 事件查看
//! 灵魂: 可追溯(kubectl操作审计) / 透明(状态推送) / 可控(策略可配)

use crate::audit::AuditLog;
use crate::error::Result;
use crate::executor::TaskExecutor;
use crate::models::*;
use tracing::info;

/// K8s 运维引擎
pub struct K8sOps;

impl K8sOps {
    /// 执行 K8s 操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        context: &str,
        action: &K8sAction,
        audit: &AuditLog,
    ) -> Result<K8sOpsResult> {
        let ctx_flag = if context.is_empty() {
            String::new()
        } else {
            format!("--context {}", context)
        };

        let (cmd, action_desc) = match action {
            K8sAction::ClusterHealth => (
                format!("kubectl {} cluster-info 2>&1", ctx_flag),
                "集群健康检查".to_string(),
            ),
            K8sAction::NodeStatus => (
                format!("kubectl {} get nodes -o wide --no-headers 2>&1", ctx_flag),
                "节点状态".to_string(),
            ),
            K8sAction::PodStatus { namespace } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_else(|| "--all-namespaces".to_string());
                (
                    format!(
                        "kubectl {} get pods {} -o wide --no-headers 2>&1",
                        ctx_flag, ns
                    ),
                    format!("Pod状态(ns={:?})", namespace),
                )
            }
            K8sAction::TroubleshootFailedPods { namespace } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_else(|| "--all-namespaces".to_string());
                (
                    format!(
                        "kubectl {} get pods {} --field-selector=status.phase!=Running,status.phase!=Succeeded --no-headers 2>&1",
                        ctx_flag, ns
                    ),
                    "排查失败Pod".to_string(),
                )
            }
            K8sAction::ResourceUsage { namespace } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_else(|| "--all-namespaces".to_string());
                (
                    format!("kubectl {} top pods {} --no-headers 2>&1", ctx_flag, ns),
                    "资源使用".to_string(),
                )
            }
            K8sAction::Events { namespace, limit } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_else(|| "--all-namespaces".to_string());
                (
                    format!(
                        "kubectl {} get events {} --sort-by='.lastTimestamp' | tail -{} 2>&1",
                        ctx_flag, ns, limit
                    ),
                    format!("事件(ns={:?}, limit={})", namespace, limit),
                )
            }
            K8sAction::Describe {
                resource_type,
                name,
                namespace,
            } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_default();
                (
                    format!(
                        "kubectl {} describe {} {} {} 2>&1",
                        ctx_flag, resource_type, name, ns
                    ),
                    format!("描述 {}/{}", resource_type, name),
                )
            }
            K8sAction::Delete {
                resource_type,
                name,
                namespace,
                force,
            } => {
                let ns = namespace
                    .as_deref()
                    .map(|n| format!("-n {}", n))
                    .unwrap_or_default();
                let force_flag = if *force {
                    "--force --grace-period=0"
                } else {
                    ""
                };
                (
                    format!(
                        "kubectl {} delete {} {} {} {} 2>&1",
                        ctx_flag, resource_type, name, ns, force_flag
                    ),
                    format!("删除 {}/{} (force={})", resource_type, name, force),
                )
            }
            K8sAction::Kubectl { args } => {
                let args_str = args.join(" ");
                (
                    format!("kubectl {} {} 2>&1", ctx_flag, args_str),
                    format!("kubectl {}", args_str),
                )
            }
        };

        let result = executor.execute_command(&[host.to_string()], &cmd).await?;
        let hr = result.host_results.first();

        let status = match hr {
            Some(h) if h.exit_code == 0 => TaskStatus::Success,
            Some(h) => {
                if h.stderr.contains("not found") || h.stdout.contains("not found") {
                    TaskStatus::Failed
                } else if h.exit_code == 0 {
                    TaskStatus::Success
                } else {
                    TaskStatus::Failed
                }
            }
            None => TaskStatus::Failed,
        };

        let output = hr.map(|h| h.stdout.clone()).unwrap_or_default();

        // 解析节点和Pod
        let nodes = if matches!(action, K8sAction::NodeStatus) {
            Self::parse_nodes(&output)
        } else {
            Vec::new()
        };

        let pods = if matches!(
            action,
            K8sAction::PodStatus { .. } | K8sAction::TroubleshootFailedPods { .. }
        ) {
            Self::parse_pods(&output)
        } else {
            Vec::new()
        };

        // 生成建议
        let mut recommendations = Vec::new();
        if matches!(action, K8sAction::TroubleshootFailedPods { .. }) {
            for pod in &pods {
                if pod.status == "CrashLoopBackOff" {
                    recommendations.push(format!(
                        "Pod {}/{} 处于 CrashLoopBackOff, 建议检查容器日志: kubectl logs {} -n {}",
                        pod.namespace, pod.name, pod.name, pod.namespace
                    ));
                } else if pod.status == "OOMKilled" {
                    recommendations.push(format!(
                        "Pod {}/{} 被 OOM Kill, 建议增加内存限制",
                        pod.namespace, pod.name
                    ));
                } else if pod.status == "ImagePullBackOff" {
                    recommendations.push(format!(
                        "Pod {}/{} 镜像拉取失败, 检查镜像名和仓库凭据",
                        pod.namespace, pod.name
                    ));
                } else if pod.status == "Pending" {
                    recommendations.push(format!(
                        "Pod {}/{} 处于 Pending, 检查资源配额和调度约束",
                        pod.namespace, pod.name
                    ));
                }
            }
        }

        // 可追溯: 记录审计
        audit.log_action(
            "system",
            "K8sOps",
            host,
            &format!("{}: {:?}", action_desc, status),
        )?;

        info!(host = %host, context = %context, action = %action_desc, status = ?status, "K8s操作完成");

        Ok(K8sOpsResult {
            context: context.to_string(),
            action: action.clone(),
            status,
            nodes,
            pods,
            output,
            recommendations,
            audit_trail: vec![],
        })
    }

    /// 解析 kubectl get nodes 输出
    fn parse_nodes(output: &str) -> Vec<K8sNode> {
        output
            .lines()
            .filter(|line| !line.is_empty() && !line.contains("error"))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 7 {
                    Some(K8sNode {
                        name: parts[0].to_string(),
                        status: parts[1].to_string(),
                        roles: parts[2].to_string(),
                        age: parts[3].to_string(),
                        version: parts[4].to_string(),
                        cpu: parts.get(5).unwrap_or(&"").to_string(),
                        memory: parts.get(6).unwrap_or(&"").to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// 解析 kubectl get pods 输出
    fn parse_pods(output: &str) -> Vec<K8sPod> {
        output
            .lines()
            .filter(|line| !line.is_empty() && !line.contains("error"))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let (namespace, name, ready, status, restarts, age) = if parts.len() >= 6 {
                        // All-namespaces 模式: NAMESPACE NAME READY STATUS RESTARTS AGE
                        (
                            parts[0].to_string(),
                            parts[1].to_string(),
                            parts[2].to_string(),
                            parts[3].to_string(),
                            parts[4].parse().unwrap_or(0),
                            parts[5].to_string(),
                        )
                    } else {
                        // 单 namespace 模式: NAME READY STATUS RESTARTS AGE
                        (
                            String::new(),
                            parts[0].to_string(),
                            parts[1].to_string(),
                            parts[2].to_string(),
                            parts[3].parse().unwrap_or(0),
                            parts[4].to_string(),
                        )
                    };
                    Some(K8sPod {
                        name,
                        namespace,
                        ready,
                        status,
                        restarts,
                        age,
                        cpu: None,
                        memory: None,
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
    fn test_k8s_action_variants() {
        let actions = vec![
            K8sAction::ClusterHealth,
            K8sAction::NodeStatus,
            K8sAction::PodStatus {
                namespace: Some("default".to_string()),
            },
            K8sAction::PodStatus { namespace: None },
            K8sAction::TroubleshootFailedPods {
                namespace: Some("default".to_string()),
            },
            K8sAction::ResourceUsage { namespace: None },
            K8sAction::Events {
                namespace: None,
                limit: 50,
            },
            K8sAction::Describe {
                resource_type: "pod".to_string(),
                name: "web".to_string(),
                namespace: Some("default".to_string()),
            },
            K8sAction::Delete {
                resource_type: "pod".to_string(),
                name: "web".to_string(),
                namespace: Some("default".to_string()),
                force: false,
            },
            K8sAction::Kubectl {
                args: vec!["get".to_string(), "svc".to_string()],
            },
        ];
        assert_eq!(actions.len(), 10);
    }

    #[test]
    fn test_parse_nodes() {
        let output = "node1   Ready    control-plane   5d   v1.28.0   4       8Gi\nnode2   Ready    <none>          5d   v1.28.0   8       16Gi";
        let nodes = K8sOps::parse_nodes(output);
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].name, "node1");
        assert_eq!(nodes[0].status, "Ready");
        assert_eq!(nodes[1].name, "node2");
    }

    #[test]
    fn test_parse_nodes_empty() {
        let nodes = K8sOps::parse_nodes("");
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_parse_pods_all_ns() {
        let output = "default    web-abc123   1/1     Running   0   5d\ndefault    db-def456    0/1     CrashLoopBackOff   3   1h";
        let pods = K8sOps::parse_pods(output);
        assert_eq!(pods.len(), 2);
        assert_eq!(pods[0].namespace, "default");
        assert_eq!(pods[0].name, "web-abc123");
        assert_eq!(pods[1].status, "CrashLoopBackOff");
        assert_eq!(pods[1].restarts, 3);
    }

    #[test]
    fn test_parse_pods_single_ns() {
        let output = "web-abc123   1/1     Running   0   5d";
        let pods = K8sOps::parse_pods(output);
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].name, "web-abc123");
        assert!(pods[0].namespace.is_empty());
    }

    #[test]
    fn test_parse_pods_empty() {
        let pods = K8sOps::parse_pods("");
        assert!(pods.is_empty());
    }

    #[test]
    fn test_k8s_node_creation() {
        let node = K8sNode {
            name: "node1".to_string(),
            status: "Ready".to_string(),
            roles: "control-plane".to_string(),
            age: "5d".to_string(),
            version: "v1.28.0".to_string(),
            cpu: "4".to_string(),
            memory: "8Gi".to_string(),
        };
        assert_eq!(node.name, "node1");
        assert_eq!(node.status, "Ready");
    }

    #[test]
    fn test_k8s_pod_creation() {
        let pod = K8sPod {
            name: "web".to_string(),
            namespace: "default".to_string(),
            ready: "1/1".to_string(),
            status: "Running".to_string(),
            restarts: 0,
            age: "5d".to_string(),
            cpu: Some("100m".to_string()),
            memory: Some("128Mi".to_string()),
        };
        assert_eq!(pod.name, "web");
        assert_eq!(pod.restarts, 0);
        assert!(pod.cpu.is_some());
    }

    #[test]
    fn test_k8s_ops_result_creation() {
        let result = K8sOpsResult {
            context: "test".to_string(),
            action: K8sAction::ClusterHealth,
            status: TaskStatus::Success,
            nodes: vec![],
            pods: vec![],
            output: "ok".to_string(),
            recommendations: vec![],
            audit_trail: vec![],
        };
        assert_eq!(result.context, "test");
        assert_eq!(result.status, TaskStatus::Success);
    }

    #[test]
    fn test_k8s_action_serialization() {
        let actions = vec![
            K8sAction::ClusterHealth,
            K8sAction::NodeStatus,
            K8sAction::PodStatus {
                namespace: Some("default".to_string()),
            },
            K8sAction::Kubectl {
                args: vec!["get".to_string(), "svc".to_string()],
            },
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let d: K8sAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, d);
        }
    }

    #[test]
    fn test_parse_pods_with_error_line() {
        let output = "error: the server doesn't have a resource type \"pods\"\n";
        let pods = K8sOps::parse_pods(output);
        assert!(pods.is_empty());
    }
}
