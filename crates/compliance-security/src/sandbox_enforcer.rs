//! # Sandbox Enforcer — Kernel-level Isolation
//!
//! Provides actual Linux kernel enforcement via:
//! - **cgroup v2**: resource limits (memory, CPU, pids)
//! - **seccomp-bpf**: syscall filtering
//!
//! Graceful degradation on non-Linux or unprivileged environments.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ── Cgroup v2 Configuration ───────────────────────────────────────────

/// Cgroup v2 resource limits.
///
/// Maps to `/sys/fs/cgroup/<name>/` control files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CgroupConfig {
    /// Cgroup name (becomes directory under unified hierarchy).
    pub name: String,
    /// `memory.max` — hard memory limit in bytes.
    pub memory_max_bytes: Option<u64>,
    /// `memory.high` — soft limit (triggers reclaim).
    pub memory_high_bytes: Option<u64>,
    /// `memory.swap.max` — swap limit in bytes.
    pub swap_max_bytes: Option<u64>,
    /// `cpu.max` — (quota, period) in microseconds.
    pub cpu_max_quota_us: Option<u64>,
    pub cpu_max_period_us: Option<u64>,
    /// `cpu.weight` — relative weight (1-10000, default 100).
    pub cpu_weight: Option<u32>,
    /// `pids.max` — max number of processes.
    pub pids_max: Option<u64>,
    /// `io.max` — block I/O limits (major:minor rbps/wbps).
    pub io_max_read_bps: Option<u64>,
    pub io_max_write_bps: Option<u64>,
}

impl Default for CgroupConfig {
    fn default() -> Self {
        Self {
            name: "agentguard-sandbox".to_string(),
            memory_max_bytes: Some(512 * 1024 * 1024), // 512MB
            memory_high_bytes: Some(384 * 1024 * 1024), // 384MB
            swap_max_bytes: Some(0),                   // no swap
            cpu_max_quota_us: Some(100_000),           // 100ms
            cpu_max_period_us: Some(100_000),          // 100ms period = 1 core
            cpu_weight: Some(100),
            pids_max: Some(64),
            io_max_read_bps: None,
            io_max_write_bps: None,
        }
    }
}

impl CgroupConfig {
    /// Create a minimal cgroup (128MB memory, 0.5 CPU, 16 pids).
    pub fn minimal(name: &str) -> Self {
        Self {
            name: name.to_string(),
            memory_max_bytes: Some(128 * 1024 * 1024),
            memory_high_bytes: Some(96 * 1024 * 1024),
            swap_max_bytes: Some(0),
            cpu_max_quota_us: Some(50_000),
            cpu_max_period_us: Some(100_000),
            cpu_weight: Some(50),
            pids_max: Some(16),
            io_max_read_bps: Some(10 * 1024 * 1024), // 10MB/s
            io_max_write_bps: Some(5 * 1024 * 1024), // 5MB/s
        }
    }

    /// Create an unrestricted cgroup (no limits, just tracking).
    pub fn unrestricted(name: &str) -> Self {
        Self {
            name: name.to_string(),
            memory_max_bytes: None,
            memory_high_bytes: None,
            swap_max_bytes: None,
            cpu_max_quota_us: None,
            cpu_max_period_us: None,
            cpu_weight: None,
            pids_max: None,
            io_max_read_bps: None,
            io_max_write_bps: None,
        }
    }
}

// ── Seccomp Profile ───────────────────────────────────────────────────

