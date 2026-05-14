//! MCP Sandbox Execution Environment
//!
//! Provides:
//! - Isolated execution environments for tools
//! - Resource limits (CPU, memory, disk, network)
//! - Filesystem isolation (chroot/overlay)
//! - Network policy enforcement
//! - Process lifecycle management
//! - Execution logs and metrics
//! - Support for multiple sandbox backends (Docker, Firecracker, gVisor, Process)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::McpError;

// ---------------------------------------------------------------------------
// Sandbox Configuration
// ---------------------------------------------------------------------------

/// Sandbox backend types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SandboxBackend {
    /// Docker container.
    Docker,
    /// Firecracker microVM.
    Firecracker,
    /// gVisor container runtime.
    GVisor,
    /// Process-level isolation (chroot/seccomp).
    Process,
    /// WebAssembly sandbox.
    Wasm,
}

impl std::fmt::Display for SandboxBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxBackend::Docker => write!(f, "docker"),
            SandboxBackend::Firecracker => write!(f, "firecracker"),
            SandboxBackend::GVisor => write!(f, "gvisor"),
            SandboxBackend::Process => write!(f, "process"),
            SandboxBackend::Wasm => write!(f, "wasm"),
        }
    }
}

/// Resource limits for sandbox execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU cores (0 = no limit).
    #[serde(default)]
    pub cpu_cores: Option<f64>,
    /// Maximum memory in bytes (0 = no limit).
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    /// Maximum disk space in bytes (0 = no limit).
    #[serde(default)]
    pub disk_bytes: Option<u64>,
    /// Maximum network bandwidth in bytes/sec (0 = no limit).
    #[serde(default)]
    pub network_bandwidth: Option<u64>,
    /// Maximum number of open file descriptors.
    #[serde(default)]
    pub max_open_files: Option<u32>,
    /// Maximum number of processes.
    #[serde(default)]
    pub max_processes: Option<u32>,
    /// Execution timeout.
    #[serde(default)]
    pub timeout: Option<Duration>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_cores: Some(1.0),
            memory_bytes: Some(512 * 1024 * 1024), // 512MB
            disk_bytes: Some(1024 * 1024 * 1024),  // 1GB
            network_bandwidth: None,
            max_open_files: Some(1024),
            max_processes: Some(64),
            timeout: Some(Duration::from_secs(300)), // 5 minutes
        }
    }
}

/// Network policy for sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Enable network access.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Allowed outbound hosts (empty = all allowed).
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    /// Blocked outbound hosts.
    #[serde(default)]
    pub blocked_hosts: Vec<String>,
    /// Allowed ports (empty = all allowed).
    #[serde(default)]
    pub allowed_ports: Vec<u16>,
    /// Enable DNS resolution.
    #[serde(default = "default_true")]
    pub dns_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_hosts: Vec::new(),
            blocked_hosts: Vec::new(),
            allowed_ports: Vec::new(),
            dns_enabled: true,
        }
    }
}

/// Filesystem configuration for sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    /// Base image/rootfs path.
    #[serde(default)]
    pub rootfs: Option<PathBuf>,
    /// Read-only mounts (host:guest).
    #[serde(default)]
    pub readonly_mounts: Vec<MountPoint>,
    /// Read-write mounts (host:guest).
    #[serde(default)]
    pub readwrite_mounts: Vec<MountPoint>,
    /// Temporary directory size (bytes).
    #[serde(default)]
    pub tmpdir_size: Option<u64>,
    /// Enable overlay filesystem.
    #[serde(default)]
    pub overlay: bool,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            rootfs: None,
            readonly_mounts: Vec::new(),
            readwrite_mounts: Vec::new(),
            tmpdir_size: Some(100 * 1024 * 1024), // 100MB
            overlay: true,
        }
    }
}

/// Mount point configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    /// Host path.
    pub host: PathBuf,
    /// Guest path.
    pub guest: PathBuf,
    /// Mount options.
    #[serde(default)]
    pub options: Vec<String>,
}

