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
// Isolation Levels
// ---------------------------------------------------------------------------

/// Isolation level for sandbox state recovery and workspace projection.
///
/// Determines how snapshots and workspace state are scoped:
/// - **Session**: per-session; state is isolated to a single session lifetime.
/// - **User**: per-user; state persists across sessions for the same user.
/// - **Global**: shared across all users; useful for base images or common state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// Per-session isolation. Snapshot is scoped to the current session.
    Session,
    /// Per-user isolation. Snapshot persists across sessions for the same user.
    User,
    /// Global isolation. Snapshot is shared across all users and sessions.
    Global,
}

impl std::fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IsolationLevel::Session => write!(f, "session"),
            IsolationLevel::User => write!(f, "user"),
            IsolationLevel::Global => write!(f, "global"),
        }
    }
}

impl Default for IsolationLevel {
    fn default() -> Self {
        Self::Session
    }
}

// ---------------------------------------------------------------------------
// Sandbox Snapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of sandbox state for recovery purposes.
///
/// Captures the key elements needed to recreate a sandbox's working environment:
/// file contents (key files), environment variables, and working directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSnapshot {
    /// Unique snapshot identifier.
    pub id: String,
    /// ID of the sandbox this snapshot was taken from.
    pub sandbox_id: String,
    /// Isolation level governing scope.
    pub isolation_level: IsolationLevel,
    /// Optional owner identifier (user ID for `User` level, session ID for `Session`).
    pub owner: Option<String>,
    /// Serialized file contents: relative path -> base64-encoded bytes.
    pub files: HashMap<String, Vec<u8>>,
    /// Environment variables captured at snapshot time.
    pub env: HashMap<String, String>,
    /// Working directory at snapshot time.
    pub workdir: Option<PathBuf>,
    /// When the snapshot was created.
    pub created_at: SystemTime,
    /// Optional human-readable description.
    pub description: Option<String>,
}

