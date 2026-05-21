//! # Agent Sandbox Isolation
//!
//! Provides resource limits, filesystem isolation, network policies, and
//! capability-based access control for agent execution environments.
//! Inspired by Linux namespaces, cgroups, and seccomp-bpf.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

// ── Errors ─────────────────────────────────────────────────────────────

/// Sandbox operation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxError {
    /// The agent exceeded its resource limits.
    ResourceLimitExceeded(String),
    /// The agent attempted a disallowed filesystem operation.
    FilesystemDenied(String),
    /// The agent attempted a disallowed network operation.
    NetworkDenied(String),
    /// The agent attempted a disallowed syscall.
    SyscallDenied(String),
    /// The agent attempted a disallowed capability.
    CapabilityDenied(String),
    /// Sandbox configuration is invalid.
    InvalidConfig(String),
    /// Internal sandbox error.
    Internal(String),
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimitExceeded(e) => write!(f, "Resource limit exceeded: {e}"),
            Self::FilesystemDenied(e) => write!(f, "Filesystem access denied: {e}"),
            Self::NetworkDenied(e) => write!(f, "Network access denied: {e}"),
            Self::SyscallDenied(e) => write!(f, "Syscall denied: {e}"),
            Self::CapabilityDenied(e) => write!(f, "Capability denied: {e}"),
            Self::InvalidConfig(e) => write!(f, "Invalid config: {e}"),
            Self::Internal(e) => write!(f, "Internal error: {e}"),
        }
    }
}

impl std::error::Error for SandboxError {}

// ── Resource Limits ────────────────────────────────────────────────────

/// Resource limits for an agent sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory in bytes (default: 512MB).
    pub max_memory_bytes: u64,
    /// Maximum CPU time in milliseconds per request (default: 30000 = 30s).
    pub max_cpu_time_ms: u64,
    /// Maximum number of open file descriptors.
    pub max_open_files: u32,
    /// Maximum number of concurrent network connections.
    pub max_network_connections: u32,
    /// Maximum disk write bytes per operation.
    pub max_write_bytes: u64,
    /// Maximum number of child processes.
    pub max_child_processes: u32,
    /// Maximum output size in bytes.
    pub max_output_bytes: u64,
    /// Wall-clock timeout in seconds.
    pub timeout_seconds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024,       // 512MB
            max_cpu_time_ms: 30_000,                    // 30s
            max_open_files: 64,
            max_network_connections: 16,
            max_write_bytes: 64 * 1024 * 1024,          // 64MB
            max_child_processes: 4,
            max_output_bytes: 10 * 1024 * 1024,         // 10MB
            timeout_seconds: 60,
        }
    }
}

// ── Filesystem Policy ──────────────────────────────────────────────────

/// Filesystem access policy for the sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    /// Allowed paths (can be directories or files). Supports glob patterns.
    pub allowed_paths: Vec<String>,
    /// Denied paths (override allowed_paths). Supports glob patterns.
    pub denied_paths: Vec<String>,
    /// Whether the sandbox has read-only filesystem.
    pub read_only: bool,
    /// Allow temporary file creation in designated temp dirs.
    pub allow_temp: bool,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            allowed_paths: vec!["/tmp/sandbox/*".to_string()],
            denied_paths: vec![
                "/etc/*".to_string(),
                "/proc/*".to_string(),
                "/sys/*".to_string(),
                "/dev/*".to_string(),
            ],
            read_only: false,
            allow_temp: true,
        }
    }
}

// ── Network Policy ─────────────────────────────────────────────────────

/// Network access policy for the sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Whether networking is enabled at all.
    pub enabled: bool,
    /// Allowed destination CIDR ranges.
    pub allowed_cidrs: Vec<String>,
    /// Denied destination CIDR ranges (overrides allowed).
    pub denied_cidrs: Vec<String>,
    /// Allowed destination ports.
    pub allowed_ports: Vec<u16>,
    /// Whether DNS resolution is allowed.
    pub allow_dns: bool,
    /// Maximum request rate (requests per second).
    pub max_rps: u32,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_cidrs: vec!["0.0.0.0/0".to_string()],
            denied_cidrs: vec![
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
                "169.254.0.0/16".to_string(),
            ],
            allowed_ports: vec![80, 443],
            allow_dns: true,
            max_rps: 100,
        }
    }
}

// ── Syscall Filter ─────────────────────────────────────────────────────