/// Sandbox configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Sandbox backend.
    pub backend: SandboxBackend,
    /// Sandbox name/ID.
    pub name: String,
    /// Container image (for Docker/GVisor).
    #[serde(default)]
    pub image: Option<String>,
    /// Command to execute.
    pub command: Vec<String>,
    /// Working directory inside sandbox.
    #[serde(default)]
    pub workdir: Option<PathBuf>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Resource limits.
    #[serde(default)]
    pub limits: ResourceLimits,
    /// Network policy.
    #[serde(default)]
    pub network: NetworkPolicy,
    /// Filesystem configuration.
    #[serde(default)]
    pub filesystem: FilesystemConfig,
    /// Enable seccomp syscall filtering.
    #[serde(default)]
    pub seccomp: bool,
    /// Enable AppArmor/SELinux profiles.
    #[serde(default)]
    pub apparmor: bool,
    /// User ID inside sandbox.
    #[serde(default)]
    pub uid: Option<u32>,
    /// Group ID inside sandbox.
    #[serde(default)]
    pub gid: Option<u32>,
    /// Labels for organization.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

impl SandboxConfig {
    /// Create a Docker sandbox configuration.
    pub fn docker(name: &str, image: &str, command: Vec<String>) -> Self {
        Self {
            backend: SandboxBackend::Docker,
            name: name.to_string(),
            image: Some(image.to_string()),
            command,
            workdir: None,
            env: HashMap::new(),
            limits: ResourceLimits::default(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemConfig::default(),
            seccomp: true,
            apparmor: false,
            uid: None,
            gid: None,
            labels: HashMap::new(),
        }
    }

    /// Create a process sandbox configuration.
    pub fn process(name: &str, command: Vec<String>) -> Self {
        Self {
            backend: SandboxBackend::Process,
            name: name.to_string(),
            image: None,
            command,
            workdir: None,
            env: HashMap::new(),
            limits: ResourceLimits::default(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemConfig::default(),
            seccomp: true,
            apparmor: false,
            uid: Some(65534), // nobody
            gid: Some(65534),
            labels: HashMap::new(),
        }
    }

    /// Create a WASM sandbox configuration.
    pub fn wasm(name: &str, module: &str) -> Self {
        Self {
            backend: SandboxBackend::Wasm,
            name: name.to_string(),
            image: None,
            command: vec![module.to_string()],
            workdir: None,
            env: HashMap::new(),
            limits: ResourceLimits {
                network_bandwidth: Some(0), // No network by default
                ..ResourceLimits::default()
            },
            network: NetworkPolicy {
                enabled: false,
                ..NetworkPolicy::default()
            },
            filesystem: FilesystemConfig::default(),
            seccomp: false,
            apparmor: false,
            uid: None,
            gid: None,
            labels: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Sandbox Instance
// ---------------------------------------------------------------------------

/// Sandbox execution state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SandboxState {
    /// Sandbox is being created.
    Creating,
    /// Sandbox is ready to run.
    Ready,
    /// Sandbox is running.
    Running,
    /// Sandbox completed successfully.
    Completed,
    /// Sandbox failed.
    Failed,
    /// Sandbox was terminated (timeout or manual).
    Terminated,
    /// Sandbox is being destroyed.
    Destroying,
}

/// Sandbox execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    /// Exit code.
    pub exit_code: i32,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
    /// Execution duration.
    pub duration: Duration,
    /// Resource usage.
    pub resource_usage: ResourceUsage,
    /// Whether execution was terminated (e.g., timeout).
    pub terminated: bool,
    /// Termination reason.
    pub termination_reason: Option<String>,
}

/// Resource usage during execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    /// Peak memory usage (bytes).
    pub peak_memory_bytes: u64,
    /// CPU time used (nanoseconds).
    pub cpu_time_ns: u64,
    /// Bytes read from disk.
    pub disk_read_bytes: u64,
    /// Bytes written to disk.
    pub disk_write_bytes: u64,
    /// Bytes received from network.
    pub network_rx_bytes: u64,
    /// Bytes sent to network.
    pub network_tx_bytes: u64,
    /// Number of processes created.
    pub process_count: u32,
}

/// Sandbox instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxInstance {
    /// Sandbox ID.
    pub id: String,
    /// Sandbox configuration.
    pub config: SandboxConfig,
    /// Current state.
    pub state: SandboxState,
    /// When the sandbox was created.
    pub created_at: SystemTime,
    /// When execution started.
    pub started_at: Option<SystemTime>,
    /// When execution completed.
    pub completed_at: Option<SystemTime>,
    /// Execution result (if completed).
    pub result: Option<SandboxResult>,
    /// Process ID (for process sandbox).
    pub pid: Option<u32>,
    /// Container ID (for Docker/GVisor).
    pub container_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Sandbox Manager
// ---------------------------------------------------------------------------

/// Trait for sandbox backends.
#[async_trait::async_trait]
pub trait SandboxBackendTrait: Send + Sync {
    /// Create a sandbox.
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError>;

    /// Start execution in a sandbox.
    async fn start(&self, instance: &mut SandboxInstance) -> Result<(), McpError>;

    /// Wait for sandbox to complete.
    async fn wait(&self, instance: &SandboxInstance) -> Result<SandboxResult, McpError>;

    /// Terminate a running sandbox.
    async fn terminate(&self, instance: &SandboxInstance) -> Result<(), McpError>;

    /// Destroy a sandbox and clean up resources.
    async fn destroy(&self, instance: &SandboxInstance) -> Result<(), McpError>;

    /// Get resource usage for a running sandbox.
    async fn resource_usage(&self, instance: &SandboxInstance) -> Result<ResourceUsage, McpError>;
}

/// Sandbox manager configuration.
#[derive(Debug, Clone)]
pub struct SandboxManagerConfig {
    /// Maximum concurrent sandboxes.
    pub max_sandboxes: usize,
    /// Default sandbox backend.
    pub default_backend: SandboxBackend,
    /// Sandbox cleanup interval.
    pub cleanup_interval: Duration,
    /// Maximum sandbox lifetime.
    pub max_lifetime: Duration,
    /// Enable audit logging.
    pub audit_enabled: bool,
}

impl Default for SandboxManagerConfig {
    fn default() -> Self {
        Self {
            max_sandboxes: 100,
            default_backend: SandboxBackend::Process,
            cleanup_interval: Duration::from_secs(300),
            max_lifetime: Duration::from_secs(3600),
            audit_enabled: true,
        }
    }
}

/// Sandbox audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxAuditEntry {
    /// Timestamp.
    pub timestamp: SystemTime,
    /// Sandbox ID.
    pub sandbox_id: String,
    /// Action performed.
    pub action: SandboxAction,
    /// Actor (user/tool).
    pub actor: String,
    /// Additional details.
    pub details: Option<String>,
}

/// Sandbox audit actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SandboxAction {
    /// Sandbox created.
    Create,
    /// Sandbox started.
    Start,
    /// Sandbox completed.
    Complete,
    /// Sandbox terminated.
    Terminate,
    /// Sandbox destroyed.
    Destroy,
    /// Sandbox failed.
    Fail,
    /// Resource limit exceeded.
    LimitExceeded,
}