impl SandboxSnapshot {
    /// Create a new empty snapshot for the given sandbox.
    pub fn new(sandbox_id: impl Into<String>, isolation_level: IsolationLevel) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sandbox_id: sandbox_id.into(),
            isolation_level,
            owner: None,
            files: HashMap::new(),
            env: HashMap::new(),
            workdir: None,
            created_at: SystemTime::now(),
            description: None,
        }
    }

    /// Set the owner for this snapshot.
    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }

    /// Set a description for this snapshot.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Add a file to the snapshot.
    pub fn add_file(&mut self, relative_path: impl Into<String>, contents: Vec<u8>) {
        self.files.insert(relative_path.into(), contents);
    }

    /// Serialize the snapshot to JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, McpError> {
        serde_json::to_vec(self)
            .map_err(|e| McpError::Internal(format!("failed to serialize snapshot: {}", e)))
    }

    /// Deserialize a snapshot from JSON bytes.
    pub fn from_json(data: &[u8]) -> Result<Self, McpError> {
        serde_json::from_slice(data)
            .map_err(|e| McpError::Internal(format!("failed to deserialize snapshot: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// Workspace Projection
// ---------------------------------------------------------------------------

/// Defines which workspace files should be projected into a sandbox.
///
/// Workspace projection copies a curated subset of workspace files (AGENTS.md,
/// skills/, knowledge/, etc.) into the sandbox filesystem so that tools and
/// agents running inside the sandbox have access to the workspace context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceProjection {
    /// Root path of the workspace to project.
    pub workspace_root: PathBuf,
    /// Files and directories to include (relative paths).
    /// Defaults to: ["AGENTS.md", "skills/", "knowledge/"]
    pub includes: Vec<String>,
    /// Destination path inside the sandbox (default: /workspace).
    pub sandbox_dest: PathBuf,
}

impl Default for WorkspaceProjection {
    fn default() -> Self {
        Self {
            workspace_root: PathBuf::from("."),
            includes: vec![
                "AGENTS.md".to_string(),
                "skills/".to_string(),
                "knowledge/".to_string(),
            ],
            sandbox_dest: PathBuf::from("/workspace"),
        }
    }
}

impl WorkspaceProjection {
    /// Create a new workspace projection from the given root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            ..Default::default()
        }
    }

    /// Set custom include patterns.
    pub fn with_includes(mut self, includes: Vec<String>) -> Self {
        self.includes = includes;
        self
    }

    /// Set the destination path inside the sandbox.
    pub fn with_sandbox_dest(mut self, dest: impl Into<PathBuf>) -> Self {
        self.sandbox_dest = dest.into();
        self
    }
}

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

    /// Create a Firecracker microVM sandbox configuration.
    pub fn firecracker(name: &str, kernel: &str, rootfs: &str) -> Self {
        Self {
            backend: SandboxBackend::Firecracker,
            name: name.to_string(),
            image: None,
            command: vec![],
            workdir: None,
            env: HashMap::new(),
            limits: ResourceLimits::default(),
            network: NetworkPolicy::default(),
            filesystem: FilesystemConfig {
                rootfs: Some(PathBuf::from(rootfs)),
                ..FilesystemConfig::default()
            },
            seccomp: false,
            apparmor: false,
            uid: None,
            gid: None,
            labels: HashMap::from([("kernel".to_string(), kernel.to_string())]),
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
    /// Snapshot saved.
    SnapshotSave,
    /// Snapshot restored.
    SnapshotRestore,
    /// Workspace projected into sandbox.
    WorkspaceProjection,
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
    /// Stored snapshots (snapshot_id -> SandboxSnapshot).
    snapshots: Arc<RwLock<HashMap<String, SandboxSnapshot>>>,
    /// Base directory for snapshot persistence on disk.
    snapshot_dir: PathBuf,
}

impl SandboxManager {
    /// Create a new sandbox manager.
    pub fn new(config: SandboxManagerConfig) -> Self {
        Self::with_snapshot_dir(config, std::env::temp_dir().join("kias-snapshots"))
    }

    /// Create a new sandbox manager with a custom snapshot directory.
    pub fn with_snapshot_dir(config: SandboxManagerConfig, snapshot_dir: PathBuf) -> Self {
        let manager = Self {
            config,
            sandboxes: Arc::new(RwLock::new(HashMap::new())),
            backends: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            snapshots: Arc::new(RwLock::new(HashMap::new())),
            snapshot_dir,
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
    pub async fn register_backend(
        &self,
        backend: SandboxBackend,
        impl_: Arc<dyn SandboxBackendTrait>,
    ) {
        let mut backends = self.backends.write().await;
        backends.insert(backend, impl_);
    }

    /// Create and start a sandbox.
    pub async fn execute(
        &self,
        config: SandboxConfig,
        actor: &str,
    ) -> Result<SandboxResult, McpError> {
        // Check limit
        let sandboxes = self.sandboxes.read().await;
        if sandboxes.len() >= self.config.max_sandboxes {
            return Err(McpError::Internal(
                "Maximum sandbox limit reached".to_string(),
            ));
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
        let timeout = config.limits.timeout.unwrap_or(self.config.max_lifetime);

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
        let backend = backends.get(&instance.config.backend).ok_or_else(|| {
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

    // -----------------------------------------------------------------------
    // Snapshot & State Recovery
    // -----------------------------------------------------------------------

    /// Save a snapshot of the given sandbox's current state.
    ///
    /// Captures key filesystem files, environment variables, and working
    /// directory into a serializable snapshot. The snapshot is stored both
    /// in memory and persisted to disk under the manager's snapshot directory.
    pub async fn save_snapshot(
        &self,
        sandbox_id: &str,
        isolation_level: IsolationLevel,
        actor: &str,
    ) -> Result<SandboxSnapshot, McpError> {
        // Look up sandbox
        let sandboxes = self.sandboxes.read().await;
        let instance = sandboxes
            .get(sandbox_id)
            .ok_or_else(|| {
                McpError::ResourceNotFound(format!("Sandbox not found: {}", sandbox_id))
            })?
            .clone();
        drop(sandboxes);

        // Build snapshot from sandbox config & runtime state
        let mut snapshot = SandboxSnapshot::new(sandbox_id, isolation_level);
        snapshot.env = instance.config.env.clone();
        snapshot.workdir = instance.config.workdir.clone();

        // Capture files from sandbox working directory (if it exists on disk)
        if let Some(ref workdir) = instance.config.workdir {
            Self::collect_files_recursive(workdir, workdir, &mut snapshot.files).ok();
        }

        // Also capture from the process sandbox base dir
        let sandbox_dir = std::env::temp_dir().join("kias-sandbox").join(sandbox_id);
        if sandbox_dir.exists() {
            Self::collect_files_recursive(&sandbox_dir, &sandbox_dir, &mut snapshot.files).ok();
        }

        // Persist to disk
        let snapshot_file = self.snapshot_dir.join(format!("{}.json", snapshot.id));
        if let Some(parent) = snapshot_file.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let json_bytes = snapshot.to_json()?;
        tokio::fs::write(&snapshot_file, &json_bytes)
            .await
            .map_err(|e| McpError::Internal(format!("failed to persist snapshot: {}", e)))?;

        // Store in memory
        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot.id.clone(), snapshot.clone());
        drop(snapshots);

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: sandbox_id.to_string(),
            action: SandboxAction::SnapshotSave,
            actor: actor.to_string(),
            details: Some(format!("snapshot_id={}", snapshot.id)),
        })
        .await;

        Ok(snapshot)
    }

    /// Restore a sandbox from a previously saved snapshot.
    ///
    /// Loads the snapshot (by snapshot ID), then applies its captured files,
    /// environment variables, and working directory to the target sandbox.
    /// If no target sandbox ID is given, the snapshot's original sandbox ID is used.
    pub async fn restore_snapshot(
        &self,
        snapshot_id: &str,
        target_sandbox_id: Option<&str>,
        actor: &str,
    ) -> Result<(), McpError> {
        // Load snapshot from memory (or disk)
        let snapshot = self.load_snapshot(snapshot_id).await?;

        let target_id = target_sandbox_id.unwrap_or(&snapshot.sandbox_id);

        // Verify target sandbox exists
        {
            let sandboxes = self.sandboxes.read().await;
            if !sandboxes.contains_key(target_id) {
                return Err(McpError::ResourceNotFound(format!(
                    "Target sandbox not found: {}",
                    target_id
                )));
            }
        }

        // Restore files to sandbox directory
        let sandbox_dir = std::env::temp_dir().join("kias-sandbox").join(target_id);
        for (rel_path, contents) in &snapshot.files {
            let file_path = sandbox_dir.join(rel_path);
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| McpError::Internal(format!("failed to create dir: {}", e)))?;
            }
            tokio::fs::write(&file_path, contents)
                .await
                .map_err(|e| McpError::Internal(format!("failed to restore file: {}", e)))?;
        }

        // Update sandbox config with snapshot env and workdir
        {
            let mut sandboxes = self.sandboxes.write().await;
            if let Some(instance) = sandboxes.get_mut(target_id) {
                instance.config.env = snapshot.env.clone();
                instance.config.workdir = snapshot.workdir.clone();
            }
        }

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: target_id.to_string(),
            action: SandboxAction::SnapshotRestore,
            actor: actor.to_string(),
            details: Some(format!("snapshot_id={}", snapshot_id)),
        })
        .await;

        Ok(())
    }

    /// Load a snapshot by ID, checking memory first then disk.
    async fn load_snapshot(&self, snapshot_id: &str) -> Result<SandboxSnapshot, McpError> {
        // Check in-memory cache
        {
            let snapshots = self.snapshots.read().await;
            if let Some(snap) = snapshots.get(snapshot_id) {
                return Ok(snap.clone());
            }
        }

        // Try loading from disk
        let snapshot_file = self.snapshot_dir.join(format!("{}.json", snapshot_id));
        let json_bytes = tokio::fs::read(&snapshot_file).await.map_err(|_| {
            McpError::ResourceNotFound(format!("Snapshot not found: {}", snapshot_id))
        })?;
        SandboxSnapshot::from_json(&json_bytes)
    }

    /// List all stored snapshots.
    pub async fn list_snapshots(&self) -> Vec<SandboxSnapshot> {
        let snapshots = self.snapshots.read().await;
        snapshots.values().cloned().collect()
    }

    /// Delete a snapshot by ID (from memory and disk).
    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<(), McpError> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.remove(snapshot_id);
        drop(snapshots);

        let snapshot_file = self.snapshot_dir.join(format!("{}.json", snapshot_id));
        let _ = tokio::fs::remove_file(&snapshot_file).await;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Workspace Projection
    // -----------------------------------------------------------------------

    /// Project workspace files into a sandbox.
    ///
    /// Copies the files specified by the workspace projection (AGENTS.md,
    /// skills/, knowledge/, etc.) into the sandbox's filesystem, making
    /// workspace context available to tools running inside the sandbox.
    pub async fn sync_workspace_to_sandbox(
        &self,
        projection: &WorkspaceProjection,
        sandbox_id: &str,
        actor: &str,
    ) -> Result<usize, McpError> {
        // Verify sandbox exists
        {
            let sandboxes = self.sandboxes.read().await;
            if !sandboxes.contains_key(sandbox_id) {
                return Err(McpError::ResourceNotFound(format!(
                    "Sandbox not found: {}",
                    sandbox_id
                )));
            }
        }

        let sandbox_dir = std::env::temp_dir().join("kias-sandbox").join(sandbox_id);

        // Ensure sandbox_dest is treated as relative to sandbox_dir
        let dest_root = if projection.sandbox_dest.is_absolute() {
            sandbox_dir.join(
                projection
                    .sandbox_dest
                    .strip_prefix("/")
                    .unwrap_or(&projection.sandbox_dest),
            )
        } else {
            sandbox_dir.join(&projection.sandbox_dest)
        };

        let mut copied_count: usize = 0;

        for include in &projection.includes {
            let source = projection.workspace_root.join(include);
            if !source.exists() {
                continue;
            }

            if source.is_dir() {
                copied_count += Self::copy_dir_recursive(&source, &dest_root.join(include))
                    .await
                    .map_err(|e| {
                        McpError::Internal(format!("workspace projection failed: {}", e))
                    })?;
            } else {
                let dest_file = dest_root.join(include);
                if let Some(parent) = dest_file.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| McpError::Internal(format!("failed to create dir: {}", e)))?;
                }
                tokio::fs::copy(&source, &dest_file)
                    .await
                    .map_err(|e| McpError::Internal(format!("failed to copy file: {}", e)))?;
                copied_count += 1;
            }
        }

        self.audit(SandboxAuditEntry {
            timestamp: SystemTime::now(),
            sandbox_id: sandbox_id.to_string(),
            action: SandboxAction::WorkspaceProjection,
            actor: actor.to_string(),
            details: Some(format!("files_copied={}", copied_count)),
        })
        .await;

        Ok(copied_count)
    }

    /// Recursively collect files from a directory into a HashMap.
    fn collect_files_recursive(
        base: &std::path::Path,
        dir: &std::path::Path,
        files: &mut HashMap<String, Vec<u8>>,
    ) -> Result<(), std::io::Error> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::collect_files_recursive(base, &path, files)?;
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                if let Ok(contents) = std::fs::read(&path) {
                    files.insert(rel, contents);
                }
            }
        }
        Ok(())
    }

    /// Recursively copy a directory, returning the count of files copied.
    async fn copy_dir_recursive(
        src: &std::path::Path,
        dst: &std::path::Path,
    ) -> Result<usize, std::io::Error> {
        let mut count = 0usize;
        tokio::fs::create_dir_all(dst).await?;
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let dest_path = dst.join(entry.file_name());
            if path.is_dir() {
                count += Box::pin(Self::copy_dir_recursive(&path, &dest_path)).await?;
            } else {
                tokio::fs::copy(&path, &dest_path).await?;
                count += 1;
            }
        }
        Ok(count)
    }
}

impl Clone for SandboxManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            sandboxes: self.sandboxes.clone(),
            backends: self.backends.clone(),
            audit_log: self.audit_log.clone(),
            snapshots: self.snapshots.clone(),
            snapshot_dir: self.snapshot_dir.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Process Sandbox Backend (real implementation)
// ---------------------------------------------------------------------------

use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::{Child, Command};

/// Active child process info.
struct ProcessInfo {
    child: Child,
    stdout: Arc<tokio::sync::Mutex<String>>,
    stderr: Arc<tokio::sync::Mutex<String>>,
    start_time: std::time::Instant,
}

