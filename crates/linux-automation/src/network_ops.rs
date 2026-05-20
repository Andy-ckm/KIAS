//! R032: 网络配置和故障排查模块
//!
//! 网络接口管理 / 路由配置 / DNS诊断 / 连通性测试 / 防火墙管理 / 端口扫描
//! 灵魂: 可追溯(操作审计) / 透明(状态推送) / 可控(策略可配)
//!
//! 竞品参考: Ansible network modules, Puppet network resource, Chef network cookbook
//! AgentGuard差异化: 网络诊断→根因分析→自动修复→合规审计（竞品只做配置下发）

use tracing::info;

use crate::audit::AuditLog;
use crate::error::{AutomationError, Result};
use crate::executor::TaskExecutor;
use crate::models::*;

/// 网络运维引擎
///
/// 提供网络配置和故障排查：接口管理、路由配置、DNS诊断、
/// 连通性测试、防火墙管理、端口扫描、综合诊断
/// 所有操作自动生成 SSH 命令并通过 TaskExecutor 执行
pub struct NetworkManager;

impl NetworkManager {
    /// 执行网络操作
    pub async fn execute(
        executor: &TaskExecutor,
        host: &str,
        action: &NetworkAction,
        audit: &AuditLog,
    ) -> Result<NetworkOpsResult> {
        let audit_id = uuid::Uuid::new_v4().to_string();
        let mut commands_executed = Vec::new();

        let (action_desc, result) = match action {
            NetworkAction::ListInterfaces => {
                let cmd = "ip -j addr show";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let interfaces = Self::parse_interfaces(&output);
                (
                    "列出网络接口".to_string(),
                    NetworkOpsResult {
                        action: "ListInterfaces".to_string(),
                        success: true,
                        interfaces,
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::InterfaceDetail { interface } => {
                let cmd = format!("ip -j addr show {}", interface);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let interfaces = Self::parse_interfaces(&output);
                (
                    format!("查看接口详情: {}", interface),
                    NetworkOpsResult {
                        action: "InterfaceDetail".to_string(),
                        success: !interfaces.is_empty(),
                        interfaces,
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::SetIp {
                interface,
                ip,
                prefix,
                gateway,
            } => {
                let cmd = format!("ip addr add {}/{} dev {}", ip, prefix, interface);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let mut errors = Vec::new();
                if let Some(gw) = gateway {
                    let gw_cmd = format!("ip route add default via {} dev {}", gw, interface);
                    match Self::run_cmd(executor, host, &gw_cmd).await {
                        Ok(_) => commands_executed.push(gw_cmd),
                        Err(e) => errors.push(format!("设置网关失败: {}", e)),
                    }
                }
                (
                    format!("配置IP: {}/{} on {}", ip, prefix, interface),
                    NetworkOpsResult {
                        action: "SetIp".to_string(),
                        success: errors.is_empty(),
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors,
                    },
                )
            }
            NetworkAction::SetInterfaceState { interface, up } => {
                let state = if *up { "up" } else { "down" };
                let cmd = format!("ip link set {} {}", interface, state);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("{}接口: {}", if *up { "启用" } else { "禁用" }, interface),
                    NetworkOpsResult {
                        action: "SetInterfaceState".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::ShowRoutes => {
                let cmd = "ip -j route";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let routes = Self::parse_routes(&output);
                (
                    "查看路由表".to_string(),
                    NetworkOpsResult {
                        action: "ShowRoutes".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes,
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::AddRoute {
                destination,
                gateway,
                interface,
            } => {
                let mut cmd = format!("ip route add {} via {}", destination, gateway);
                if let Some(iface) = interface {
                    cmd.push_str(&format!(" dev {}", iface));
                }
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("添加路由: {} via {}", destination, gateway),
                    NetworkOpsResult {
                        action: "AddRoute".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::DeleteRoute { destination } => {
                let cmd = format!("ip route del {}", destination);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("删除路由: {}", destination),
                    NetworkOpsResult {
                        action: "DeleteRoute".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::DnsDiag { domain, server } => {
                let server_arg = server
                    .as_ref()
                    .map(|s| format!("@{}", s))
                    .unwrap_or_default();
                let cmd = format!("dig +short +stats {} {}", server_arg, domain);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let dns_result = DnsDiagResult {
                    domain: domain.clone(),
                    server: server.clone().unwrap_or_else(|| "system".to_string()),
                    resolved_ips: Self::parse_dig_ips(&output),
                    query_time_ms: Self::parse_dig_time(&output),
                    authoritative: output.contains("flags: qr aa"),
                    records: Self::parse_dig_records(&output),
                };
                (
                    format!("DNS诊断: {}", domain),
                    NetworkOpsResult {
                        action: "DnsDiag".to_string(),
                        success: !dns_result.resolved_ips.is_empty(),
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: Some(dns_result),
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::SetDns { servers } => {
                let servers_str = servers.join(" ");
                let cmd = format!(
                    "echo 'nameservers={}' > /etc/resolv.conf.head && resolvconf -u",
                    servers_str
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("配置DNS: {}", servers_str),
                    NetworkOpsResult {
                        action: "SetDns".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::Ping {
                host: target,
                count,
                timeout_secs,
            } => {
                let c = count.unwrap_or(4);
                let t = timeout_secs.unwrap_or(5);
                let cmd = format!("ping -c {} -W {} -q {}", c, t, target);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                let ping_result = Self::parse_ping(&output, target);
                (
                    format!("连通性测试: {}", target),
                    NetworkOpsResult {
                        action: "Ping".to_string(),
                        success: ping_result.received > 0,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: Some(ping_result),
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::Traceroute {
                host: target,
                max_hops,
            } => {
                let m = max_hops.unwrap_or(30);
                let cmd = format!("traceroute -n -m {} {}", m, target);
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("路由追踪: {}", target),
                    NetworkOpsResult {
                        action: "Traceroute".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::PortCheck {
                host: target,
                ports,
                timeout_secs,
            } => {
                let t = timeout_secs.unwrap_or(3);
                let mut port_results = Vec::new();
                for port in ports {
                    let cmd = format!(
                        "timeout {} bash -c 'echo >/dev/tcp/{}/{}' 2>&1 && echo OPEN || echo CLOSED",
                        t, target, port
                    );
                    let output = Self::run_cmd(executor, host, &cmd).await?;
                    commands_executed.push(format!("port_check {}:{}", target, port));
                    port_results.push(PortCheckResult {
                        host: target.clone(),
                        port: *port,
                        open: output.contains("OPEN"),
                        service: Self::guess_port_service(*port),
                        response_time_ms: 0,
                    });
                }
                (
                    format!("端口扫描: {} ({})", target, ports.len()),
                    NetworkOpsResult {
                        action: "PortCheck".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results,
                        connections: vec![],
                        firewall_rules: vec![],
                        output: String::new(),
                        errors: vec![],
                    },
                )
            }
            NetworkAction::ShowConnections { protocol, state } => {
                let mut cmd = "ss -tulnp".to_string();
                if let Some(proto) = protocol {
                    cmd = format!("ss -{}lnp", proto);
                }
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd.clone());
                let connections = Self::parse_connections(&output, state);
                (
                    "查看网络连接".to_string(),
                    NetworkOpsResult {
                        action: "ShowConnections".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections,
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::ShowFirewall => {
                let cmd = "iptables -L -n --line-numbers";
                let output = Self::run_cmd(executor, host, cmd).await?;
                commands_executed.push(cmd.to_string());
                let firewall_rules = Self::parse_iptables(&output);
                (
                    "查看防火墙规则".to_string(),
                    NetworkOpsResult {
                        action: "ShowFirewall".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules,
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::AddFirewallRule { rule } => {
                let cmd = format!(
                    "iptables -A INPUT -p {} --dport {} -j {}",
                    rule.protocol, rule.port, rule.action
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!(
                        "添加防火墙规则: {}:{}/{}",
                        rule.port, rule.protocol, rule.action
                    ),
                    NetworkOpsResult {
                        action: "AddFirewallRule".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![rule.clone()],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::DeleteFirewallRule { port, protocol } => {
                let cmd = format!(
                    "iptables -D INPUT -p {} --dport {} -j ACCEPT",
                    protocol, port
                );
                let output = Self::run_cmd(executor, host, &cmd).await?;
                commands_executed.push(cmd);
                (
                    format!("删除防火墙规则: {}/{}", port, protocol),
                    NetworkOpsResult {
                        action: "DeleteFirewallRule".to_string(),
                        success: true,
                        interfaces: vec![],
                        routes: vec![],
                        dns_result: None,
                        ping_result: None,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output,
                        errors: vec![],
                    },
                )
            }
            NetworkAction::BandwidthTest {
                host: target,
                duration_secs,
            } => {
                let d = duration_secs.unwrap_or(10);
                let cmd = format!("iperf3 -c {} -t {} -J", target, d);
                let output = Self::run_cmd(executor, host, &cmd).await;
                commands_executed.push(format!("iperf3 -c {} -t {}", target, d));
                match output {
                    Ok(output) => (
                        format!("带宽测试: {} ({}s)", target, d),
                        NetworkOpsResult {
                            action: "BandwidthTest".to_string(),
                            success: true,
                            interfaces: vec![],
                            routes: vec![],
                            dns_result: None,
                            ping_result: None,
                            port_results: vec![],
                            connections: vec![],
                            firewall_rules: vec![],
                            output,
                            errors: vec![],
                        },
                    ),
                    Err(e) => (
                        format!("带宽测试: {} ({}s)", target, d),
                        NetworkOpsResult {
                            action: "BandwidthTest".to_string(),
                            success: false,
                            interfaces: vec![],
                            routes: vec![],
                            dns_result: None,
                            ping_result: None,
                            port_results: vec![],
                            connections: vec![],
                            firewall_rules: vec![],
                            output: String::new(),
                            errors: vec![format!("iperf3失败: {}", e)],
                        },
                    ),
                }
            }
            NetworkAction::FullDiag { host: target } => {
                let mut errors = Vec::new();
                // 1. Ping
                let ping_cmd = format!("ping -c 3 -W 2 -q {}", target);
                let ping_output = Self::run_cmd(executor, host, &ping_cmd).await;
                commands_executed.push(ping_cmd);
                let ping_result = match ping_output {
                    Ok(ref out) => Some(Self::parse_ping(out, target)),
                    Err(e) => {
                        errors.push(format!("Ping失败: {}", e));
                        None
                    }
                };
                // 2. DNS
                let dns_cmd = format!("dig +short {}", target);
                let dns_output = Self::run_cmd(executor, host, &dns_cmd).await;
                commands_executed.push(dns_cmd);
                let dns_result = match dns_output {
                    Ok(ref out) => Some(DnsDiagResult {
                        domain: target.clone(),
                        server: "system".to_string(),
                        resolved_ips: out
                            .lines()
                            .map(|l| l.trim().to_string())
                            .filter(|l| !l.is_empty())
                            .collect(),
                        query_time_ms: 0,
                        authoritative: false,
                        records: vec![],
                    }),
                    Err(e) => {
                        errors.push(format!("DNS查询失败: {}", e));
                        None
                    }
                };
                // 3. Traceroute
                let trace_cmd = format!("traceroute -n -m 15 {}", target);
                let trace_output = Self::run_cmd(executor, host, &trace_cmd).await;
                commands_executed.push(trace_cmd);
                let trace_str = match trace_output {
                    Ok(out) => out,
                    Err(e) => {
                        errors.push(format!("Traceroute失败: {}", e));
                        String::new()
                    }
                };
                let combined_output = format!(
                    "=== Ping ===\n{}\n=== DNS ===\n{}\n=== Traceroute ===\n{}",
                    ping_result
                        .as_ref()
                        .map(|p| format!(
                            "sent={}, received={}, loss={:.1}%, rtt_avg={:.1}ms",
                            p.sent, p.received, p.loss_pct, p.rtt_avg_ms
                        ))
                        .unwrap_or_else(|| "N/A".to_string()),
                    dns_result
                        .as_ref()
                        .map(|d| d.resolved_ips.join(", "))
                        .unwrap_or_else(|| "N/A".to_string()),
                    trace_str
                );
                (
                    format!("综合诊断: {}", target),
                    NetworkOpsResult {
                        action: "FullDiag".to_string(),
                        success: errors.is_empty(),
                        interfaces: vec![],
                        routes: vec![],
                        dns_result,
                        ping_result,
                        port_results: vec![],
                        connections: vec![],
                        firewall_rules: vec![],
                        output: combined_output,
                        errors,
                    },
                )
            }
        };

        info!(
            host = %host,
            action = %action_desc,
            audit_id = %audit_id,
            commands = ?commands_executed,
            "NetworkManager: 操作完成"
        );

        // 审计记录
        let _ = audit.log_action(
            "system",
            "NetworkManager",
            &format!("{}: {}", host, action_desc),
            &serde_json::to_string(&commands_executed).unwrap_or_default(),
        );

        Ok(result)
    }

    /// 预览命令（不执行）
    pub fn preview_commands(action: &NetworkAction) -> Vec<String> {
        match action {
            NetworkAction::ListInterfaces => vec!["ip -j addr show".to_string()],
            NetworkAction::InterfaceDetail { interface } => {
                vec![format!("ip -j addr show {}", interface)]
            }
            NetworkAction::SetIp {
                interface,
                ip,
                prefix,
                gateway,
            } => {
                let mut cmds = vec![format!("ip addr add {}/{} dev {}", ip, prefix, interface)];
                if let Some(gw) = gateway {
                    cmds.push(format!("ip route add default via {} dev {}", gw, interface));
                }
                cmds
            }
            NetworkAction::SetInterfaceState { interface, up } => {
                vec![format!(
                    "ip link set {} {}",
                    interface,
                    if *up { "up" } else { "down" }
                )]
            }
            NetworkAction::ShowRoutes => vec!["ip -j route".to_string()],
            NetworkAction::AddRoute {
                destination,
                gateway,
                interface,
            } => {
                let mut cmd = format!("ip route add {} via {}", destination, gateway);
                if let Some(iface) = interface {
                    cmd.push_str(&format!(" dev {}", iface));
                }
                vec![cmd]
            }
            NetworkAction::DeleteRoute { destination } => {
                vec![format!("ip route del {}", destination)]
            }
            NetworkAction::DnsDiag { domain, server } => {
                let server_arg = server
                    .as_ref()
                    .map(|s| format!("@{}", s))
                    .unwrap_or_default();
                vec![format!("dig +short +stats {} {}", server_arg, domain)]
            }
            NetworkAction::SetDns { servers } => {
                vec![format!(
                    "echo 'nameservers={}' > /etc/resolv.conf.head && resolvconf -u",
                    servers.join(" ")
                )]
            }
            NetworkAction::Ping {
                host,
                count,
                timeout_secs,
            } => vec![format!(
                "ping -c {} -W {} -q {}",
                count.unwrap_or(4),
                timeout_secs.unwrap_or(5),
                host
            )],
            NetworkAction::Traceroute { host, max_hops } => {
                vec![format!(
                    "traceroute -n -m {} {}",
                    max_hops.unwrap_or(30),
                    host
                )]
            }
            NetworkAction::PortCheck {
                host,
                ports,
                timeout_secs,
            } => {
                let t = timeout_secs.unwrap_or(3);
                ports
                    .iter()
                    .map(|p| format!("timeout {} bash -c 'echo >/dev/tcp/{}/{}'", t, host, p))
                    .collect()
            }
            NetworkAction::ShowConnections { protocol, state: _ } => {
                let proto = protocol.as_deref().unwrap_or("tul");
                vec![format!("ss -{}lnp", proto)]
            }
            NetworkAction::ShowFirewall => {
                vec!["iptables -L -n --line-numbers".to_string()]
            }
            NetworkAction::AddFirewallRule { rule } => vec![format!(
                "iptables -A INPUT -p {} --dport {} -j {}",
                rule.protocol, rule.port, rule.action
            )],
            NetworkAction::DeleteFirewallRule { port, protocol } => vec![format!(
                "iptables -D INPUT -p {} --dport {} -j ACCEPT",
                protocol, port
            )],
            NetworkAction::BandwidthTest {
                host,
                duration_secs,
            } => vec![format!(
                "iperf3 -c {} -t {} -J",
                host,
                duration_secs.unwrap_or(10)
            )],
            NetworkAction::FullDiag { host } => vec![
                format!("ping -c 3 -W 2 -q {}", host),
                format!("dig +short {}", host),
                format!("traceroute -n -m 15 {}", host),
            ],
        }
    }

    // --- 辅助方法 ---

    async fn run_cmd(executor: &TaskExecutor, host: &str, cmd: &str) -> Result<String> {
        let result = executor
            .execute_command(&[host.to_string()], cmd)
            .await
            .map_err(|e| {
                AutomationError::NetworkOperation(format!("命令执行失败 '{}': {}", cmd, e))
            })?;
        result
            .host_results
            .first()
            .map(|h| h.stdout.clone())
            .ok_or_else(|| AutomationError::NetworkOperation("无主机结果".to_string()))
    }

    fn parse_interfaces(output: &str) -> Vec<NetworkInterface> {
        // 尝试 JSON 解析 (ip -j addr show)
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(arr) = arr.as_array() {
                return arr
                    .iter()
                    .filter_map(|item| {
                        let name = item.get("ifname")?.as_str()?.to_string();
                        let state = item
                            .get("operstate")
                            .and_then(|v| v.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        let mac = item
                            .get("address")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mtu = item.get("mtu").and_then(|v| v.as_u64()).unwrap_or(1500) as u32;
                        let mut ipv4 = Vec::new();
                        let mut ipv6 = Vec::new();
                        if let Some(addrs) = item.get("addr_info").and_then(|v| v.as_array()) {
                            for addr in addrs {
                                let ip = addr.get("local")?.as_str()?.to_string();
                                let prefix =
                                    addr.get("prefixlen").and_then(|v| v.as_u64()).unwrap_or(0)
                                        as u8;
                                let family = addr
                                    .get("family")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                let scope = addr
                                    .get("scope")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("global")
                                    .to_string();
                                let ip_addr = IpAddress {
                                    address: ip,
                                    prefix,
                                    scope,
                                };
                                if family == "inet" {
                                    ipv4.push(ip_addr);
                                } else {
                                    ipv6.push(ip_addr);
                                }
                            }
                        }
                        Some(NetworkInterface {
                            name,
                            state,
                            mac,
                            mtu,
                            ipv4,
                            ipv6,
                        })
                    })
                    .collect();
            }
        }
        vec![]
    }

    fn parse_routes(output: &str) -> Vec<RouteEntry> {
        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(arr) = arr.as_array() {
                return arr
                    .iter()
                    .map(|item| {
                        let destination = item
                            .get("dst")
                            .and_then(|v| v.as_str())
                            .unwrap_or("default")
                            .to_string();
                        let gateway = item
                            .get("gateway")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0.0.0.0")
                            .to_string();
                        let interface = item
                            .get("dev")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let metric = item
                            .get("metric")
                            .and_then(|v| v.as_u64())
                            .map(|v| v as u32);
                        let proto = item
                            .get("protocol")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        RouteEntry {
                            destination,
                            gateway,
                            interface,
                            metric,
                            proto,
                        }
                    })
                    .collect();
            }
        }
        vec![]
    }

    fn parse_dig_ips(output: &str) -> Vec<String> {
        output
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with(';') && !l.starts_with('#'))
            .filter(|l| {
                l.chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
            })
            .map(|l| l.to_string())
            .collect()
    }

    fn parse_dig_time(output: &str) -> u64 {
        for line in output.lines() {
            if line.contains("Query time:") {
                if let Some(ms) = line.split_whitespace().nth(3) {
                    return ms.parse().unwrap_or(0);
                }
            }
        }
        0
    }

    fn parse_dig_records(output: &str) -> Vec<DnsRecord> {
        output
            .lines()
            .filter(|l| !l.starts_with(';') && !l.starts_with('#'))
            .filter_map(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                if parts.len() >= 5 {
                    let ttl: u32 = parts[1].parse().unwrap_or(0);
                    Some(DnsRecord {
                        record_type: parts[3].to_string(),
                        value: parts[4..].join(" "),
                        ttl,
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    fn parse_ping(output: &str, target: &str) -> PingResult {
        let mut sent = 0u32;
        let mut received = 0u32;
        let mut rtt_min = 0.0f64;
        let mut rtt_avg = 0.0f64;
        let mut rtt_max = 0.0f64;

        for line in output.lines() {
            if line.contains("packets transmitted") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    sent = parts[0].parse().unwrap_or(0);
                    received = parts[3].parse().unwrap_or(0);
                }
            }
            if line.contains("min/avg/max") || line.contains("rtt min/avg/max") {
                if let Some(eq_part) = line.split('=').nth(1) {
                    let nums: Vec<&str> = eq_part.trim().split('/').collect();
                    if nums.len() >= 3 {
                        rtt_min = nums[0].trim().parse().unwrap_or(0.0);
                        rtt_avg = nums[1].trim().parse().unwrap_or(0.0);
                        rtt_max = nums[2].trim().parse().unwrap_or(0.0);
                    }
                }
            }
        }

        let loss_pct = if sent > 0 {
            ((sent - received) as f64 / sent as f64) * 100.0
        } else {
            100.0
        };

        PingResult {
            host: target.to_string(),
            sent,
            received,
            loss_pct,
            rtt_min_ms: rtt_min,
            rtt_avg_ms: rtt_avg,
            rtt_max_ms: rtt_max,
        }
    }

    fn parse_connections(output: &str, filter_state: &Option<String>) -> Vec<NetworkConnection> {
        output
            .lines()
            .skip(1) // skip header
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // ss -tunap output: State Recv-Q Send-Q LocalAddr:Port PeerAddr:Port Process
                // needs at least 5 columns: State Recv-Q Send-Q Local Peer
                if parts.len() < 5 {
                    return None;
                }
                let state = parts[0].to_string();
                if let Some(ref fs) = filter_state {
                    if !state.contains(fs) {
                        return None;
                    }
                }
                let local = parts[3].to_string();
                let remote = parts[4].to_string();
                let (local_addr, local_port) = Self::parse_addr_port(&local);
                let (remote_addr, remote_port) = Self::parse_addr_port(&remote);
                let process = parts
                    .get(5)
                    .map(|s| s.to_string())
                    .filter(|s| s.contains('"'));
                Some(NetworkConnection {
                    protocol: state.clone(),
                    local_addr,
                    local_port,
                    remote_addr,
                    remote_port,
                    state,
                    process,
                })
            })
            .collect()
    }

    fn parse_addr_port(s: &str) -> (String, u16) {
        if let Some(idx) = s.rfind(':') {
            let addr = s[..idx].to_string();
            let port = s[idx + 1..].parse().unwrap_or(0);
            (addr, port)
        } else {
            (s.to_string(), 0)
        }
    }

    fn parse_iptables(output: &str) -> Vec<FirewallRule> {
        output
            .lines()
            .filter(|l| l.contains("ACCEPT") || l.contains("DROP") || l.contains("REJECT"))
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 {
                    return None;
                }
                // Find protocol and port from flags
                let mut protocol = "all".to_string();
                let mut port = 0u16;
                let mut action = "ACCEPT".to_string();
                let mut source = None;
                // Track position after "--" separator for iptables -L format
                let mut after_separator = false;
                let mut source_idx = 0;
                for (i, part) in parts.iter().enumerate() {
                    match *part {
                        "-p" => {
                            if let Some(p) = parts.get(i + 1) {
                                protocol = p.to_string();
                            }
                        }
                        "--dport" => {
                            if let Some(p) = parts.get(i + 1) {
                                port = p.parse().unwrap_or(0);
                            }
                        }
                        "ACCEPT" | "DROP" | "REJECT" => action = part.to_string(),
                        "-s" => {
                            if let Some(s) = parts.get(i + 1) {
                                source = Some(s.to_string());
                            }
                        }
                        "--" => {
                            after_separator = true;
                            source_idx = i + 1;
                        }
                        _ => {
                            // Handle "dpt:22" format from iptables -L output
                            if let Some(stripped) = part.strip_prefix("dpt:") {
                                if let Ok(p) = stripped.parse::<u16>() {
                                    port = p;
                                }
                            }
                        }
                    }
                }
                // If we found "--" separator and no explicit -s flag, use the field after "--" as source
                if after_separator && source.is_none() {
                    if let Some(s) = parts.get(source_idx) {
                        if *s != "0.0.0.0/0" && *s != "anywhere" {
                            source = Some(s.to_string());
                        }
                    }
                }
                if port == 0 {
                    return None;
                }
                Some(FirewallRule {
                    port,
                    protocol,
                    action,
                    source,
                })
            })
            .collect()
    }

    fn guess_port_service(port: u16) -> Option<String> {
        match port {
            22 => Some("ssh".to_string()),
            80 => Some("http".to_string()),
            443 => Some("https".to_string()),
            3306 => Some("mysql".to_string()),
            5432 => Some("postgresql".to_string()),
            6379 => Some("redis".to_string()),
            8080 => Some("http-alt".to_string()),
            2379 => Some("etcd".to_string()),
            9090 => Some("prometheus".to_string()),
            3000 => Some("grafana".to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_list_interfaces() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::ListInterfaces);
        assert_eq!(cmds, vec!["ip -j addr show"]);
    }

    #[test]
    fn test_preview_interface_detail() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::InterfaceDetail {
            interface: "eth0".to_string(),
        });
        assert_eq!(cmds, vec!["ip -j addr show eth0"]);
    }

    #[test]
    fn test_preview_set_ip_no_gateway() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::SetIp {
            interface: "eth0".to_string(),
            ip: "192.168.1.100".to_string(),
            prefix: 24,
            gateway: None,
        });
        assert_eq!(cmds, vec!["ip addr add 192.168.1.100/24 dev eth0"]);
    }

    #[test]
    fn test_preview_set_ip_with_gateway() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::SetIp {
            interface: "eth0".to_string(),
            ip: "192.168.1.100".to_string(),
            prefix: 24,
            gateway: Some("192.168.1.1".to_string()),
        });
        assert_eq!(cmds.len(), 2);
        assert!(cmds[1].contains("192.168.1.1"));
    }

    #[test]
    fn test_preview_set_interface_state_up() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::SetInterfaceState {
            interface: "eth0".to_string(),
            up: true,
        });
        assert_eq!(cmds, vec!["ip link set eth0 up"]);
    }

    #[test]
    fn test_preview_set_interface_state_down() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::SetInterfaceState {
            interface: "eth0".to_string(),
            up: false,
        });
        assert_eq!(cmds, vec!["ip link set eth0 down"]);
    }

    #[test]
    fn test_preview_show_routes() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::ShowRoutes);
        assert_eq!(cmds, vec!["ip -j route"]);
    }

    #[test]
    fn test_preview_add_route_with_interface() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::AddRoute {
            destination: "10.0.0.0/8".to_string(),
            gateway: "192.168.1.1".to_string(),
            interface: Some("eth0".to_string()),
        });
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("dev eth0"));
    }

    #[test]
    fn test_preview_add_route_no_interface() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::AddRoute {
            destination: "10.0.0.0/8".to_string(),
            gateway: "192.168.1.1".to_string(),
            interface: None,
        });
        assert_eq!(cmds.len(), 1);
        assert!(!cmds[0].contains("dev"));
    }

    #[test]
    fn test_preview_delete_route() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::DeleteRoute {
            destination: "10.0.0.0/8".to_string(),
        });
        assert_eq!(cmds, vec!["ip route del 10.0.0.0/8"]);
    }

    #[test]
    fn test_preview_dns_diag() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::DnsDiag {
            domain: "example.com".to_string(),
            server: Some("8.8.8.8".to_string()),
        });
        assert!(cmds[0].contains("@8.8.8.8"));
        assert!(cmds[0].contains("example.com"));
    }

    #[test]
    fn test_preview_dns_diag_no_server() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::DnsDiag {
            domain: "example.com".to_string(),
            server: None,
        });
        assert!(cmds[0].contains("example.com"));
    }

    #[test]
    fn test_preview_set_dns() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::SetDns {
            servers: vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
        });
        assert!(cmds[0].contains("8.8.8.8"));
        assert!(cmds[0].contains("8.8.4.4"));
    }

    #[test]
    fn test_preview_ping_defaults() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::Ping {
            host: "10.0.0.1".to_string(),
            count: None,
            timeout_secs: None,
        });
        assert!(cmds[0].contains("-c 4"));
        assert!(cmds[0].contains("-W 5"));
        assert!(cmds[0].contains("10.0.0.1"));
    }

    #[test]
    fn test_preview_ping_custom() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::Ping {
            host: "10.0.0.1".to_string(),
            count: Some(10),
            timeout_secs: Some(3),
        });
        assert!(cmds[0].contains("-c 10"));
        assert!(cmds[0].contains("-W 3"));
    }

    #[test]
    fn test_preview_traceroute() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::Traceroute {
            host: "8.8.8.8".to_string(),
            max_hops: Some(20),
        });
        assert!(cmds[0].contains("-m 20"));
        assert!(cmds[0].contains("8.8.8.8"));
    }

    #[test]
    fn test_preview_port_check() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::PortCheck {
            host: "10.0.0.1".to_string(),
            ports: vec![22, 80, 443],
            timeout_secs: Some(2),
        });
        assert_eq!(cmds.len(), 3);
        assert!(cmds[0].contains("/22"));
        assert!(cmds[1].contains("/80"));
        assert!(cmds[2].contains("/443"));
    }

    #[test]
    fn test_preview_show_connections() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::ShowConnections {
            protocol: None,
            state: None,
        });
        assert!(cmds[0].contains("ss"));
    }

    #[test]
    fn test_preview_show_connections_tcp() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::ShowConnections {
            protocol: Some("t".to_string()),
            state: None,
        });
        assert!(cmds[0].contains("ss -tlnp"));
    }

    #[test]
    fn test_preview_show_firewall() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::ShowFirewall);
        assert_eq!(cmds, vec!["iptables -L -n --line-numbers"]);
    }

    #[test]
    fn test_preview_add_firewall_rule() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::AddFirewallRule {
            rule: FirewallRule {
                port: 8080,
                protocol: "tcp".to_string(),
                action: "ACCEPT".to_string(),
                source: None,
            },
        });
        assert!(cmds[0].contains("8080"));
        assert!(cmds[0].contains("tcp"));
        assert!(cmds[0].contains("ACCEPT"));
    }

    #[test]
    fn test_preview_delete_firewall_rule() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::DeleteFirewallRule {
            port: 8080,
            protocol: "tcp".to_string(),
        });
        assert!(cmds[0].contains("8080"));
        assert!(cmds[0].contains("tcp"));
    }

    #[test]
    fn test_preview_bandwidth_test() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::BandwidthTest {
            host: "10.0.0.1".to_string(),
            duration_secs: Some(30),
        });
        assert!(cmds[0].contains("iperf3"));
        assert!(cmds[0].contains("-t 30"));
    }

    #[test]
    fn test_preview_full_diag() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::FullDiag {
            host: "10.0.0.1".to_string(),
        });
        assert_eq!(cmds.len(), 3);
        assert!(cmds[0].contains("ping"));
        assert!(cmds[1].contains("dig"));
        assert!(cmds[2].contains("traceroute"));
    }

    // --- 解析函数测试 ---

    #[test]
    fn test_parse_ping_normal() {
        let output = "PING 10.0.0.1 (10.0.0.1) 56(84) bytes of data.\n\n--- 10.0.0.1 ping statistics ---\n4 packets transmitted, 4 received, 0% packet loss, time 3003ms\nrtt min/avg/max/mdev = 0.500/1.200/2.100/0.500 ms";
        let result = NetworkManager::parse_ping(output, "10.0.0.1");
        assert_eq!(result.sent, 4);
        assert_eq!(result.received, 4);
        assert!((result.loss_pct - 0.0).abs() < 0.01);
        assert!((result.rtt_avg_ms - 1.2).abs() < 0.01);
        assert_eq!(result.host, "10.0.0.1");
    }

    #[test]
    fn test_parse_ping_loss() {
        let output = "4 packets transmitted, 2 received, 50% packet loss\nrtt min/avg/max/mdev = 1.000/2.000/3.000/0.500 ms";
        let result = NetworkManager::parse_ping(output, "10.0.0.1");
        assert_eq!(result.sent, 4);
        assert_eq!(result.received, 2);
        assert!((result.loss_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_ping_all_lost() {
        let output = "4 packets transmitted, 0 received, 100% packet loss";
        let result = NetworkManager::parse_ping(output, "10.0.0.1");
        assert_eq!(result.sent, 4);
        assert_eq!(result.received, 0);
        assert!((result.loss_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_dig_ips() {
        let output = "93.184.216.34\n";
        let ips = NetworkManager::parse_dig_ips(output);
        assert_eq!(ips, vec!["93.184.216.34"]);
    }

    #[test]
    fn test_parse_dig_ips_multiple() {
        let output = "93.184.216.34\n93.184.216.35\n";
        let ips = NetworkManager::parse_dig_ips(output);
        assert_eq!(ips.len(), 2);
    }

    #[test]
    fn test_parse_dig_time() {
        let output = ";; Query time: 42 msec\n;; SERVER: 8.8.8.8#53(8.8.8.8)\n93.184.216.34";
        let time = NetworkManager::parse_dig_time(output);
        assert_eq!(time, 42);
    }

    #[test]
    fn test_parse_dig_time_no_match() {
        let output = "93.184.216.34\n";
        let time = NetworkManager::parse_dig_time(output);
        assert_eq!(time, 0);
    }

    #[test]
    fn test_parse_addr_port() {
        let (addr, port) = NetworkManager::parse_addr_port("192.168.1.1:8080");
        assert_eq!(addr, "192.168.1.1");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_addr_port_ipv6() {
        let (addr, port) = NetworkManager::parse_addr_port("[::1]:8080");
        assert_eq!(addr, "[::1]");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_addr_port_no_port() {
        let (addr, port) = NetworkManager::parse_addr_port("*");
        assert_eq!(addr, "*");
        assert_eq!(port, 0);
    }

    #[test]
    fn test_guess_port_service_known() {
        assert_eq!(
            NetworkManager::guess_port_service(22),
            Some("ssh".to_string())
        );
        assert_eq!(
            NetworkManager::guess_port_service(80),
            Some("http".to_string())
        );
        assert_eq!(
            NetworkManager::guess_port_service(443),
            Some("https".to_string())
        );
        assert_eq!(
            NetworkManager::guess_port_service(3306),
            Some("mysql".to_string())
        );
        assert_eq!(
            NetworkManager::guess_port_service(5432),
            Some("postgresql".to_string())
        );
        assert_eq!(
            NetworkManager::guess_port_service(6379),
            Some("redis".to_string())
        );
    }

    #[test]
    fn test_guess_port_service_unknown() {
        assert_eq!(NetworkManager::guess_port_service(12345), None);
    }

    #[test]
    fn test_parse_iptables_with_rules() {
        let output = "Chain INPUT (policy ACCEPT)\nnum  target     prot opt source               destination\n1    ACCEPT     tcp  --  0.0.0.0/0            0.0.0.0/0            tcp dpt:22\n2    DROP       tcp  --  10.0.0.0/8           0.0.0.0/0            tcp dpt:8080\n";
        let rules = NetworkManager::parse_iptables(output);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].port, 22);
        assert_eq!(rules[0].action, "ACCEPT");
        assert_eq!(rules[1].port, 8080);
        assert_eq!(rules[1].action, "DROP");
        assert_eq!(rules[1].source, Some("10.0.0.0/8".to_string()));
    }

    #[test]
    fn test_parse_iptables_no_rules() {
        let output = "Chain INPUT (policy ACCEPT)\nnum  target     prot opt source               destination\n";
        let rules = NetworkManager::parse_iptables(output);
        assert!(rules.is_empty());
    }

    #[test]
    fn test_network_action_equality() {
        assert_eq!(NetworkAction::ListInterfaces, NetworkAction::ListInterfaces);
        assert_eq!(NetworkAction::ShowRoutes, NetworkAction::ShowRoutes);
        assert_eq!(NetworkAction::ShowFirewall, NetworkAction::ShowFirewall);
        assert_ne!(NetworkAction::ListInterfaces, NetworkAction::ShowRoutes);
    }

    #[test]
    fn test_network_action_serialization_roundtrip() {
        let actions = vec![
            NetworkAction::ListInterfaces,
            NetworkAction::ShowRoutes,
            NetworkAction::ShowFirewall,
            NetworkAction::Ping {
                host: "10.0.0.1".to_string(),
                count: Some(4),
                timeout_secs: None,
            },
            NetworkAction::DnsDiag {
                domain: "example.com".to_string(),
                server: None,
            },
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let deserialized: NetworkAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, deserialized);
        }
    }

    #[test]
    fn test_network_ops_result_creation() {
        let result = NetworkOpsResult {
            action: "ListInterfaces".to_string(),
            success: true,
            interfaces: vec![],
            routes: vec![],
            dns_result: None,
            ping_result: None,
            port_results: vec![],
            connections: vec![],
            firewall_rules: vec![],
            output: "ok".to_string(),
            errors: vec![],
        };
        assert!(result.success);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_network_interface_creation() {
        let iface = NetworkInterface {
            name: "eth0".to_string(),
            state: "UP".to_string(),
            mac: "00:11:22:33:44:55".to_string(),
            mtu: 1500,
            ipv4: vec![IpAddress {
                address: "192.168.1.100".to_string(),
                prefix: 24,
                scope: "global".to_string(),
            }],
            ipv6: vec![],
        };
        assert_eq!(iface.name, "eth0");
        assert_eq!(iface.mtu, 1500);
        assert_eq!(iface.ipv4.len(), 1);
    }

    #[test]
    fn test_firewall_rule_creation() {
        let rule = FirewallRule {
            port: 443,
            protocol: "tcp".to_string(),
            action: "ACCEPT".to_string(),
            source: Some("10.0.0.0/8".to_string()),
        };
        assert_eq!(rule.port, 443);
        assert_eq!(rule.protocol, "tcp");
        assert!(rule.source.is_some());
    }

    #[test]
    fn test_ping_result_loss_calculation() {
        let result = PingResult {
            host: "test".to_string(),
            sent: 10,
            received: 7,
            loss_pct: 30.0,
            rtt_min_ms: 1.0,
            rtt_avg_ms: 2.0,
            rtt_max_ms: 5.0,
        };
        assert_eq!(result.sent - result.received, 3);
        assert!((result.loss_pct - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_interfaces_empty() {
        let interfaces = NetworkManager::parse_interfaces("invalid json");
        assert!(interfaces.is_empty());
    }

    #[test]
    fn test_parse_routes_empty() {
        let routes = NetworkManager::parse_routes("invalid json");
        assert!(routes.is_empty());
    }

    #[test]
    fn test_preview_bandwidth_test_defaults() {
        let cmds = NetworkManager::preview_commands(&NetworkAction::BandwidthTest {
            host: "10.0.0.1".to_string(),
            duration_secs: None,
        });
        assert!(cmds[0].contains("-t 10"));
    }

    // --- parse_dig_records 测试 ---

    #[test]
    fn test_parse_dig_records_normal() {
        let output = "example.com.\t300\tIN\tA\t93.184.216.34\nexample.com.\t300\tIN\tAAAA\t2606:2800:220:1:248:1893:25c8:1946\n";
        let records = NetworkManager::parse_dig_records(output);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].record_type, "A");
        assert_eq!(records[0].value, "93.184.216.34");
        assert_eq!(records[0].ttl, 300);
        assert_eq!(records[1].record_type, "AAAA");
        assert!(records[1].value.contains("2606"));
    }

    #[test]
    fn test_parse_dig_records_skips_comments() {
        let output = "; <<>> DiG 9.18 <<>> example.com\n;; QUESTION SECTION:\n;; ANSWER SECTION:\nexample.com.\t300\tIN\tA\t93.184.216.34\n";
        let records = NetworkManager::parse_dig_records(output);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_type, "A");
    }

    #[test]
    fn test_parse_dig_records_empty() {
        let records = NetworkManager::parse_dig_records("");
        assert!(records.is_empty());
    }

    #[test]
    fn test_parse_dig_records_short_line_skipped() {
        let output = "example.com.\t300\n"; // only 2 fields, needs >= 5
        let records = NetworkManager::parse_dig_records(output);
        assert!(records.is_empty());
    }

    #[test]
    fn test_parse_dig_records_mx_record() {
        let output = "example.com.\t3600\tIN\tMX\t10 mail.example.com.\n";
        let records = NetworkManager::parse_dig_records(output);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_type, "MX");
        assert_eq!(records[0].value, "10 mail.example.com.");
        assert_eq!(records[0].ttl, 3600);
    }

    #[test]
    fn test_parse_dig_records_ns_record() {
        let output = "example.com.\t86400\tIN\tNS\tns1.example.com.\nexample.com.\t86400\tIN\tNS\tns2.example.com.\n";
        let records = NetworkManager::parse_dig_records(output);
        assert_eq!(records.len(), 2);
        assert!(records.iter().all(|r| r.record_type == "NS"));
    }

    // --- parse_connections 测试 ---

    #[test]
    fn test_parse_connections_normal() {
        let output = "State      Recv-Q Send-Q Local Address:Port    Peer Address:Port  Process\nLISTEN     0      128    0.0.0.0:22             0.0.0.0:*          users:((\"sshd\",pid=1234,fd=3))\nESTABLISHED 0     0      192.168.1.100:22       192.168.1.1:54321   users:((\"sshd\",pid=5678,fd=4))\n";
        let conns = NetworkManager::parse_connections(output, &None);
        assert_eq!(conns.len(), 2);
        assert_eq!(conns[0].protocol, "LISTEN");
        assert_eq!(conns[0].local_addr, "0.0.0.0");
        assert_eq!(conns[0].local_port, 22);
        assert_eq!(conns[1].protocol, "ESTABLISHED");
        assert_eq!(conns[1].remote_port, 54321);
    }

    #[test]
    fn test_parse_connections_with_filter() {
        let output = "State      Recv-Q Send-Q Local Address:Port    Peer Address:Port  Process\nLISTEN     0      128    0.0.0.0:22             0.0.0.0:*\nESTABLISHED 0     0      192.168.1.100:22       192.168.1.1:54321\n";
        let filter = Some("LISTEN".to_string());
        let conns = NetworkManager::parse_connections(output, &filter);
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].protocol, "LISTEN");
    }

    #[test]
    fn test_parse_connections_empty() {
        let conns = NetworkManager::parse_connections("", &None);
        assert!(conns.is_empty());
    }

    #[test]
    fn test_parse_connections_header_only() {
        let output = "State      Recv-Q Send-Q Local Address:Port    Peer Address:Port  Process\n";
        let conns = NetworkManager::parse_connections(output, &None);
        assert!(conns.is_empty());
    }

    #[test]
    fn test_parse_connections_short_line_skipped() {
        let output = "header line\nonly three words\n";
        let conns = NetworkManager::parse_connections(output, &None);
        assert!(conns.is_empty());
    }

    #[test]
    fn test_parse_connections_with_process() {
        let output = "Proto Recv-Q Send-Q Local Address:Port    Peer Address:Port  Process\ntcp   0      0      127.0.0.1:6379         0.0.0.0:*          users:((\"redis-server\",pid=999,fd=6))\n";
        let conns = NetworkManager::parse_connections(output, &None);
        assert_eq!(conns.len(), 1);
        assert_eq!(conns[0].process, Some("users:((\"redis-server\",pid=999,fd=6))".to_string()));
    }
}