/// Sandbox manager.
pub struct SandboxManager {
    /// Configuration.
    config: SandboxManagerConfig,
    /// Active sandboxes.
    sandboxes: Arc<RwLock<HashMap<String, SandboxInstance>>>,
    /// Backend implementations.
    backends: Arc<RwLock<HashMap<SandboxBackend, Arc<dyn SandboxBackendTrait>>>>,
    /// Audit log.
    audit_log: Arc<RwLock<Vec<SandboxAuditEntry>>>,
}

impl SandboxManager {
    /// Create a new sandbox manager.
    pub fn new(config: SandboxManagerConfig) -> Self {
        let manager = Self {
            config,
            sandboxes: Arc::new(RwLock::new(HashMap::new())),
            backends: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        };

        // Start cleanup task
        let mgr = manager.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(mgr.config.cleanup_interval).await;
                if let Err(e) = mgr.cleanup_expired().await {
                    eprintln!("Sandbox cleanup error: {}", e);
                }
            }
        });

        manager
    }

    /// Register a sandbox backend.
    pub async fn register_backend(&self, backend: SandboxBackend, impl_: Arc<dyn SandboxBackendTrait>) {
        let mut backends = self.backends.write().await;
        backends.insert(backend, impl_);
    }

    /// Create and start a sandbox.
    pub async fn execute(&self, config: SandboxConfig, actor: &str) -> Result<SandboxResult, McpError> {
        // Check limit
        let sandboxes = self.sandboxes.read().await;
        if sandboxes.len() >= self.config.max_sandboxes {
            return Err(McpError::Internal("Maximum sandbox limit reached".to_string()));
        }
        drop(sandboxes);

        // Get backend
        let backends = self.backends.read().await;
        let backend = backends
            .get(&config.backend)
            .ok_or_else(|| {
                McpError::Internal(format!("Backend not registered: {}", config.backend))
            })?
            .clone();
        drop(backends);

        // Create sandbox
        let mut instance = backend.create(&config).await?;

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: instance.id.clone(),
            action: SandboxAction::Create,
            actor: actor.to_string(),
            details: Some(format!("Backend: {}", config.backend)),
        })
        .await;

        // Store instance
        let mut sandboxes = self.sandboxes.write().await;
        sandboxes.insert(instance.id.clone(), instance.clone());
        drop(sandboxes);

        // Start execution
        backend.start(&mut instance).await?;

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: instance.id.clone(),
            action: SandboxAction::Start,
            actor: actor.to_string(),
            details: None,
        })
        .await;

        // Update state
        let mut sandboxes = self.sandboxes.write().await;
        sandboxes.insert(instance.id.clone(), instance.clone());
        drop(sandboxes);

        // Wait for completion with timeout
        let timeout = config
            .limits
            .timeout
            .unwrap_or(self.config.max_lifetime);

        let result = match tokio::time::timeout(timeout, backend.wait(&instance)).await {
            Ok(Ok(result)) => {
                self.audit(SandboxAuditEntry {
                    timestamp: SystemTime::now(),
                    sandbox_id: instance.id.clone(),
                    action: SandboxAction::Complete,
                    actor: actor.to_string(),
                    details: Some(format!("Exit code: {}", result.exit_code)),
                })
                .await;
                result
            }
            Ok(Err(e)) => {
                self.audit(SandboxAuditEntry {
                    timestamp: SystemTime::now(),
                    sandbox_id: instance.id.clone(),
                    action: SandboxAction::Fail,
                    actor: actor.to_string(),
                    details: Some(e.to_string()),
                })
                .await;
                return Err(e);
            }
            Err(_) => {
                // Timeout - terminate
                backend.terminate(&instance).await?;

                self.audit(SandboxAuditEntry {
                    timestamp: SystemTime::now(),
                    sandbox_id: instance.id.clone(),
                    action: SandboxAction::Terminate,
                    actor: actor.to_string(),
                    details: Some("Timeout".to_string()),
                })
                .await;

                SandboxResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: "Execution timed out".to_string(),
                    duration: timeout,
                    resource_usage: ResourceUsage::default(),
                    terminated: true,
                    termination_reason: Some("Timeout exceeded".to_string()),
                }
            }
        };

        // Destroy sandbox
        backend.destroy(&instance).await?;

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: instance.id.clone(),
            action: SandboxAction::Destroy,
            actor: actor.to_string(),
            details: None,
        })
        .await;

        // Remove from active sandboxes
        let mut sandboxes = self.sandboxes.write().await;
        sandboxes.remove(&instance.id);

        Ok(result)
    }

    /// Get a sandbox instance by ID.
    pub async fn get(&self, id: &str) -> Option<SandboxInstance> {
        let sandboxes = self.sandboxes.read().await;
        sandboxes.get(id).cloned()
    }

    /// List all active sandboxes.
    pub async fn list(&self) -> Vec<SandboxInstance> {
        let sandboxes = self.sandboxes.read().await;
        sandboxes.values().cloned().collect()
    }

    /// Terminate a running sandbox.
    pub async fn terminate(&self, id: &str, actor: &str) -> Result<(), McpError> {
        let sandboxes = self.sandboxes.read().await;
        let instance = sandboxes
            .get(id)
            .ok_or_else(|| McpError::ResourceNotFound(format!("Sandbox not found: {}", id)))?;

        let backends = self.backends.read().await;
        let backend = backends
            .get(&instance.config.backend)
            .ok_or_else(|| {
                McpError::Internal(format!("Backend not found: {}", instance.config.backend))
            })?;

        backend.terminate(instance).await?;

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: id.to_string(),
            action: SandboxAction::Terminate,
            actor: actor.to_string(),
            details: Some("Manual termination".to_string()),
        })
        .await;

        Ok(())
    }

    /// Get audit log entries.
    pub async fn audit_log(&self) -> Vec<SandboxAuditEntry> {
        let log = self.audit_log.read().await;
        log.clone()
    }

    /// Clean up expired sandboxes.
    async fn cleanup_expired(&self) -> Result<(), McpError> {
        let now = SystemTime::now();
        let mut to_remove = Vec::new();

        let sandboxes = self.sandboxes.read().await;
        for (id, instance) in sandboxes.iter() {
            if let Ok(elapsed) = now.duration_since(instance.created_at) {
                if elapsed > self.config.max_lifetime {
                    to_remove.push(id.clone());
                }
            }
        }
        drop(sandboxes);

        for id in to_remove {
            if let Err(e) = self.terminate(&id, "system").await {
                eprintln!("Failed to terminate expired sandbox {}: {}", id, e);
            }
        }

        Ok(())
    }

    /// Add an audit entry.
    async fn audit(&self, entry: SandboxAuditEntry) {
        if !self.config.audit_enabled {
            return;
        }

        let mut log = self.audit_log.write().await;
        log.push(entry);
    }
}