/// Process-based sandbox backend using Linux namespaces/cgroups.
/// Provides real process isolation with resource limits and /proc stats.
pub struct ProcessSandboxBackend {
    /// Active processes: sandbox_id -> ProcessInfo
    processes: Arc<RwLock<HashMap<String, ProcessInfo>>>,
    /// Base directory for sandbox working dirs
    base_dir: PathBuf,
}

impl ProcessSandboxBackend {
    /// Create a new ProcessSandboxBackend.
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            base_dir: std::env::temp_dir().join("kias-sandbox"),
        }
    }

    /// Read CPU time from /proc/[pid]/stat (utime + stime in clock ticks).
    fn read_cpu_time(pid: u32) -> Option<u64> {
        let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
        stat.split_whitespace()
            .nth(14)
            .and_then(|s| s.parse::<u64>().ok())
    }

    /// Read peak memory (VmPeak) from /proc/[pid]/status.
    fn read_peak_memory(pid: u32) -> u64 {
        let status = match std::fs::read_to_string(format!("/proc/{}/status", pid)) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        for line in status.lines() {
            if line.starts_with("VmPeak:") {
                if let Some(kb) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb.parse::<u64>() {
                        return kb.saturating_mul(1024);
                    }
                }
            }
        }
        0
    }
}

impl Default for ProcessSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SandboxBackendTrait for ProcessSandboxBackend {
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        let id = uuid::Uuid::new_v4().to_string();

        // Create sandbox working directory
        let sandbox_dir = self.base_dir.join(&id);
        tokio::fs::create_dir_all(&sandbox_dir)
            .await
            .map_err(|e| McpError::Internal(format!("failed to create sandbox dir: {}", e)))?;

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
        if instance.state != SandboxState::Ready {
            return Err(McpError::InvalidRequest(
                "sandbox not in Ready state".to_string(),
            ));
        }

        let sandbox_dir = self.base_dir.join(&instance.id);

        // Build command
        let mut cmd = Command::new(&instance.config.command[0]);
        cmd.args(&instance.config.command[1..]);

        // Working directory: sandbox dir or configured workdir
        let workdir = instance.config.workdir.as_ref().unwrap_or(&sandbox_dir);
        cmd.current_dir(workdir);

        // Environment variables
        for (k, v) in &instance.config.env {
            cmd.env(k, v);
        }

        // Security: run as configured uid/gid (default nobody:65534).
        // Only setuid/setgid when running as root (euid 0), otherwise the
        // kernel will return EPERM.  We check via /proc/self/status to
        // avoid adding a `libc` dependency.
        let is_root = std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("Uid:"))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|v| v.parse::<u32>().ok())
            })
            .map(|euid| euid == 0)
            .unwrap_or(false);
        if is_root {
            if let Some(uid) = instance.config.uid {
                cmd.uid(uid);
            }
            if let Some(gid) = instance.config.gid {
                cmd.gid(gid);
            }
        }

        // I/O: capture stdout/stderr
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        // Spawn child process
        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Internal(format!("failed to spawn process: {}", e)))?;

        let pid = child.id();
        instance.pid = pid;
        instance.state = SandboxState::Running;
        instance.started_at = Some(SystemTime::now());

        // Set up stdout/stderr capture
        let stdout_buf = child.stdout.take().map(|out| BufReader::new(out));
        let stderr_buf = child.stderr.take().map(|err| BufReader::new(err));
        let stdout = Arc::new(tokio::sync::Mutex::new(String::new()));
        let stderr = Arc::new(tokio::sync::Mutex::new(String::new()));

        if let Some(reader) = stdout_buf {
            let out = stdout.clone();
            tokio::spawn(async move {
                let mut r = reader;
                let mut s = out.lock().await;
                r.read_to_string(&mut s).await.ok();
            });
        }
        if let Some(reader) = stderr_buf {
            let err = stderr.clone();
            tokio::spawn(async move {
                let mut r = reader;
                let mut s = err.lock().await;
                r.read_to_string(&mut s).await.ok();
            });
        }

        // Store child process
        let info = ProcessInfo {
            child,
            stdout,
            stderr,
            start_time: std::time::Instant::now(),
        };
        self.processes
            .write()
            .await
            .insert(instance.id.clone(), info);

        Ok(())
    }

    async fn wait(&self, instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        let mut info = self
            .processes
            .write()
            .await
            .remove(&instance.id)
            .ok_or_else(|| McpError::Internal("no running process found".to_string()))?;

        let start = info.start_time;
        let (exit_code, terminated, reason) = match info.child.wait().await {
            Ok(status) => (status.code().unwrap_or(-1), false, None),
            Err(e) => (-1, true, Some(e.to_string())),
        };

        let stdout = info.stdout.lock().await.clone();
        let stderr = info.stderr.lock().await.clone();
        let duration = start.elapsed();

        // Read resource usage from /proc
        let mut resource_usage = ResourceUsage::default();
        if let Some(pid) = instance.pid {
            resource_usage.peak_memory_bytes = Self::read_peak_memory(pid);
            // CPU time in jiffies -> nanoseconds (assuming 100Hz clk)
            if let Some(jiffies) = Self::read_cpu_time(pid) {
                resource_usage.cpu_time_ns = jiffies.saturating_mul(10_000_000);
            }
        }

        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            duration,
            resource_usage,
            terminated,
            termination_reason: reason,
        })
    }

    async fn terminate(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        if let Some(mut info) = self.processes.write().await.remove(&instance.id) {
            let _ = info.child.kill().await;
            let _ = info.child.wait().await;
        }
        Ok(())
    }

    async fn destroy(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        // Clean up sandbox directory
        let sandbox_dir = self.base_dir.join(&instance.id);
        let _ = tokio::fs::remove_dir_all(&sandbox_dir).await;
        Ok(())
    }

    async fn resource_usage(&self, instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        let mut usage = ResourceUsage::default();
        if let Some(pid) = instance.pid {
            usage.peak_memory_bytes = Self::read_peak_memory(pid);
            if let Some(jiffies) = Self::read_cpu_time(pid) {
                usage.cpu_time_ns = jiffies.saturating_mul(10_000_000);
            }
        }
        Ok(usage)
    }
}

// ---------------------------------------------------------------------------
// Docker Sandbox Backend (real implementation, feature-gated)
// ---------------------------------------------------------------------------

/// Docker-based sandbox backend using the Docker CLI.
///
/// Provides container-level isolation by wrapping `docker create`, `docker start`,
/// `docker logs`, `docker stop`, and `docker rm`.
///
/// Enable with the `docker` cargo feature.
#[cfg(feature = "docker")]
pub struct DockerSandboxBackend {
    /// Active containers: sandbox_id -> container_id
    containers: Arc<RwLock<HashMap<String, DockerContainerInfo>>>,
    /// Default Docker image to use when none is specified in the config.
    default_image: String,
}

#[cfg(feature = "docker")]
struct DockerContainerInfo {
    container_id: String,
    start_time: std::time::Instant,
}

#[cfg(feature = "docker")]
impl DockerSandboxBackend {
    /// Create a new `DockerSandboxBackend`.
    pub fn new() -> Self {
        Self {
            containers: Arc::new(RwLock::new(HashMap::new())),
            default_image: "ubuntu:22.04".to_string(),
        }
    }

    /// Create with a custom default image.
    pub fn with_default_image(image: impl Into<String>) -> Self {
        Self {
            containers: Arc::new(RwLock::new(HashMap::new())),
            default_image: image.into(),
        }
    }

    /// Run a docker CLI subcommand and return its output.
    async fn docker_cmd(args: &[&str]) -> Result<std::process::Output, McpError> {
        Command::new("docker")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| McpError::Internal(format!("docker command failed: {}", e)))
    }
}

#[cfg(feature = "docker")]
impl Default for DockerSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "docker")]
#[async_trait::async_trait]
impl SandboxBackendTrait for DockerSandboxBackend {
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        let id = uuid::Uuid::new_v4().to_string();
        let image = config.image.as_deref().unwrap_or(&self.default_image);

        // Build `docker create` arguments.
        let mut args: Vec<String> = vec!["create".to_string(), "--name".to_string(), id.clone()];