/// Syscall filtering policy (seccomp-style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallFilter {
    /// Mode: allowlist (only these allowed) or denylist (these blocked).
    pub mode: SyscallFilterMode,
    /// Syscall names in the list.
    pub syscalls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyscallFilterMode {
    Allowlist,
    Denylist,
}

impl Default for SyscallFilter {
    fn default() -> Self {
        Self {
            mode: SyscallFilterMode::Denylist,
            syscalls: vec![
                "ptrace".to_string(),
                "mount".to_string(),
                "umount2".to_string(),
                "reboot".to_string(),
                "kexec_load".to_string(),
                "init_module".to_string(),
                "finit_module".to_string(),
                "delete_module".to_string(),
                "bpf".to_string(),
                "userfaultfd".to_string(),
            ],
        }
    }
}

// ── Capability ─────────────────────────────────────────────────────────

/// Linux-style capabilities that can be granted to a sandbox.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SandboxCapability {
    /// Read files within allowed paths.
    FileRead,
    /// Write files within allowed paths.
    FileWrite,
    /// Make outbound network requests.
    NetworkOutbound,
    /// Listen for inbound connections.
    NetworkInbound,
    /// Spawn child processes.
    ProcessSpawn,
    /// Access environment variables.
    EnvAccess,
    /// Execute shell commands.
    ShellExec,
    /// Access GPU devices.
    GpuAccess,
    /// Access to clipboard.
    ClipboardAccess,
}

// ── Sandbox Configuration ──────────────────────────────────────────────

/// Complete sandbox configuration for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Unique sandbox instance ID.
    pub id: String,
    /// Agent this sandbox belongs to.
    pub agent_id: String,
    /// Resource limits.
    pub resource_limits: ResourceLimits,
    /// Filesystem policy.
    pub filesystem: FilesystemPolicy,
    /// Network policy.
    pub network: NetworkPolicy,
    /// Syscall filter.
    pub syscall_filter: SyscallFilter,
    /// Granted capabilities.
    pub capabilities: HashSet<SandboxCapability>,
    /// Environment variables to inject (key -> value).
    pub env_vars: HashMap<String, String>,
    /// Whether the sandbox is currently active.
    pub active: bool,
    /// When the sandbox was created.
    pub created_at: DateTime<Utc>,
}

impl SandboxConfig {
    /// Create a new sandbox config with defaults for the given agent.
    pub fn new(id: &str, agent_id: &str) -> Self {
        let mut capabilities = HashSet::new();
        capabilities.insert(SandboxCapability::FileRead);
        capabilities.insert(SandboxCapability::NetworkOutbound);

        Self {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            resource_limits: ResourceLimits::default(),
            filesystem: FilesystemPolicy::default(),
            network: NetworkPolicy::default(),
            syscall_filter: SyscallFilter::default(),
            capabilities,
            env_vars: HashMap::new(),
            active: true,
            created_at: Utc::now(),
        }
    }

    /// Create a minimal sandbox (read-only, no network, no shell).
    pub fn minimal(id: &str, agent_id: &str) -> Self {
        let mut capabilities = HashSet::new();
        capabilities.insert(SandboxCapability::FileRead);

        Self {
            id: id.to_string(),
            agent_id: agent_id.to_string(),
            resource_limits: ResourceLimits {
                max_memory_bytes: 128 * 1024 * 1024,
                max_cpu_time_ms: 10_000,
                max_open_files: 16,
                max_network_connections: 0,
                max_write_bytes: 0,
                max_child_processes: 0,
                max_output_bytes: 1024 * 1024,
                timeout_seconds: 30,
            },
            filesystem: FilesystemPolicy {
                read_only: true,
                allow_temp: false,
                ..FilesystemPolicy::default()
            },
            network: NetworkPolicy {
                enabled: false,
                ..NetworkPolicy::default()
            },
            syscall_filter: SyscallFilter::default(),
            capabilities,
            env_vars: HashMap::new(),
            active: true,
            created_at: Utc::now(),
        }
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), SandboxError> {
        if self.resource_limits.max_memory_bytes == 0 {
            return Err(SandboxError::InvalidConfig(
                "max_memory_bytes must be > 0".to_string(),
            ));
        }
        if self.resource_limits.timeout_seconds == 0 {
            return Err(SandboxError::InvalidConfig(
                "timeout_seconds must be > 0".to_string(),
            ));
        }
        Ok(())
    }
}