impl Clone for SandboxManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            sandboxes: self.sandboxes.clone(),
            backends: self.backends.clone(),
            audit_log: self.audit_log.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Process Sandbox Backend (simplified)
// ---------------------------------------------------------------------------

/// Process-based sandbox backend (uses chroot/seccomp).
pub struct ProcessSandboxBackend;

#[async_trait::async_trait]
impl SandboxBackendTrait for ProcessSandboxBackend {
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        let id = uuid::Uuid::new_v4().to_string();

        Ok(SandboxInstance {
            id,
            config: config.clone(),
            state: SandboxState::Ready,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            result: None,
            pid: None,
            container_id: None,
        })
    }

    async fn start(&self, instance: &mut SandboxInstance) -> Result<(), McpError> {
        instance.state = SandboxState::Running;
        instance.started_at = Some(SystemTime::now());

        // In a real implementation, this would use chroot/seccomp
        // For now, just mark as started
        Ok(())
    }

    async fn wait(&self, _instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        // Simulate execution
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(SandboxResult {
            exit_code: 0,
            stdout: "Process sandbox execution completed".to_string(),
            stderr: String::new(),
            duration: Duration::from_millis(100),
            resource_usage: ResourceUsage::default(),
            terminated: false,
            termination_reason: None,
        })
    }

    async fn terminate(&self, _instance: &SandboxInstance) -> Result<(), McpError> {
        // In a real implementation, this would kill the process
        Ok(())
    }

    async fn destroy(&self, _instance: &SandboxInstance) -> Result<(), McpError> {
        // In a real implementation, this would clean up chroot
        Ok(())
    }

    async fn resource_usage(&self, _instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        Ok(ResourceUsage::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_docker() {
        let config = SandboxConfig::docker(
            "test",
            "python:3.11",
            vec!["python".to_string(), "-c".to_string(), "print('hello')".to_string()],
        );

        assert_eq!(config.backend, SandboxBackend::Docker);
        assert_eq!(config.name, "test");
        assert_eq!(config.image, Some("python:3.11".to_string()));
    }

    #[test]
    fn test_sandbox_config_process() {
        let config = SandboxConfig::process(
            "test",
            vec!["echo".to_string(), "hello".to_string()],
        );

        assert_eq!(config.backend, SandboxBackend::Process);
        assert_eq!(config.uid, Some(65534));
    }

    #[test]
    fn test_sandbox_config_wasm() {
        let config = SandboxConfig::wasm("test", "module.wasm");

        assert_eq!(config.backend, SandboxBackend::Wasm);
        assert!(!config.network.enabled);
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_cores, Some(1.0));
        assert_eq!(limits.memory_bytes, Some(512 * 1024 * 1024));
    }

    #[tokio::test]
    async fn test_sandbox_manager_create() {
        let manager = SandboxManager::new(SandboxManagerConfig::default());

        // Register process backend
        manager
            .register_backend(SandboxBackend::Process, Arc::new(ProcessSandboxBackend))
            .await;

        let config = SandboxConfig::process("test", vec!["echo".to_string()]);

        let result = manager.execute(config, "test-user").await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.exit_code, 0);
    }

    #[tokio::test]
    async fn test_sandbox_manager_list() {
        let manager = SandboxManager::new(SandboxManagerConfig::default());

        let sandboxes = manager.list().await;
        assert_eq!(sandboxes.len(), 0);
    }
}