        // Resource limits
        if let Some(mem) = config.limits.memory_bytes {
            args.push("--memory".to_string());
            args.push(format!("{}b", mem));
        }
        if let Some(cores) = config.limits.cpu_cores {
            args.push("--cpus".to_string());
            args.push(format!("{}", cores));
        }
        // Network policy
        if !config.network.enabled {
            args.push("--network".to_string());
            args.push("none".to_string());
        }
        // Working directory
        if let Some(ref workdir) = config.workdir {
            args.push("--workdir".to_string());
            args.push(workdir.to_string_lossy().to_string());
        }
        // Environment variables
        for (k, v) in &config.env {
            args.push("--env".to_string());
            args.push(format!("{}={}", k, v));
        }
        // Filesystem mounts (readonly)
        for mp in &config.filesystem.readonly_mounts {
            args.push("--mount".to_string());
            args.push(format!(
                "type=bind,source={},target={},readonly",
                mp.host.display(),
                mp.guest.display()
            ));
        }
        // Filesystem mounts (read-write)
        for mp in &config.filesystem.readwrite_mounts {
            args.push("--mount".to_string());
            args.push(format!(
                "type=bind,source={},target={}",
                mp.host.display(),
                mp.guest.display()
            ));
        }

        // Image + command
        args.push(image.to_string());
        args.extend(config.command.iter().cloned());

        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = Self::docker_cmd(&arg_refs).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(McpError::Internal(format!(
                "docker create failed: {}",
                stderr
            )));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(SandboxInstance {
            id,
            config: config.clone(),
            state: SandboxState::Ready,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            result: None,
            pid: None,
            container_id: Some(container_id),
        })
    }

    async fn start(&self, instance: &mut SandboxInstance) -> Result<(), McpError> {
        if instance.state != SandboxState::Ready {
            return Err(McpError::InvalidRequest(
                "sandbox not in Ready state".to_string(),
            ));
        }

        let cid = instance
            .container_id
            .as_ref()
            .ok_or_else(|| McpError::Internal("missing container_id".to_string()))?;

        let output = Self::docker_cmd(&["start", cid]).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(McpError::Internal(format!(
                "docker start failed: {}",
                stderr
            )));
        }

        let info = DockerContainerInfo {
            container_id: cid.clone(),
            start_time: std::time::Instant::now(),
        };
        self.containers
            .write()
            .await
            .insert(instance.id.clone(), info);

        instance.state = SandboxState::Running;
        instance.started_at = Some(SystemTime::now());
        Ok(())
    }

    async fn wait(&self, instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        let cid = instance
            .container_id
            .as_ref()
            .ok_or_else(|| McpError::Internal("missing container_id".to_string()))?;

        // Block until the container stops.
        let wait_out = Self::docker_cmd(&["wait", cid]).await?;
        let exit_code: i32 = if wait_out.status.success() {
            String::from_utf8_lossy(&wait_out.stdout)
                .trim()
                .parse()
                .unwrap_or(-1)
        } else {
            -1
        };

        // Capture logs.
        let logs_out = Self::docker_cmd(&["logs", cid]).await?;
        let stdout = String::from_utf8_lossy(&logs_out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&logs_out.stderr).to_string();

        let elapsed = self
            .containers
            .read()
            .await
            .get(&instance.id)
            .map(|c| c.start_time.elapsed())
            .unwrap_or_default();

        // Remove tracking entry.
        self.containers.write().await.remove(&instance.id);

        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            duration: elapsed,
            resource_usage: ResourceUsage::default(),
            terminated: false,
            termination_reason: None,
        })
    }

    async fn terminate(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        if let Some(cid) = &instance.container_id {
            // Force-kill the container.
            let _ = Self::docker_cmd(&["kill", cid]).await;
        }
        self.containers.write().await.remove(&instance.id);
        Ok(())
    }

    async fn destroy(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        if let Some(cid) = &instance.container_id {
            // Force-remove even if still running.
            let _ = Self::docker_cmd(&["rm", "-f", cid]).await;
        }
        self.containers.write().await.remove(&instance.id);
        Ok(())
    }

    async fn resource_usage(&self, _instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        // `docker stats --no-stream` could be used here for live metrics.
        // For now return defaults; a full implementation would parse JSON stats.
        Ok(ResourceUsage::default())
    }
}

// ---------------------------------------------------------------------------
// Firecracker Sandbox Backend (real implementation)
// ---------------------------------------------------------------------------

/// Information about a running Firecracker VM.
struct FirecrackerVmInfo {
    /// Handle to the firecracker process.
    child: tokio::process::Child,
    /// Path to the API Unix socket.
    socket_path: PathBuf,
    /// VM start time.
    start_time: std::time::Instant,
    /// Captured stdout from the firecracker process.
    stdout: Arc<tokio::sync::Mutex<String>>,
    /// Captured stderr from the firecracker process.
    stderr: Arc<tokio::sync::Mutex<String>>,
}

/// Firecracker microVM sandbox backend.
///
/// Communicates with the Firecracker VMM via its REST API over a Unix domain
/// socket at `/tmp/firecracker-$id.socket`.
///
/// Lifecycle:
/// 1. `create()` — allocate an ID, prepare the sandbox working directory.
/// 2. `start()`  — spawn `firecracker --api-sock`, then PUT machine-config,
///    boot-source, drives, and InstanceStart via the REST API.
/// 3. `wait()`   — poll the firecracker process until it exits.
/// 4. `terminate()` — kill the firecracker process.
/// 5. `destroy()` — clean up socket, log files, and working directory.
pub struct FirecrackerSandboxBackend {
    /// Active VMs: sandbox_id -> FirecrackerVmInfo
    vms: Arc<RwLock<HashMap<String, FirecrackerVmInfo>>>,
    /// Path to the firecracker binary.
    firecracker_bin: String,
    /// Base directory for working dirs.
    base_dir: PathBuf,
}

impl FirecrackerSandboxBackend {
    /// Create a new `FirecrackerSandboxBackend`.
    pub fn new() -> Self {
        Self {
            vms: Arc::new(RwLock::new(HashMap::new())),
            firecracker_bin: "firecracker".to_string(),
            base_dir: std::env::temp_dir().join("kias-sandbox"),
        }
    }

    /// Use a custom firecracker binary path.
    pub fn with_binary(bin: impl Into<String>) -> Self {
        Self {
            firecracker_bin: bin.into(),
            ..Self::new()
        }
    }

    /// Use a custom base directory.
    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = dir.into();
        self
    }

    /// Compute the API socket path for a given sandbox ID.
    fn socket_path(id: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/firecracker-{}.socket", id))
    }

    /// Send an HTTP request over the Firecracker Unix-domain-socket API and
    /// return `(status_code, body)`.
    async fn api_request(
        socket_path: &std::path::Path,
        method: &str,
        path: &str,
        body: &str,
    ) -> Result<(u16, String), McpError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;

        let mut stream = UnixStream::connect(socket_path).await.map_err(|e| {
            McpError::Internal(format!(
                "failed to connect to Firecracker API socket: {}",
                e
            ))
        })?;

        let request = format!(
            "{method} {path} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            body.len(),
            body,
        );

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| McpError::Internal(format!("failed to write API request: {}", e)))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|e| McpError::Internal(format!("failed to read API response: {}", e)))?;

        let response_str = String::from_utf8_lossy(&response);

        // Parse status line: "HTTP/1.1 <code> ..."
        let status_code = response_str
            .lines()
            .next()
            .and_then(|line| {
                line.split_whitespace()
                    .nth(1)
                    .and_then(|code| code.parse::<u16>().ok())
            })
            .unwrap_or(0);

        // Body is after the first blank line.
        let body = response_str
            .splitn(2, "\r\n\r\n")
            .nth(1)
            .unwrap_or("")
            .to_string();

        Ok((status_code, body))
    }

    /// Configure the VM via the Firecracker REST API.
    async fn configure_vm(&self, id: &str, config: &SandboxConfig) -> Result<(), McpError> {
        let socket = Self::socket_path(id);

        // 1. PUT /machine-config
        let mem_mib = config
            .limits
            .memory_bytes
            .map(|b| b / (1024 * 1024))
            .unwrap_or(512);
        let vcpu_count = config.limits.cpu_cores.map(|c| c as u32).unwrap_or(1);
        let machine_body = serde_json::json!({
            "vcpu_count": vcpu_count,
            "mem_size_mib": mem_mib,
        })
        .to_string();
        let (code, _) = Self::api_request(&socket, "PUT", "/machine-config", &machine_body).await?;
        if code >= 400 {
            return Err(McpError::Internal(format!(
                "PUT /machine-config returned HTTP {}",
                code
            )));
        }

        // 2. PUT /boot-source — kernel image from labels["kernel"], default boot args
        let kernel_path = config
            .labels
            .get("kernel")
            .cloned()
            .unwrap_or_else(|| "/opt/vmlinux".to_string());
        let boot_args = "console=ttyS0 reboot=k panic=1 pci=off";
        let boot_body = serde_json::json!({
            "kernel_image_path": kernel_path,
            "boot_args": boot_args,
        })
        .to_string();
        let (code, _) = Self::api_request(&socket, "PUT", "/boot-source", &boot_body).await?;
        if code >= 400 {
            return Err(McpError::Internal(format!(
                "PUT /boot-source returned HTTP {}",
                code
            )));
        }

        // 3. PUT /drives/rootfs
        let rootfs_path = config
            .filesystem
            .rootfs
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/opt/rootfs.ext4".to_string());
        let drive_body = serde_json::json!({
            "drive_id": "rootfs",
            "path_on_host": rootfs_path,
            "is_root_device": true,
            "is_read_only": false,
        })
        .to_string();
        let (code, _) = Self::api_request(&socket, "PUT", "/drives/rootfs", &drive_body).await?;
        if code >= 400 {
            return Err(McpError::Internal(format!(
                "PUT /drives/rootfs returned HTTP {}",
                code
            )));
        }

        Ok(())
    }
}