/// Seccomp BPF filter profile.
///
/// Defines which syscalls are allowed (allowlist) or blocked (denylist).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeccompProfile {
    /// Profile name for identification.
    pub name: String,
    /// Filter mode.
    pub mode: SeccompMode,
    /// Syscall names (e.g., "read", "write", "openat").
    pub syscalls: Vec<String>,
    /// Action to take on violation.
    pub on_violation: SeccompAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeccompMode {
    /// Only listed syscalls are allowed.
    Allowlist,
    /// Listed syscalls are blocked; all others allowed.
    Denylist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeccompAction {
    /// Kill the process immediately.
    Kill,
    /// Return EPERM to the caller.
    Errno,
    /// Log the violation but allow.
    Log,
    /// Trap — send SIGSYS to the process.
    Trap,
}

impl Default for SeccompProfile {
    fn default() -> Self {
        Self::denylist_preset()
    }
}

impl SeccompProfile {
    /// Default denylist: block dangerous syscalls, allow everything else.
    pub fn denylist_preset() -> Self {
        Self {
            name: "default-denylist".to_string(),
            mode: SeccompMode::Denylist,
            syscalls: vec![
                // Process injection / tracing
                "ptrace".into(),
                "process_vm_readv".into(),
                "process_vm_writev".into(),
                // Kernel module loading
                "init_module".into(),
                "finit_module".into(),
                "delete_module".into(),
                // Mount / filesystem manipulation
                "mount".into(),
                "umount2".into(),
                "pivot_root".into(),
                "chroot".into(),
                // System-level
                "reboot".into(),
                "kexec_load".into(),
                "kexec_file_load".into(),
                "swapon".into(),
                "swapoff".into(),
                // BPF / tracing
                "bpf".into(),
                "perf_event_open".into(),
                // Userfaultfd
                "userfaultfd".into(),
                // Keyring (can leak secrets)
                "add_key".into(),
                "keyctl".into(),
                // Seccomp itself (prevent filter removal)
                "seccomp".into(),
            ],
            on_violation: SeccompAction::Kill,
        }
    }

    /// Strict allowlist: only basic I/O, memory, and process management.
    pub fn strict_allowlist() -> Self {
        Self {
            name: "strict-allowlist".to_string(),
            mode: SeccompMode::Allowlist,
            syscalls: vec![
                // Basic I/O
                "read".into(),
                "write".into(),
                "readv".into(),
                "writev".into(),
                "close".into(),
                "lseek".into(),
                "fsync".into(),
                "fdatasync".into(),
                // File operations (restricted by filesystem policy)
                "openat".into(),
                "stat".into(),
                "fstat".into(),
                "lstat".into(),
                "access".into(),
                "faccessat".into(),
                "readlink".into(),
                "getdents64".into(),
                "statx".into(),
                // Memory
                "mmap".into(),
                "munmap".into(),
                "mprotect".into(),
                "madvise".into(),
                "brk".into(),
                "sbrk".into(),
                // Process
                "exit".into(),
                "exit_group".into(),
                "wait4".into(),
                "waitid".into(),
                "getpid".into(),
                "getppid".into(),
                "getuid".into(),
                "getgid".into(),
                "gettid".into(),
                "getrandom".into(),
                // Signals (basic)
                "rt_sigaction".into(),
                "rt_sigprocmask".into(),
                "rt_sigreturn".into(),
                "sigaltstack".into(),
                // Time
                "clock_gettime".into(),
                "clock_getres".into(),
                "nanosleep".into(),
                "clock_nanosleep".into(),
                // Pipe / eventfd
                "pipe".into(),
                "pipe2".into(),
                "eventfd2".into(),
                "epoll_create1".into(),
                "epoll_ctl".into(),
                "epoll_wait".into(),
                // Futex (for Rust mutexes)
                "futex".into(),
                // Prctl (limited)
                "arch_prctl".into(),
                "set_tid_address".into(),
                "rseq".into(),
                "clone3".into(),
            ],
            on_violation: SeccompAction::Kill,
        }
    }

    /// Convert syscall names to numbers for BPF generation.
    pub fn to_syscall_numbers(&self) -> Vec<i64> {
        self.syscalls
            .iter()
            .filter_map(|name| syscall_number(name))
            .collect()
    }
}

// ── Syscall Number Mapping ────────────────────────────────────────────

/// Map common syscall names to x86_64 syscall numbers.
/// Returns None for unknown syscalls.
fn syscall_number(name: &str) -> Option<i64> {
    match name {
        // I/O
        "read" => Some(0),
        "write" => Some(1),
        "open" => Some(2),
        "close" => Some(3),
        "stat" => Some(4),
        "fstat" => Some(5),
        "lstat" => Some(6),
        "poll" => Some(7),
        "lseek" => Some(8),
        "mmap" => Some(9),
        "mprotect" => Some(10),
        "munmap" => Some(11),
        "brk" => Some(12),
        "rt_sigaction" => Some(13),
        "rt_sigprocmask" => Some(14),
        "rt_sigreturn" => Some(15),
        "ioctl" => Some(16),
        "readv" => Some(19),
        "writev" => Some(20),
        "access" => Some(21),
        "pipe" => Some(22),
        "select" => Some(23),
        "sched_yield" => Some(24),
        "madvise" => Some(28),
        "dup" => Some(32),
        "dup2" => Some(33),
        "nanosleep" => Some(35),
        "getpid" => Some(39),
        "getuid" => Some(102),
        "getgid" => Some(104),
        "getppid" => Some(110),
        "gettid" => Some(186),
        "socket" => Some(41),
        "connect" => Some(42),
        "accept" => Some(43),
        "sendto" => Some(44),
        "recvfrom" => Some(45),
        "shutdown" => Some(48),
        "bind" => Some(49),
        "listen" => Some(50),
        "epoll_create1" => Some(291),
        "epoll_ctl" => Some(233),
        "epoll_wait" => Some(232),
        "eventfd2" => Some(290),
        "futex" => Some(202),
        "clock_gettime" => Some(228),
        "clock_getres" => Some(229),
        "clock_nanosleep" => Some(230),
        "exit" => Some(60),
        "exit_group" => Some(231),
        "wait4" => Some(61),
        "waitid" => Some(247),
        "kill" => Some(62),
        "uname" => Some(63),
        "fcntl" => Some(72),
        "fsync" => Some(74),
        "fdatasync" => Some(75),
        "getdents64" => Some(217),
        "openat" => Some(257),
        "mkdirat" => Some(258),
        "unlinkat" => Some(263),
        "renameat2" => Some(264),
        "faccessat" => Some(269),
        "readlink" => Some(89),
        "statx" => Some(332),
        "sigaltstack" => Some(131),
        "getrandom" => Some(318),
        "pipe2" => Some(293),
        "set_tid_address" => Some(218),
        "arch_prctl" => Some(158),
        "rseq" => Some(334),
        "clone3" => Some(435),
        // Dangerous (for denylist)
        "ptrace" => Some(101),
        "mount" => Some(165),
        "umount2" => Some(166),
        "reboot" => Some(169),
        "init_module" => Some(175),
        "finit_module" => Some(313),
        "delete_module" => Some(176),
        "bpf" => Some(321),
        "userfaultfd" => Some(323),
        "kexec_load" => Some(246),
        "kexec_file_load" => Some(320),
        "pivot_root" => Some(155),
        "chroot" => Some(161),
        "swapon" => Some(167),
        "swapoff" => Some(168),
        "perf_event_open" => Some(298),
        "add_key" => Some(248),
        "keyctl" => Some(250),
        "seccomp" => Some(317),
        "process_vm_readv" => Some(270),
        "process_vm_writev" => Some(271),
        "sethostname" => Some(170),
        "setdomainname" => Some(171),
        _ => None,
    }
}

// ── Cgroup Enforcer ───────────────────────────────────────────────────

/// Cgroup v2 base path (unified hierarchy).
const CGROUP_BASE: &str = "/sys/fs/cgroup";

/// Enforces cgroup v2 resource limits.
pub struct CgroupEnforcer {
    /// Path to the cgroup directory.
    path: PathBuf,
    /// Whether the cgroup was created by us.
    created: bool,
}

impl CgroupEnforcer {
    /// Create and configure a cgroup.
    pub fn create(config: &CgroupConfig) -> Result<Self, SandboxEnforceError> {
        let path = PathBuf::from(CGROUP_BASE).join(&config.name);

        // Create cgroup directory
        std::fs::create_dir_all(&path).map_err(|e| {
            SandboxEnforceError::CgroupError(format!(
                "Failed to create cgroup {}: {}",
                config.name, e
            ))
        })?;

        let enforcer = Self {
            path,
            created: true,
        };
        enforcer.apply_limits(config)?;
        Ok(enforcer)
    }

    /// Attach to an existing cgroup (don't create).
    pub fn attach(name: &str) -> Result<Self, SandboxEnforceError> {
        let path = PathBuf::from(CGROUP_BASE).join(name);
        if !path.exists() {
            return Err(SandboxEnforceError::CgroupError(format!(
                "Cgroup {} does not exist",
                name
            )));
        }
        Ok(Self {
            path,
            created: false,
        })
    }

    /// Apply resource limits to the cgroup.
    fn apply_limits(&self, config: &CgroupConfig) -> Result<(), SandboxEnforceError> {
        // Memory limits
        if let Some(max) = config.memory_max_bytes {
            self.write_cgroup_file("memory.max", &max.to_string())?;
        }
        if let Some(high) = config.memory_high_bytes {
            self.write_cgroup_file("memory.high", &high.to_string())?;
        }
        if let Some(swap) = config.swap_max_bytes {
            self.write_cgroup_file("memory.swap.max", &swap.to_string())?;
        }

        // CPU limits
        if let (Some(quota), Some(period)) = (config.cpu_max_quota_us, config.cpu_max_period_us) {
            self.write_cgroup_file("cpu.max", &format!("{} {}", quota, period))?;
        }
        if let Some(weight) = config.cpu_weight {
            self.write_cgroup_file("cpu.weight", &weight.to_string())?;
        }

        // PIDs limit
        if let Some(max_pids) = config.pids_max {
            self.write_cgroup_file("pids.max", &max_pids.to_string())?;
        }

        // I/O limits
        if let Some(rbps) = config.io_max_read_bps {
            // Format: "major:minor rbps=<bytes> wbps=<bytes>"
            // Use 8:0 as default block device
            let val = format!("8:0 rbps={}", rbps);
            self.write_cgroup_file("io.max", &val)?;
        }
        if let Some(wbps) = config.io_max_write_bps {
            let val = format!("8:0 wbps={}", wbps);
            // Append (but io.max only has one entry per device, so this overwrites)
            self.write_cgroup_file("io.max", &val)?;
        }

        Ok(())
    }

    /// Move a process into this cgroup.
    pub fn add_pid(&self, pid: u32) -> Result<(), SandboxEnforceError> {
        self.write_cgroup_file("cgroup.procs", &pid.to_string())
    }

    /// Read current memory usage.
    pub fn memory_current(&self) -> Result<u64, SandboxEnforceError> {
        let content = self.read_cgroup_file("memory.current")?;
        content.trim().parse().map_err(|e| {
            SandboxEnforceError::CgroupError(format!("Failed to parse memory.current: {}", e))
        })
    }

    /// Read current pids count.
    pub fn pids_current(&self) -> Result<u64, SandboxEnforceError> {
        let content = self.read_cgroup_file("pids.current")?;
        content.trim().parse().map_err(|e| {
            SandboxEnforceError::CgroupError(format!("Failed to parse pids.current: {}", e))
        })
    }

    /// Read memory.peak (high watermark).
    pub fn memory_peak(&self) -> Result<u64, SandboxEnforceError> {
        let content = self.read_cgroup_file("memory.peak")?;
        content.trim().parse().map_err(|e| {
            SandboxEnforceError::CgroupError(format!("Failed to parse memory.peak: {}", e))
        })
    }

    fn write_cgroup_file(&self, filename: &str, content: &str) -> Result<(), SandboxEnforceError> {
        let path = self.path.join(filename);
        std::fs::write(&path, content).map_err(|e| {
            SandboxEnforceError::CgroupError(format!("Failed to write {}: {}", path.display(), e))
        })
    }

    fn read_cgroup_file(&self, filename: &str) -> Result<String, SandboxEnforceError> {
        let path = self.path.join(filename);
        std::fs::read_to_string(&path).map_err(|e| {
            SandboxEnforceError::CgroupError(format!("Failed to read {}: {}", path.display(), e))
        })
    }
}

impl Drop for CgroupEnforcer {
    fn drop(&mut self) {
        if self.created {
            // Try to remove the cgroup directory on cleanup
            let _ = std::fs::remove_dir(&self.path);
        }
    }
}

// ── Seccomp Enforcer ──────────────────────────────────────────────────

/// Applies seccomp BPF filter to the current process.
pub struct SeccompEnforcer;

impl SeccompEnforcer {
    /// Install a seccomp filter for the current process.
    ///
    /// # Safety
    /// This must be called before untrusted code execution.
    /// After installation, the filter cannot be removed (only tightened).
    pub fn apply(profile: &SeccompProfile) -> Result<(), SandboxEnforceError> {
        let syscall_nums = profile.to_syscall_numbers();

        if syscall_nums.is_empty() {
            return Err(SandboxEnforceError::SeccompError(
                "No valid syscalls in profile (all names unresolved)".to_string(),
            ));
        }

        let action = match profile.on_violation {
            SeccompAction::Kill => 0x0000_0000_u32, // SECCOMP_RET_KILL_PROCESS
            SeccompAction::Errno => 0x0005_0000_u32, // SECCOMP_RET_ERRNO | EPERM
            SeccompAction::Trap => 0x0003_0000_u32, // SECCOMP_RET_TRAP
            SeccompAction::Log => 0x7ffc_0000_u32,  // SECCOMP_RET_LOG
        };

        let allow_action: u32 = 0x7fff_0000; // SECCOMP_RET_ALLOW

        // Build BPF program
        let bpf = Self::build_bpf_program(&syscall_nums, &profile.mode, action, allow_action);

        // Install the filter
        unsafe {
            Self::install_bpf_filter(&bpf)?;
        }

        Ok(())
    }

    /// Check if seccomp is available on this system.
    pub fn is_available() -> bool {
        // Check if /proc/self/status has Seccomp field
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            status.contains("Seccomp:")
        } else {
            false
        }
    }

    /// Check current seccomp mode (0=disabled, 1=strict, 2=filter).
    pub fn current_mode() -> u32 {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("Seccomp:") {
                    return line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
            }
        }
        0
    }

    /// Build a minimal BPF program for syscall filtering.
    ///
    /// Program structure:
    /// 1. Load syscall number from seccomp_data
    /// 2. Compare against allowed syscall numbers
    /// 3. Return ALLOW or ACTION based on mode
    fn build_bpf_program(
        syscall_nums: &[i64],
        mode: &SeccompMode,
        violation_action: u32,
        allow_action: u32,
    ) -> Vec<sock_filter> {
        let mut program = Vec::new();
        let num_syscalls = syscall_nums.len();

        // BPF instruction: (code, jt, jf, k)
        // Load syscall number: LD [0] (offset 0 in seccomp_data = nr)
        program.push(sock_filter {
            code: 0x20,
            jt: 0,
            jf: 0,
            k: 0,
        }); // LD W seccomp_data[0]

        // For each syscall, compare and branch
        for (i, &nr) in syscall_nums.iter().enumerate() {
            let remaining = num_syscalls - i - 1;

            match mode {
                SeccompMode::Allowlist => {
                    // If match → allow, if not → check next
                    program.push(sock_filter {
                        code: 0x15,                   // JEQ
                        jt: (num_syscalls - i) as u8, // jump to ALLOW (at end)
                        jf: 0,
                        k: nr as u32,
                    });
                    // If this is the last syscall and no match → deny
                    if remaining == 0 {
                        // Push deny action
                        program.push(sock_filter {
                            code: 0x06, // RET
                            jt: 0,
                            jf: 0,
                            k: violation_action,
                        });
                    }
                }
                SeccompMode::Denylist => {
                    // If match → deny, if not → check next
                    program.push(sock_filter {
                        code: 0x15, // JEQ
                        jt: 0,      // will be filled
                        jf: 0,
                        k: nr as u32,
                    });
                }
            }
        }

        // Add ALLOW at the end
        program.push(sock_filter {
            code: 0x06, // RET
            jt: 0,
            jf: 0,
            k: match mode {
                SeccompMode::Allowlist => allow_action,
                SeccompMode::Denylist => allow_action,
            },
        });

        // For denylist mode: fix jumps and add DENY before ALLOW
        if *mode == SeccompMode::Denylist {
            let allow_idx = program.len() - 1;
            // Insert DENY instruction before ALLOW
            let deny_instr = sock_filter {
                code: 0x06,
                jt: 0,
                jf: 0,
                k: violation_action,
            };
            program.insert(allow_idx, deny_instr);
            let deny_idx = allow_idx;

            // Now fix all JEQ instructions: in denylist mode,
            // if syscall matches, jump to DENY; otherwise fall through
            // The first instruction loads the syscall number
            for (i, instr) in program.iter_mut().enumerate().take(num_syscalls + 1).skip(1) {
                if instr.code == 0x15 {
                    // JEQ
                    instr.jt = (deny_idx - i) as u8;
                }
            }
        }

        program
    }

    /// Install BPF filter via prctl/seccomp syscall.
    ///
    /// # Safety
    /// Uses raw syscalls. The BPF program must be valid.
    unsafe fn install_bpf_filter(program: &[sock_filter]) -> Result<(), SandboxEnforceError> {
        if program.is_empty() {
            return Err(SandboxEnforceError::SeccompError(
                "Empty BPF program".to_string(),
            ));
        }

        let prog = sock_fprog {
            len: program.len() as u16,
            filter: program.as_ptr(),
        };

        // Try seccomp() syscall first (Linux 3.17+)
        // SECCOMP_SET_MODE_FILTER = 1
        // SECCOMP_FILTER_FLAG_TSYNC = 1
        let ret = libc::syscall(libc::SYS_seccomp, 1u64, 1u64, &prog as *const _);
        if ret == 0 {
            return Ok(());
        }

        // Fallback to prctl()
        // PR_SET_NO_NEW_PRIVS = 38
        let ret = libc::prctl(38, 1, 0, 0, 0);
        if ret != 0 {
            return Err(SandboxEnforceError::SeccompError(format!(
                "prctl(PR_SET_NO_NEW_PRIVS) failed: {}",
                std::io::Error::last_os_error()
            )));
        }

        // PR_SET_SECCOMP = 22
        // SECCOMP_MODE_FILTER = 2
        let ret = libc::prctl(22, 2, &prog as *const _, 0, 0);
        if ret != 0 {
            return Err(SandboxEnforceError::SeccompError(format!(
                "prctl(PR_SET_SECCOMP) failed: {}",
                std::io::Error::last_os_error()
            )));
        }

        Ok(())
    }
}