// ── Sandbox Manager ────────────────────────────────────────────────────

/// Manages sandbox lifecycles and enforces policies.
pub struct SandboxManager {
    sandboxes: HashMap<String, SandboxConfig>,
}

impl SandboxManager {
    pub fn new() -> Self {
        Self {
            sandboxes: HashMap::new(),
        }
    }

    /// Create and register a new sandbox.
    pub fn create_sandbox(&mut self, config: SandboxConfig) -> Result<(), SandboxError> {
        config.validate()?;
        self.sandboxes.insert(config.id.clone(), config);
        Ok(())
    }

    /// Destroy a sandbox.
    pub fn destroy_sandbox(&mut self, sandbox_id: &str) -> bool {
        self.sandboxes.remove(sandbox_id).is_some()
    }

    /// Get a sandbox config.
    pub fn get_sandbox(&self, sandbox_id: &str) -> Option<&SandboxConfig> {
        self.sandboxes.get(sandbox_id)
    }

    /// List all sandbox IDs.
    pub fn list_sandboxes(&self) -> Vec<String> {
        self.sandboxes.keys().cloned().collect()
    }

    /// Check if an agent has a specific capability.
    pub fn check_capability(
        &self,
        sandbox_id: &str,
        capability: &SandboxCapability,
    ) -> Result<(), SandboxError> {
        let sandbox = self
            .sandboxes
            .get(sandbox_id)
            .ok_or_else(|| SandboxError::Internal(format!("Sandbox {sandbox_id} not found")))?;

        if !sandbox.active {
            return Err(SandboxError::Internal("Sandbox is not active".to_string()));
        }

        if sandbox.capabilities.contains(capability) {
            Ok(())
        } else {
            Err(SandboxError::CapabilityDenied(format!(
                "Capability {:?} not granted to sandbox {}",
                capability, sandbox_id
            )))
        }
    }

    /// Check filesystem access.
    pub fn check_filesystem_access(
        &self,
        sandbox_id: &str,
        path: &str,
        write: bool,
    ) -> Result<(), SandboxError> {
        let sandbox = self
            .sandboxes
            .get(sandbox_id)
            .ok_or_else(|| SandboxError::Internal(format!("Sandbox {sandbox_id} not found")))?;

        if write && sandbox.filesystem.read_only {
            return Err(SandboxError::FilesystemDenied(
                "Filesystem is read-only".to_string(),
            ));
        }

        if write {
            self.check_capability(sandbox_id, &SandboxCapability::FileWrite)?;
        } else {
            self.check_capability(sandbox_id, &SandboxCapability::FileRead)?;
        }

        // Check denied paths first
        for denied in &sandbox.filesystem.denied_paths {
            if path_matches(path, denied) {
                return Err(SandboxError::FilesystemDenied(format!(
                    "Path {path} matches denied pattern {denied}"
                )));
            }
        }

        // Check allowed paths
        for allowed in &sandbox.filesystem.allowed_paths {
            if path_matches(path, allowed) {
                return Ok(());
            }
        }

        Err(SandboxError::FilesystemDenied(format!(
            "Path {path} not in allowed paths"
        )))
    }