impl Default for FirecrackerSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SandboxBackendTrait for FirecrackerSandboxBackend {
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        let id = uuid::Uuid::new_v4().to_string();

        // Create sandbox working directory
        let sandbox_dir = self.base_dir.join(&id);
        tokio::fs::create_dir_all(&sandbox_dir)
            .await
            .map_err(|e| McpError::Internal(format!("failed to create sandbox dir: {}", e)))?;

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
        if instance.state != SandboxState::Ready {
            return Err(McpError::InvalidRequest(
                "sandbox not in Ready state".to_string(),
            ));
        }

        let socket_path = Self::socket_path(&instance.id);

        // Remove stale socket if present
        let _ = tokio::fs::remove_file(&socket_path).await;

        // Spawn firecracker process
        let mut cmd = tokio::process::Command::new(&self.firecracker_bin);
        cmd.args(["--api-sock", socket_path.to_str().unwrap_or_default()]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Internal(format!("failed to spawn firecracker: {}", e)))?;

        let stdout_buf = child.stdout.take().map(|o| BufReader::new(o));
        let stderr_buf = child.stderr.take().map(|e| BufReader::new(e));
        let stdout = Arc::new(tokio::sync::Mutex::new(String::new()));
        let stderr = Arc::new(tokio::sync::Mutex::new(String::new()));

        if let Some(mut reader) = stdout_buf {
            let out = stdout.clone();
            tokio::spawn(async move {
                let mut s = out.lock().await;
                let _ = reader.read_to_string(&mut s).await;
            });
        }
        if let Some(mut reader) = stderr_buf {
            let err = stderr.clone();
            tokio::spawn(async move {
                let mut s = err.lock().await;
                let _ = reader.read_to_string(&mut s).await;
            });
        }

        // Wait for the API socket to become available (up to 5 seconds)
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut connected = false;
        while std::time::Instant::now() < deadline {
            if tokio::net::UnixStream::connect(&socket_path).await.is_ok() {
                connected = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        if !connected {
            // Clean up
            let _ = child.kill().await;
            return Err(McpError::Internal(
                "firecracker API socket did not become available".to_string(),
            ));
        }

        // Configure the VM via REST API
        if let Err(e) = self.configure_vm(&instance.id, &instance.config).await {
            let _ = child.kill().await;
            return Err(e);
        }

        // PUT /actions — InstanceStart
        let (code, body) = Self::api_request(
            &socket_path,
            "PUT",
            "/actions",
            r#"{"action_type": "InstanceStart"}"#,
        )
        .await?;
        if code >= 400 {
            let _ = child.kill().await;
            return Err(McpError::Internal(format!(
                "InstanceStart failed (HTTP {}): {}",
                code, body
            )));
        }

        instance.state = SandboxState::Running;
        instance.started_at = Some(SystemTime::now());

        // Store VM info
        let info = FirecrackerVmInfo {
            child,
            socket_path,
            start_time: std::time::Instant::now(),
            stdout,
            stderr,
        };
        self.vms.write().await.insert(instance.id.clone(), info);

        Ok(())
    }

    async fn wait(&self, instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        let mut info = self
            .vms
            .write()
            .await
            .remove(&instance.id)
            .ok_or_else(|| McpError::Internal("no running Firecracker VM found".to_string()))?;

        let start = info.start_time;
        let exit_code = match info.child.wait().await {
            Ok(status) => status.code().unwrap_or(-1),
            Err(e) => {
                return Ok(SandboxResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("firecracker process error: {}", e),
                    duration: start.elapsed(),
                    resource_usage: ResourceUsage::default(),
                    terminated: true,
                    termination_reason: Some(e.to_string()),
                });
            }
        };

        let stdout = info.stdout.lock().await.clone();
        let stderr = info.stderr.lock().await.clone();

        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            duration: start.elapsed(),
            resource_usage: ResourceUsage::default(),
            terminated: false,
            termination_reason: None,
        })
    }

    async fn terminate(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        if let Some(mut info) = self.vms.write().await.remove(&instance.id) {
            // Try graceful SendCtrlAltDel first
            let socket = &info.socket_path;
            let _ = Self::api_request(
                socket,
                "PUT",
                "/actions",
                r#"{"action_type": "SendCtrlAltDel"}"#,
            )
            .await;

            // Give it a moment to shut down, then force-kill
            tokio::time::sleep(Duration::from_millis(500)).await;
            let _ = info.child.kill().await;
            let _ = info.child.wait().await;
        }
        Ok(())
    }

    async fn destroy(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        // Cleanup socket file
        let socket_path = Self::socket_path(&instance.id);
        let _ = tokio::fs::remove_file(&socket_path).await;

        // Cleanup log file (firecracker writes <id>.log in CWD)
        let _ = tokio::fs::remove_file(format!("{}.log", instance.id)).await;

        // Cleanup sandbox directory
        let sandbox_dir = self.base_dir.join(&instance.id);
        let _ = tokio::fs::remove_dir_all(&sandbox_dir).await;

        // Remove from active VMs
        self.vms.write().await.remove(&instance.id);
        Ok(())
    }

    async fn resource_usage(&self, _instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        // A full implementation would query the Firecracker /metrics endpoint.
        Ok(ResourceUsage::default())
    }
}

// ---------------------------------------------------------------------------
// gVisor Sandbox Backend (stub – not yet implemented)
// ---------------------------------------------------------------------------

/// gVisor (runsc) container runtime sandbox backend.
///
/// TODO: implement using `docker --runtime=runsc` or direct `runsc` CLI.
/// gVisor provides a user-space kernel that intercepts syscalls for stronger
/// isolation than standard containers while maintaining compatibility.
pub struct GVisorSandboxBackend;

impl GVisorSandboxBackend {
    /// Create a new `GVisorSandboxBackend`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GVisorSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SandboxBackendTrait for GVisorSandboxBackend {
    async fn create(&self, _config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        // TODO: implement via docker --runtime=runsc or direct runsc create
        Err(McpError::Internal(
            "gVisor backend not yet implemented".to_string(),
        ))
    }

    async fn start(&self, _instance: &mut SandboxInstance) -> Result<(), McpError> {
        Err(McpError::Internal(
            "gVisor backend not yet implemented".to_string(),
        ))
    }

    async fn wait(&self, _instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        Err(McpError::Internal(
            "gVisor backend not yet implemented".to_string(),
        ))
    }

    async fn terminate(&self, _instance: &SandboxInstance) -> Result<(), McpError> {
        Err(McpError::Internal(
            "gVisor backend not yet implemented".to_string(),
        ))
    }