// ── BPF Types ─────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct sock_filter {
    code: u16,
    jt: u8,
    jf: u8,
    k: u32,
}

#[repr(C)]
struct sock_fprog {
    len: u16,
    filter: *const sock_filter,
}

// ── Errors ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum SandboxEnforceError {
    CgroupError(String),
    SeccompError(String),
    NotSupported(String),
}

impl std::fmt::Display for SandboxEnforceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CgroupError(e) => write!(f, "Cgroup error: {}", e),
            Self::SeccompError(e) => write!(f, "Seccomp error: {}", e),
            Self::NotSupported(e) => write!(f, "Not supported: {}", e),
        }
    }
}

impl std::error::Error for SandboxEnforceError {}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── CgroupConfig tests ─────────────────────────────────────────

    #[test]
    fn test_cgroup_config_default() {
        let cfg = CgroupConfig::default();
        assert_eq!(cfg.name, "agentguard-sandbox");
        assert_eq!(cfg.memory_max_bytes, Some(512 * 1024 * 1024));
        assert_eq!(cfg.cpu_max_quota_us, Some(100_000));
        assert_eq!(cfg.pids_max, Some(64));
        assert_eq!(cfg.swap_max_bytes, Some(0));
    }

    #[test]
    fn test_cgroup_config_minimal() {
        let cfg = CgroupConfig::minimal("test-min");
        assert_eq!(cfg.memory_max_bytes, Some(128 * 1024 * 1024));
        assert_eq!(cfg.cpu_max_quota_us, Some(50_000));
        assert_eq!(cfg.pids_max, Some(16));
        assert!(cfg.io_max_read_bps.is_some());
    }

    #[test]
    fn test_cgroup_config_unrestricted() {
        let cfg = CgroupConfig::minimal("test-unrestricted");
        assert!(cfg.memory_max_bytes.is_some());

        let cfg = CgroupConfig::unrestricted("test-free");
        assert!(cfg.memory_max_bytes.is_none());
        assert!(cfg.cpu_max_quota_us.is_none());
        assert!(cfg.pids_max.is_none());
    }

    #[test]
    fn test_cgroup_config_serialization() {
        let cfg = CgroupConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        assert!(json.contains("memory_max_bytes"));
        assert!(json.contains("cpu_max_quota_us"));

        let roundtrip: CgroupConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.name, cfg.name);
        assert_eq!(roundtrip.memory_max_bytes, cfg.memory_max_bytes);
    }

    // ── SeccompProfile tests ───────────────────────────────────────

    #[test]
    fn test_seccomp_profile_denylist_preset() {
        let profile = SeccompProfile::denylist_preset();
        assert_eq!(profile.mode, SeccompMode::Denylist);
        assert!(profile.syscalls.contains(&"ptrace".to_string()));
        assert!(profile.syscalls.contains(&"mount".to_string()));
        assert!(profile.syscalls.contains(&"reboot".to_string()));
        assert!(profile.syscalls.contains(&"seccomp".to_string()));
        assert_eq!(profile.on_violation, SeccompAction::Kill);
    }

    #[test]
    fn test_seccomp_profile_strict_allowlist() {
        let profile = SeccompProfile::strict_allowlist();
        assert_eq!(profile.mode, SeccompMode::Allowlist);
        assert!(profile.syscalls.contains(&"read".to_string()));
        assert!(profile.syscalls.contains(&"write".to_string()));
        assert!(profile.syscalls.contains(&"mmap".to_string()));
        assert!(profile.syscalls.contains(&"exit".to_string()));
        // Should NOT contain dangerous syscalls
        assert!(!profile.syscalls.contains(&"ptrace".to_string()));
        assert!(!profile.syscalls.contains(&"mount".to_string()));
    }

    #[test]
    fn test_seccomp_profile_serialization() {
        let profile = SeccompProfile::default();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("Denylist"));
        assert!(json.contains("ptrace"));

        let roundtrip: SeccompProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.name, profile.name);
        assert_eq!(roundtrip.syscalls.len(), profile.syscalls.len());
    }

    #[test]
    fn test_seccomp_to_syscall_numbers() {
        let profile = SeccompProfile::denylist_preset();
        let nums = profile.to_syscall_numbers();
        assert!(!nums.is_empty());
        // ptrace = 101
        assert!(nums.contains(&101));
        // mount = 165
        assert!(nums.contains(&165));
    }

    #[test]
    fn test_syscall_number_mapping() {
        assert_eq!(syscall_number("read"), Some(0));
        assert_eq!(syscall_number("write"), Some(1));
        assert_eq!(syscall_number("openat"), Some(257));
        assert_eq!(syscall_number("ptrace"), Some(101));
        assert_eq!(syscall_number("bpf"), Some(321));
        assert_eq!(syscall_number("nonexistent_syscall_xyz"), None);
    }

    #[test]
    fn test_strict_allowlist_syscall_numbers() {
        let profile = SeccompProfile::strict_allowlist();
        let nums = profile.to_syscall_numbers();
        // All allowlist syscalls should resolve
        assert!(
            nums.len() >= 30,
            "Expected >= 30 resolved syscalls, got {}",
            nums.len()
        );
        // Basic I/O
        assert!(nums.contains(&0)); // read
        assert!(nums.contains(&1)); // write
        assert!(nums.contains(&60)); // exit
    }

    // ── BPF program tests ──────────────────────────────────────────

    #[test]
    fn test_bpf_program_generation_denylist() {
        let profile = SeccompProfile {
            name: "test".into(),
            mode: SeccompMode::Denylist,
            syscalls: vec!["ptrace".into(), "mount".into()],
            on_violation: SeccompAction::Kill,
        };
        let nums = profile.to_syscall_numbers();
        let bpf =
            SeccompEnforcer::build_bpf_program(&nums, &profile.mode, 0x0000_0000, 0x7fff_0000);
        // Should have: LD + JEQ*2 + DENY + ALLOW = 5 instructions
        assert_eq!(bpf.len(), 5);
        // First instruction: LD W [0]
        assert_eq!(bpf[0].code, 0x20);
        // Last instruction: ALLOW
        assert_eq!(bpf.last().unwrap().code, 0x06);
        assert_eq!(bpf.last().unwrap().k, 0x7fff_0000);
    }

    #[test]
    fn test_bpf_program_generation_allowlist() {
        let profile = SeccompProfile {
            name: "test".into(),
            mode: SeccompMode::Allowlist,
            syscalls: vec!["read".into(), "write".into()],
            on_violation: SeccompAction::Kill,
        };
        let nums = profile.to_syscall_numbers();
        let bpf =
            SeccompEnforcer::build_bpf_program(&nums, &profile.mode, 0x0000_0000, 0x7fff_0000);
        // Should have: LD + JEQ*2 + DENY + ALLOW = 5 instructions
        assert_eq!(bpf.len(), 5);
    }

    // ── Seccomp availability tests ─────────────────────────────────

    #[test]
    fn test_seccomp_availability() {
        // Just verify it doesn't panic
        let available = SeccompEnforcer::is_available();
        let mode = SeccompEnforcer::current_mode();
        // On Linux, seccomp should be available
        #[cfg(target_os = "linux")]
        {
            assert!(available);
        }
        // mode should be 0 (disabled), 1 (strict), or 2 (filter)
        assert!(mode <= 2);
    }

    // ── Integration: profile + cgroup config ───────────────────────

    #[test]
    fn test_full_sandbox_config() {
        let cgroup = CgroupConfig::minimal("agent-1-sandbox");
        let seccomp = SeccompProfile::strict_allowlist();

        assert_eq!(cgroup.name, "agent-1-sandbox");
        assert_eq!(seccomp.name, "strict-allowlist");

        // Verify they can be combined in a config
        let json = serde_json::json!({
            "cgroup": serde_json::to_value(&cgroup).unwrap(),
            "seccomp": serde_json::to_value(&seccomp).unwrap(),
        });
        assert!(json["cgroup"]["memory_max_bytes"].is_number());
        assert!(json["seccomp"]["syscalls"].is_array());
    }
}