    /// Check network access.
    pub fn check_network_access(
        &self,
        sandbox_id: &str,
        host: &str,
        port: u16,
    ) -> Result<(), SandboxError> {
        let sandbox = self
            .sandboxes
            .get(sandbox_id)
            .ok_or_else(|| SandboxError::Internal(format!("Sandbox {sandbox_id} not found")))?;

        if !sandbox.network.enabled {
            return Err(SandboxError::NetworkDenied(
                "Networking is disabled".to_string(),
            ));
        }

        self.check_capability(sandbox_id, &SandboxCapability::NetworkOutbound)?;

        if !sandbox.network.allowed_ports.is_empty()
            && !sandbox.network.allowed_ports.contains(&port)
        {
            return Err(SandboxError::NetworkDenied(format!(
                "Port {port} not in allowed ports"
            )));
        }

        // For simplicity, just check that host is not a private IP
        if is_private_ip(host) {
            for denied_cidr in &sandbox.network.denied_cidrs {
                if cidr_matches_host(host, denied_cidr) {
                    return Err(SandboxError::NetworkDenied(format!(
                        "Host {host} matches denied CIDR {denied_cidr}"
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check syscall permission.
    pub fn check_syscall(
        &self,
        sandbox_id: &str,
        syscall: &str,
    ) -> Result<(), SandboxError> {
        let sandbox = self
            .sandboxes
            .get(sandbox_id)
            .ok_or_else(|| SandboxError::Internal(format!("Sandbox {sandbox_id} not found")))?;

        match sandbox.syscall_filter.mode {
            SyscallFilterMode::Allowlist => {
                if sandbox.syscall_filter.syscalls.contains(&syscall.to_string()) {
                    Ok(())
                } else {
                    Err(SandboxError::SyscallDenied(format!(
                        "Syscall {syscall} not in allowlist"
                    )))
                }
            }
            SyscallFilterMode::Denylist => {
                if sandbox.syscall_filter.syscalls.contains(&syscall.to_string()) {
                    Err(SandboxError::SyscallDenied(format!(
                        "Syscall {syscall} is denylisted"
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Default for SandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Simple glob-style path matching (supports * wildcard).
fn path_matches(path: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return path.starts_with(prefix);
    }
    path == pattern
}

/// Check if a host looks like a private IP.
fn is_private_ip(host: &str) -> bool {
    // Simple check for common private ranges
    host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.starts_with("172.16.")
        || host.starts_with("172.17.")
        || host.starts_with("172.18.")
        || host.starts_with("172.19.")
        || host.starts_with("172.2")
        || host.starts_with("172.3")
        || host.starts_with("127.")
        || host == "localhost"
}

/// Check if a host matches a CIDR (simplified — checks prefix).
fn cidr_matches_host(host: &str, cidr: &str) -> bool {
    let prefix = cidr.split('/').next().unwrap_or(cidr);
    // For /8, /16, /24 checks
    let prefix_parts: Vec<&str> = prefix.split('.').collect();
    let host_parts: Vec<&str> = host.split('.').collect();

    let mask_bits: u32 = cidr
        .split('/')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(32);

    let octets_to_check = (mask_bits as usize).div_ceil(8);
    for i in 0..octets_to_check.min(4).min(prefix_parts.len()).min(host_parts.len()) {
        if prefix_parts[i] != host_parts[i] {
            return false;
        }
    }
    true
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_resource_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.max_cpu_time_ms, 30_000);
        assert_eq!(limits.timeout_seconds, 60);
    }

    #[test]
    fn test_sandbox_config_new() {
        let config = SandboxConfig::new("sb-1", "agent-1");
        assert_eq!(config.id, "sb-1");
        assert_eq!(config.agent_id, "agent-1");
        assert!(config.active);
        assert!(config.capabilities.contains(&SandboxCapability::FileRead));
        assert!(config.capabilities.contains(&SandboxCapability::NetworkOutbound));
    }

    #[test]
    fn test_sandbox_config_minimal() {
        let config = SandboxConfig::minimal("sb-min", "agent-1");
        assert!(config.filesystem.read_only);
        assert!(!config.network.enabled);
        assert!(!config.capabilities.contains(&SandboxCapability::ShellExec));
        assert!(!config.capabilities.contains(&SandboxCapability::NetworkOutbound));
    }

    #[test]
    fn test_sandbox_config_validate() {
        let config = SandboxConfig::new("sb-1", "agent-1");
        assert!(config.validate().is_ok());

        let mut bad = SandboxConfig::new("sb-2", "agent-2");
        bad.resource_limits.max_memory_bytes = 0;
        assert!(matches!(
            bad.validate().unwrap_err(),
            SandboxError::InvalidConfig(_)
        ));
    }

    #[test]
    fn test_manager_create_destroy() {
        let mut mgr = SandboxManager::new();
        let config = SandboxConfig::new("sb-1", "agent-1");
        assert!(mgr.create_sandbox(config).is_ok());
        assert_eq!(mgr.list_sandboxes().len(), 1);

        assert!(mgr.destroy_sandbox("sb-1"));
        assert!(mgr.list_sandboxes().is_empty());
        assert!(!mgr.destroy_sandbox("sb-1")); // already gone
    }

    #[test]
    fn test_capability_check() {
        let mut mgr = SandboxManager::new();
        mgr.create_sandbox(SandboxConfig::new("sb-1", "agent-1"))
            .unwrap();

        // Has FileRead
        assert!(mgr.check_capability("sb-1", &SandboxCapability::FileRead).is_ok());
        // Does not have ShellExec
        assert!(matches!(
            mgr.check_capability("sb-1", &SandboxCapability::ShellExec).unwrap_err(),
            SandboxError::CapabilityDenied(_)
        ));
    }

    #[test]
    fn test_filesystem_access() {
        let mut mgr = SandboxManager::new();
        mgr.create_sandbox(SandboxConfig::new("sb-1", "agent-1"))
            .unwrap();

        // Allowed path
        assert!(mgr.check_filesystem_access("sb-1", "/tmp/sandbox/data.txt", false).is_ok());
        // Denied path
        assert!(matches!(
            mgr.check_filesystem_access("sb-1", "/etc/passwd", false).unwrap_err(),
            SandboxError::FilesystemDenied(_)
        ));
    }

    #[test]
    fn test_filesystem_read_only() {
        let mut mgr = SandboxManager::new();
        let mut config = SandboxConfig::new("sb-1", "agent-1");
        config.filesystem.read_only = true;
        config.capabilities.insert(SandboxCapability::FileWrite);
        mgr.create_sandbox(config).unwrap();

        assert!(matches!(
            mgr.check_filesystem_access("sb-1", "/tmp/sandbox/data.txt", true).unwrap_err(),
            SandboxError::FilesystemDenied(_)
        ));
    }

    #[test]
    fn test_network_denied_when_disabled() {
        let mut mgr = SandboxManager::new();
        let mut config = SandboxConfig::new("sb-1", "agent-1");
        config.network.enabled = false;
        mgr.create_sandbox(config).unwrap();

        assert!(matches!(
            mgr.check_network_access("sb-1", "example.com", 443).unwrap_err(),
            SandboxError::NetworkDenied(_)
        ));
    }

    #[test]
    fn test_network_private_ip_denied() {
        let mut mgr = SandboxManager::new();
        mgr.create_sandbox(SandboxConfig::new("sb-1", "agent-1"))
            .unwrap();

        assert!(matches!(
            mgr.check_network_access("sb-1", "10.0.0.1", 443).unwrap_err(),
            SandboxError::NetworkDenied(_)
        ));
        assert!(matches!(
            mgr.check_network_access("sb-1", "192.168.1.1", 443).unwrap_err(),
            SandboxError::NetworkDenied(_)
        ));
    }

    #[test]
    fn test_syscall_denylist() {
        let mut mgr = SandboxManager::new();
        mgr.create_sandbox(SandboxConfig::new("sb-1", "agent-1"))
            .unwrap();

        assert!(mgr.check_syscall("sb-1", "read").is_ok());
        assert!(matches!(
            mgr.check_syscall("sb-1", "ptrace").unwrap_err(),
            SandboxError::SyscallDenied(_)
        ));
        assert!(matches!(
            mgr.check_syscall("sb-1", "mount").unwrap_err(),
            SandboxError::SyscallDenied(_)
        ));
    }

    #[test]
    fn test_syscall_allowlist() {
        let mut mgr = SandboxManager::new();
        let mut config = SandboxConfig::new("sb-1", "agent-1");
        config.syscall_filter = SyscallFilter {
            mode: SyscallFilterMode::Allowlist,
            syscalls: vec!["read".to_string(), "write".to_string(), "exit".to_string()],
        };
        mgr.create_sandbox(config).unwrap();

        assert!(mgr.check_syscall("sb-1", "read").is_ok());
        assert!(mgr.check_syscall("sb-1", "write").is_ok());
        assert!(matches!(
            mgr.check_syscall("sb-1", "open").unwrap_err(),
            SandboxError::SyscallDenied(_)
        ));
    }

    #[test]
    fn test_path_matching() {
        assert!(path_matches("/tmp/sandbox/file.txt", "/tmp/sandbox/*"));
        assert!(path_matches("/etc/passwd", "/etc/passwd"));
        assert!(!path_matches("/etc/passwd", "/tmp/*"));
        assert!(path_matches("/any/path", "*"));
    }

    #[test]
    fn test_is_private_ip() {
        assert!(is_private_ip("10.0.0.1"));
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("172.16.0.1"));
        assert!(is_private_ip("127.0.0.1"));
        assert!(is_private_ip("localhost"));
        assert!(!is_private_ip("8.8.8.8"));
        assert!(!is_private_ip("example.com"));
    }

    #[test]
    fn test_sandbox_not_found() {
        let mgr = SandboxManager::new();
        assert!(matches!(
            mgr.check_capability("nonexistent", &SandboxCapability::FileRead).unwrap_err(),
            SandboxError::Internal(_)
        ));
    }
}