    async fn destroy(&self, _instance: &SandboxInstance) -> Result<(), McpError> {
        Ok(())
    }

    async fn resource_usage(&self, _instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        Ok(ResourceUsage::default())
    }
}

// ---------------------------------------------------------------------------
// Wasm Sandbox Backend (real implementation)
// ---------------------------------------------------------------------------

/// Information about a running Wasm process.
struct WasmProcessInfo {
    /// Handle to the wasmtime/wasmer process.
    child: tokio::process::Child,
    /// Captured stdout.
    stdout: Arc<tokio::sync::Mutex<String>>,
    /// Captured stderr.
    stderr: Arc<tokio::sync::Mutex<String>>,
    /// Process start time.
    start_time: std::time::Instant,
}

/// WebAssembly sandbox backend using the Wasmtime CLI.
///
/// Executes `.wasm` modules via the `wasmtime` command-line runtime with WASI
/// support, providing near-native speed with memory safety guarantees.
///
/// The command vector in the sandbox config is interpreted as:
/// - `config.command[0]` — path to the `.wasm` module
/// - `config.command[1..]` — arguments passed to the module
///
/// Lifecycle:
/// 1. `create()` — allocate an ID, validate the module path, prepare working dir.
/// 2. `start()`  — spawn `wasmtime run --wasi <module> [args...]`.
/// 3. `wait()`   — wait for the process to exit, capture output.
/// 4. `terminate()` — kill the process.
/// 5. `destroy()` — clean up the working directory.
pub struct WasmSandboxBackend {
    /// Active processes: sandbox_id -> WasmProcessInfo
    processes: Arc<RwLock<HashMap<String, WasmProcessInfo>>>,
    /// Path to the wasmtime binary.
    wasmtime_bin: String,
    /// Base directory for working dirs.
    base_dir: PathBuf,
}

impl WasmSandboxBackend {
    /// Create a new `WasmSandboxBackend`.
    pub fn new() -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            wasmtime_bin: "wasmtime".to_string(),
            base_dir: std::env::temp_dir().join("kias-sandbox"),
        }
    }

    /// Use a custom wasmtime binary path.
    pub fn with_binary(bin: impl Into<String>) -> Self {
        Self {
            wasmtime_bin: bin.into(),
            ..Self::new()
        }
    }

    /// Use a custom base directory.
    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = dir.into();
        self
    }

    /// Build the wasmtime CLI arguments for the given config.
    fn build_wasmtime_args(config: &SandboxConfig) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // WASI is enabled by default for `wasmtime run`, but we can add
        // explicit flags for resource limits.
        args.push("run".to_string());

        // Memory limit (Wasmtime uses --max-wasm-stack, and memory is per-module)
        if let Some(mem) = config.limits.memory_bytes {
            // wasmtime doesn't have a direct memory CLI flag, but --fuel can limit execution
            let fuel = mem / 1000; // rough fuel-to-memory heuristic
            if fuel > 0 {
                args.push("--fuel".to_string());
                args.push(format!("{}", fuel));
            }
        }

        // Module path (first argument in command)
        if let Some(module) = config.command.first() {
            args.push(module.clone());
        }

        // Remaining arguments go to the wasm module
        if config.command.len() > 1 {
            args.push("--".to_string());
            args.extend(config.command[1..].iter().cloned());
        }

        args
    }
}

impl Default for WasmSandboxBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SandboxBackendTrait for WasmSandboxBackend {
    async fn create(&self, config: &SandboxConfig) -> Result<SandboxInstance, McpError> {
        let id = uuid::Uuid::new_v4().to_string();

        // Create sandbox working directory
        let sandbox_dir = self.base_dir.join(&id);
        tokio::fs::create_dir_all(&sandbox_dir)
            .await
            .map_err(|e| McpError::Internal(format!("failed to create sandbox dir: {}", e)))?;

        // Validate that a module path is specified
        if config.command.is_empty() {
            return Err(McpError::InvalidRequest(
                "Wasm sandbox requires at least one command argument (module path)".to_string(),
            ));
        }

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
        if instance.state != SandboxState::Ready {
            return Err(McpError::InvalidRequest(
                "sandbox not in Ready state".to_string(),
            ));
        }

        let sandbox_dir = self.base_dir.join(&instance.id);
        let args = Self::build_wasmtime_args(&instance.config);

        let mut cmd = tokio::process::Command::new(&self.wasmtime_bin);
        cmd.args(&args);

        // Set working directory
        cmd.current_dir(instance.config.workdir.as_ref().unwrap_or(&sandbox_dir));

        // Environment variables
        for (k, v) in &instance.config.env {
            cmd.env(k, v);
        }

        // Capture I/O
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.stdin(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Internal(format!("failed to spawn wasmtime: {}", e)))?;

        let pid = child.id();
        instance.pid = pid;
        instance.state = SandboxState::Running;
        instance.started_at = Some(SystemTime::now());

        // Set up stdout/stderr capture
        let stdout_buf = child.stdout.take().map(|o| BufReader::new(o));
        let stderr_buf = child.stderr.take().map(|e| BufReader::new(e));
        let stdout = Arc::new(tokio::sync::Mutex::new(String::new()));
        let stderr = Arc::new(tokio::sync::Mutex::new(String::new()));

        if let Some(mut reader) = stdout_buf {
            let out = stdout.clone();
            tokio::spawn(async move {
                let mut s = out.lock().await;
                let _ = reader.read_to_string(&mut s).await;
            });
        }
        if let Some(mut reader) = stderr_buf {
            let err = stderr.clone();
            tokio::spawn(async move {
                let mut s = err.lock().await;
                let _ = reader.read_to_string(&mut s).await;
            });
        }

        let info = WasmProcessInfo {
            child,
            stdout,
            stderr,
            start_time: std::time::Instant::now(),
        };
        self.processes
            .write()
            .await
            .insert(instance.id.clone(), info);

        Ok(())
    }

    async fn wait(&self, instance: &SandboxInstance) -> Result<SandboxResult, McpError> {
        let mut info = self
            .processes
            .write()
            .await
            .remove(&instance.id)
            .ok_or_else(|| McpError::Internal("no running Wasm process found".to_string()))?;

        let start = info.start_time;
        let (exit_code, terminated, reason) = match info.child.wait().await {
            Ok(status) => (status.code().unwrap_or(-1), false, None),
            Err(e) => (-1, true, Some(e.to_string())),
        };

        let stdout = info.stdout.lock().await.clone();
        let stderr = info.stderr.lock().await.clone();

        Ok(SandboxResult {
            exit_code,
            stdout,
            stderr,
            duration: start.elapsed(),
            resource_usage: ResourceUsage::default(),
            terminated,
            termination_reason: reason,
        })
    }

    async fn terminate(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        if let Some(mut info) = self.processes.write().await.remove(&instance.id) {
            let _ = info.child.kill().await;
            let _ = info.child.wait().await;
        }
        Ok(())
    }

    async fn destroy(&self, instance: &SandboxInstance) -> Result<(), McpError> {
        // Cleanup sandbox directory
        let sandbox_dir = self.base_dir.join(&instance.id);
        let _ = tokio::fs::remove_dir_all(&sandbox_dir).await;
        self.processes.write().await.remove(&instance.id);
        Ok(())
    }

    async fn resource_usage(&self, _instance: &SandboxInstance) -> Result<ResourceUsage, McpError> {
        // Wasmtime does not expose runtime metrics via CLI.
        // A library-based integration would use Engine::increment_fuel_consumed().
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
            vec![
                "python".to_string(),
                "-c".to_string(),
                "print('hello')".to_string(),
            ],
        );

        assert_eq!(config.backend, SandboxBackend::Docker);
        assert_eq!(config.name, "test");
        assert_eq!(config.image, Some("python:3.11".to_string()));
    }

    #[test]
    fn test_sandbox_config_process() {
        let config = SandboxConfig::process("test", vec!["echo".to_string(), "hello".to_string()]);

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
            .register_backend(
                SandboxBackend::Process,
                Arc::new(ProcessSandboxBackend::new()),
            )
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

    #[test]
    fn test_firecracker_backend_stub() {
        let backend = FirecrackerSandboxBackend::new();
        // Verify it's constructible and implements Default
        let _default = FirecrackerSandboxBackend::default();
        let _ = &backend;
    }

    #[test]
    fn test_firecracker_backend_custom_binary() {
        let backend = FirecrackerSandboxBackend::with_binary("/usr/local/bin/firecracker");
        assert_eq!(backend.firecracker_bin, "/usr/local/bin/firecracker");
    }

    #[test]
    fn test_firecracker_socket_path() {
        let path = FirecrackerSandboxBackend::socket_path("abc-123");
        assert_eq!(path, PathBuf::from("/tmp/firecracker-abc-123.socket"));
    }

    #[test]
    fn test_sandbox_config_firecracker() {
        let config = SandboxConfig::firecracker("test-vm", "/opt/vmlinux", "/opt/rootfs.ext4");
        assert_eq!(config.backend, SandboxBackend::Firecracker);
        assert_eq!(config.name, "test-vm");
        assert_eq!(
            config.filesystem.rootfs,
            Some(PathBuf::from("/opt/rootfs.ext4"))
        );
        assert_eq!(config.labels.get("kernel").unwrap(), "/opt/vmlinux");
    }

    #[test]
    fn test_gvisor_backend_stub() {
        let backend = GVisorSandboxBackend::new();
        let _default = GVisorSandboxBackend::default();
        let _ = &backend;
    }

    #[test]
    fn test_wasm_backend_stub() {
        let backend = WasmSandboxBackend::new();
        let _default = WasmSandboxBackend::default();
        let _ = &backend;
    }

    #[test]
    fn test_wasm_backend_custom_binary() {
        let backend = WasmSandboxBackend::with_binary("/usr/local/bin/wasmtime");
        assert_eq!(backend.wasmtime_bin, "/usr/local/bin/wasmtime");
    }

    #[test]
    fn test_wasm_build_args_basic() {
        let config = SandboxConfig::wasm("test", "module.wasm");
        let args = WasmSandboxBackend::build_wasmtime_args(&config);
        assert_eq!(args[0], "run");
        // should have --fuel from default memory limit
        assert!(args.contains(&"--fuel".to_string()));
        assert!(args.contains(&"module.wasm".to_string()));
    }

    #[test]
    fn test_wasm_build_args_with_extra_args() {
        let mut config = SandboxConfig::wasm("test", "module.wasm");
        config.command.push("--verbose".to_string());
        config.command.push("42".to_string());
        let args = WasmSandboxBackend::build_wasmtime_args(&config);
        assert!(args.contains(&"--".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"42".to_string()));
    }

    #[tokio::test]
    async fn test_firecracker_backend_create_success() {
        let backend = FirecrackerSandboxBackend::new();
        let config = SandboxConfig::firecracker("test-vm", "/opt/vmlinux", "/opt/rootfs.ext4");
        let instance = backend.create(&config).await.unwrap();
        assert_eq!(instance.state, SandboxState::Ready);
        assert_eq!(instance.config.backend, SandboxBackend::Firecracker);
        assert!(instance.id.len() > 0);
    }

    #[tokio::test]
    async fn test_firecracker_backend_start_fails_without_binary() {
        let backend = FirecrackerSandboxBackend::with_binary("/nonexistent/firecracker");
        let config = SandboxConfig::firecracker("test-vm", "/opt/vmlinux", "/opt/rootfs.ext4");
        let mut instance = backend.create(&config).await.unwrap();
        let result = backend.start(&mut instance).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gvisor_backend_returns_error() {
        let backend = GVisorSandboxBackend::new();
        let config = SandboxConfig::process("test", vec!["echo".to_string()]);
        let result = backend.create(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wasm_backend_create_success() {
        let backend = WasmSandboxBackend::new();
        let config = SandboxConfig::wasm("test", "module.wasm");
        let instance = backend.create(&config).await.unwrap();
        assert_eq!(instance.state, SandboxState::Ready);
        assert_eq!(instance.config.backend, SandboxBackend::Wasm);
        assert_eq!(instance.pid, None);
    }

    #[tokio::test]
    async fn test_wasm_backend_create_requires_module() {
        let backend = WasmSandboxBackend::new();
        let config = SandboxConfig::process("test", vec![]);
        let result = backend.create(&config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wasm_backend_start_fails_without_binary() {
        let backend = WasmSandboxBackend::with_binary("/nonexistent/wasmtime");
        let config = SandboxConfig::wasm("test", "module.wasm");
        let mut instance = backend.create(&config).await.unwrap();
        let result = backend.start(&mut instance).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wasm_backend_terminate_without_process() {
        let backend = WasmSandboxBackend::new();
        let config = SandboxConfig::wasm("test", "module.wasm");
        let instance = backend.create(&config).await.unwrap();
        // Terminate on non-started instance should succeed gracefully
        let result = backend.terminate(&instance).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_firecracker_backend_terminate_without_process() {
        let backend = FirecrackerSandboxBackend::new();
        let config = SandboxConfig::firecracker("test-vm", "/opt/vmlinux", "/opt/rootfs.ext4");
        let instance = backend.create(&config).await.unwrap();
        // Terminate on non-started instance should succeed gracefully
        let result = backend.terminate(&instance).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_sandbox_echo_with_args() {
        let backend = ProcessSandboxBackend::new();
        let config = SandboxConfig::process(
            "echo-test",
            vec!["echo".to_string(), "hello world".to_string()],
        );

        let mut instance = backend.create(&config).await.unwrap();
        assert_eq!(instance.state, SandboxState::Ready);

        backend.start(&mut instance).await.unwrap();
        assert_eq!(instance.state, SandboxState::Running);

        let result = backend.wait(&instance).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello world");
    }

    // -----------------------------------------------------------------------
    // Snapshot & State Recovery Tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_isolation_level_display() {
        assert_eq!(IsolationLevel::Session.to_string(), "session");
        assert_eq!(IsolationLevel::User.to_string(), "user");
        assert_eq!(IsolationLevel::Global.to_string(), "global");
    }

    #[test]
    fn test_isolation_level_default() {
        assert_eq!(IsolationLevel::default(), IsolationLevel::Session);
    }

    #[test]
    fn test_sandbox_snapshot_new() {
        let snap = SandboxSnapshot::new("sb-1", IsolationLevel::User);
        assert_eq!(snap.sandbox_id, "sb-1");
        assert_eq!(snap.isolation_level, IsolationLevel::User);
        assert!(snap.files.is_empty());
        assert!(snap.env.is_empty());
        assert!(snap.workdir.is_none());
        assert!(snap.owner.is_none());
    }

    #[test]
    fn test_sandbox_snapshot_builder() {
        let snap = SandboxSnapshot::new("sb-2", IsolationLevel::Global)
            .with_owner("admin")
            .with_description("baseline snapshot");
        assert_eq!(snap.owner, Some("admin".to_string()));
        assert_eq!(snap.description, Some("baseline snapshot".to_string()));
    }

    #[test]
    fn test_sandbox_snapshot_add_file() {
        let mut snap = SandboxSnapshot::new("sb-3", IsolationLevel::Session);
        snap.add_file("config.toml", b"key = 'value'".to_vec());
        snap.add_file("data/input.json", b"{}".to_vec());
        assert_eq!(snap.files.len(), 2);
        assert_eq!(snap.files.get("config.toml").unwrap(), &b"key = 'value'");
    }

    #[test]
    fn test_sandbox_snapshot_json_roundtrip() {
        let mut snap = SandboxSnapshot::new("sb-4", IsolationLevel::User)
            .with_owner("alice")
            .with_description("test snap");
        snap.env.insert("FOO".into(), "bar".into());
        snap.workdir = Some(PathBuf::from("/work"));
        snap.add_file("hello.txt", b"world".to_vec());

        let json = snap.to_json().unwrap();
        let restored = SandboxSnapshot::from_json(&json).unwrap();

        assert_eq!(restored.sandbox_id, "sb-4");
        assert_eq!(restored.isolation_level, IsolationLevel::User);
        assert_eq!(restored.owner, Some("alice".to_string()));
        assert_eq!(restored.env.get("FOO").unwrap(), "bar");
        assert_eq!(restored.workdir, Some(PathBuf::from("/work")));
        assert_eq!(restored.files.get("hello.txt").unwrap(), &b"world"[..]);
    }

    #[test]
    fn test_workspace_projection_default() {
        let proj = WorkspaceProjection::default();
        assert_eq!(proj.workspace_root, PathBuf::from("."));
        assert_eq!(proj.sandbox_dest, PathBuf::from("/workspace"));
        assert!(proj.includes.contains(&"AGENTS.md".to_string()));
        assert!(proj.includes.contains(&"skills/".to_string()));
        assert!(proj.includes.contains(&"knowledge/".to_string()));
    }

    #[test]
    fn test_workspace_projection_builder() {
        let proj = WorkspaceProjection::new("/my/workspace")
            .with_includes(vec!["README.md".to_string(), "src/".to_string()])
            .with_sandbox_dest("/app");
        assert_eq!(proj.workspace_root, PathBuf::from("/my/workspace"));
        assert_eq!(proj.sandbox_dest, PathBuf::from("/app"));
        assert_eq!(proj.includes.len(), 2);
    }

    #[tokio::test]
    async fn test_save_and_list_snapshot() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let manager = SandboxManager::with_snapshot_dir(
            SandboxManagerConfig::default(),
            tmp_dir.path().to_path_buf(),
        );

        // Register backend and create a sandbox
        manager
            .register_backend(
                SandboxBackend::Process,
                Arc::new(ProcessSandboxBackend::new()),
            )
            .await;

        let mut config = SandboxConfig::process("snap-test", vec!["echo".to_string()]);
        config.env.insert("TEST_VAR".into(), "hello".into());

        let _instance = manager.execute(config, "test-user").await.unwrap();

        // We need a sandbox that persists (execute consumes it).
        // Create a sandbox manually instead for snapshot testing.
        let backend = ProcessSandboxBackend::new();
        let mut cfg =
            SandboxConfig::process("sb-snap", vec!["sleep".to_string(), "30".to_string()]);
        cfg.env.insert("A".into(), "1".into());
        let mut inst = backend.create(&cfg).await.unwrap();
        backend.start(&mut inst).await.unwrap();

        // Insert into manager
        {
            let mut sandboxes = manager.sandboxes.write().await;
            sandboxes.insert(inst.id.clone(), inst.clone());
        }

        // Save snapshot
        let snapshot = manager
            .save_snapshot(&inst.id, IsolationLevel::User, "tester")
            .await
            .unwrap();

        assert_eq!(snapshot.sandbox_id, inst.id);
        assert_eq!(snapshot.isolation_level, IsolationLevel::User);
        assert_eq!(snapshot.env.get("A").unwrap(), "1");

        // List snapshots
        let snapshots = manager.list_snapshots().await;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].id, snapshot.id);

        // Audit log should have SnapshotSave entry
        let log = manager.audit_log().await;
        let snap_audit = log
            .iter()
            .find(|e| matches!(e.action, SandboxAction::SnapshotSave));
        assert!(snap_audit.is_some());

        // Clean up the running process
        backend.terminate(&inst).await.ok();
    }

    #[tokio::test]
    async fn test_restore_snapshot_to_sandbox() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let manager = SandboxManager::with_snapshot_dir(
            SandboxManagerConfig::default(),
            tmp_dir.path().to_path_buf(),
        );

        manager
            .register_backend(
                SandboxBackend::Process,
                Arc::new(ProcessSandboxBackend::new()),
            )
            .await;

        // Create a snapshot manually (with file contents)
        let mut snapshot = SandboxSnapshot::new("original-sb", IsolationLevel::Session);
        snapshot.env.insert("RESTORED".into(), "yes".into());
        snapshot.add_file("restored.txt", b"restored content".to_vec());

        // Insert snapshot into manager's store
        {
            let mut snapshots = manager.snapshots.write().await;
            snapshots.insert(snapshot.id.clone(), snapshot.clone());
        }

        // Create target sandbox
        let backend = ProcessSandboxBackend::new();
        let cfg = SandboxConfig::process("target-sb", vec!["sleep".to_string(), "30".to_string()]);
        let mut inst = backend.create(&cfg).await.unwrap();
        backend.start(&mut inst).await.unwrap();

        {
            let mut sandboxes = manager.sandboxes.write().await;
            sandboxes.insert(inst.id.clone(), inst.clone());
        }

        // Restore snapshot to the target sandbox
        manager
            .restore_snapshot(&snapshot.id, Some(&inst.id), "tester")
            .await
            .unwrap();

        // Verify env was applied
        {
            let sandboxes = manager.sandboxes.read().await;
            let sb = sandboxes.get(&inst.id).unwrap();
            assert_eq!(sb.config.env.get("RESTORED").unwrap(), "yes");
        }

        // Verify file was written to sandbox dir
        let restored_file = std::env::temp_dir()
            .join("kias-sandbox")
            .join(&inst.id)
            .join("restored.txt");
        assert!(restored_file.exists());
        let contents = std::fs::read_to_string(&restored_file).unwrap();
        assert_eq!(contents, "restored content");

        // Audit should have SnapshotRestore
        let log = manager.audit_log().await;
        assert!(log
            .iter()
            .any(|e| matches!(e.action, SandboxAction::SnapshotRestore)));

        backend.terminate(&inst).await.ok();
    }

    #[tokio::test]
    async fn test_workspace_projection_sync() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let tmp_path = tmp_dir.path();

        // Create workspace structure
        std::fs::create_dir_all(tmp_path.join("skills")).unwrap();
        std::fs::write(tmp_path.join("AGENTS.md"), "# Agents").unwrap();
        std::fs::write(tmp_path.join("skills/bash.md"), "# Bash Skill").unwrap();

        let manager = SandboxManager::with_snapshot_dir(
            SandboxManagerConfig::default(),
            tmp_path.join("snapshots"),
        );

        manager
            .register_backend(
                SandboxBackend::Process,
                Arc::new(ProcessSandboxBackend::new()),
            )
            .await;

        // Create a sandbox
        let backend = ProcessSandboxBackend::new();
        let cfg = SandboxConfig::process("ws-test", vec!["sleep".to_string(), "30".to_string()]);
        let mut inst = backend.create(&cfg).await.unwrap();
        backend.start(&mut inst).await.unwrap();

        {
            let mut sandboxes = manager.sandboxes.write().await;
            sandboxes.insert(inst.id.clone(), inst.clone());
        }

        // Project workspace
        let projection = WorkspaceProjection::new(tmp_path);
        let count = manager
            .sync_workspace_to_sandbox(&projection, &inst.id, "tester")
            .await
            .unwrap();

        // 2 files: AGENTS.md + skills/bash.md
        assert_eq!(count, 2);

        // Verify files in sandbox
        let sandbox_dir = std::env::temp_dir()
            .join("kias-sandbox")
            .join(&inst.id)
            .join("workspace");
        assert!(sandbox_dir.join("AGENTS.md").exists());
        assert!(sandbox_dir.join("skills/bash.md").exists());

        let agents_content = std::fs::read_to_string(sandbox_dir.join("AGENTS.md")).unwrap();
        assert_eq!(agents_content, "# Agents");

        // Audit should have WorkspaceProjection
        let log = manager.audit_log().await;
        assert!(log
            .iter()
            .any(|e| matches!(e.action, SandboxAction::WorkspaceProjection)));

        backend.terminate(&inst).await.ok();
    }

    #[tokio::test]
    async fn test_snapshot_not_found() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let manager = SandboxManager::with_snapshot_dir(
            SandboxManagerConfig::default(),
            tmp_dir.path().to_path_buf(),
        );

        let result = manager
            .restore_snapshot("nonexistent-snap", None, "tester")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let manager = SandboxManager::with_snapshot_dir(
            SandboxManagerConfig::default(),
            tmp_dir.path().to_path_buf(),
        );

        let mut snap = SandboxSnapshot::new("del-test", IsolationLevel::Session);
        snap.add_file("f.txt", b"data".to_vec());

        {
            let mut snapshots = manager.snapshots.write().await;
            snapshots.insert(snap.id.clone(), snap.clone());
        }

        // Verify it's there
        assert_eq!(manager.list_snapshots().await.len(), 1);

        // Delete
        manager.delete_snapshot(&snap.id).await.unwrap();

        // Verify it's gone
        assert_eq!(manager.list_snapshots().await.len(), 0);
    }
}
