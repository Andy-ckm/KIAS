//! Runtime Protection Module
//!
//! Implements system call monitoring, network behavior detection, and sandbox escape detection

use crate::error::{KiasError, KiasResult};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

/// System call category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyscallCategory {
    File,
    Network,
    Process,
    Memory,
    Signal,
    Time,
    IPC,
    Unknown,
}

/// System call event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallEvent {
    pub pid: u32,
    pub syscall: String,
    pub category: SyscallCategory,
    pub args: Vec<String>,
    pub return_value: i64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub suspicious: bool,
}

/// Anomalous syscall pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallAnomaly {
    pub pattern_type: String,
    pub description: String,
    pub severity: u8,
    pub frequency: u32,
    pub recommendation: String,
}

/// Syscall monitor for detecting anomalous system calls
pub struct SyscallMonitor {
    enabled: RwLock<bool>,
    whitelist: RwLock<HashSet<String>>,
    blacklist: RwLock<HashSet<String>>,
    recent_events: RwLock<Vec<SyscallEvent>>,
    anomaly_threshold: u32,
}

impl Default for SyscallMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl SyscallMonitor {
    pub fn new() -> Self {
        let monitor = Self {
            enabled: RwLock::new(true),
            whitelist: RwLock::new(HashSet::new()),
            blacklist: RwLock::new(HashSet::new()),
            recent_events: RwLock::new(Vec::new()),
            anomaly_threshold: 100,
        };
        
        // Default suspicious syscalls
        if let Ok(mut blacklist) = monitor.blacklist.write() {
            blacklist.insert("ptrace".to_string());
            blacklist.insert("perf_event_open".to_string());
            blacklist.insert("process_vm_readv".to_string());
            blacklist.insert("process_vm_writev".to_string());
            blacklist.insert("init_module".to_string());
            blacklist.insert("delete_module".to_string());
            blacklist.insert("syslog".to_string());
            blacklist.insert("lookup_dcookie".to_string());
        }
        
        monitor
    }

    pub fn enable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = true;
        }
    }

    pub fn disable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = false;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.read().map(|e| *e).unwrap_or(false)
    }

    pub fn add_to_whitelist(&self, syscall: &str) {
        if let Ok(mut w) = self.whitelist.write() {
            w.insert(syscall.to_string());
        }
    }

    pub fn add_to_blacklist(&self, syscall: &str) {
        if let Ok(mut b) = self.blacklist.write() {
            b.insert(syscall.to_string());
        }
    }

    pub fn record_syscall(&self, event: SyscallEvent) -> Option<SyscallAnomaly> {
        if !self.is_enabled() {
            return None;
        }

        if let Ok(mut recent) = self.recent_events.write() {
            recent.push(event.clone());
            if recent.len() > 1000 {
                recent.drain(0..100);
            }
        }

        let is_blacklisted = self.blacklist.read().ok().map(|b| b.contains(&event.syscall)).unwrap_or(false);
        let is_whitelisted = self.whitelist.read().ok().map(|w| w.contains(&event.syscall)).unwrap_or(false);
        
        if is_blacklisted && !is_whitelisted {
            return Some(SyscallAnomaly {
                pattern_type: "blacklisted_syscall".to_string(),
                description: format!("Blacklisted syscall {} called by PID {}", event.syscall, event.pid),
                severity: 8,
                frequency: 1,
                recommendation: "Investigate this syscall immediately".to_string(),
            });
        }

        if event.category == SyscallCategory::File {
            if event.args.iter().any(|a| a.contains("/proc/") || a.contains("/sys/")) {
                return Some(SyscallAnomaly {
                    pattern_type: "suspicious_file_access".to_string(),
                    description: format!("Suspicious file path access: {:?}", event.args),
                    severity: 6,
                    frequency: 1,
                    recommendation: "Monitor file access patterns".to_string(),
                });
            }
        }

        None
    }

    pub fn get_recent_events(&self, count: usize) -> Vec<SyscallEvent> {
        self.recent_events.read().ok()
            .map(|r| r.iter().rev().take(count).cloned().collect())
            .unwrap_or_default()
    }

    pub fn detect_anomaly_patterns(&self) -> Vec<SyscallAnomaly> {
        let recent = self.recent_events.read().ok();
        let recent = match recent {
            Some(r) => r,
            None => return Vec::new(),
        };
        
        let mut anomalies = Vec::new();
        let mut syscall_counts: HashMap<String, u32> = HashMap::new();
        
        for event in recent.iter() {
            *syscall_counts.entry(event.syscall.clone()).or_insert(0) += 1;
        }
        
        for (syscall, count) in syscall_counts {
            if count > self.anomaly_threshold {
                anomalies.push(SyscallAnomaly {
                    pattern_type: "high_frequency".to_string(),
                    description: format!("Syscall {} called {} times", syscall, count),
                    severity: 5,
                    frequency: count,
                    recommendation: "Investigate high syscall frequency".to_string(),
                });
            }
        }
        
        anomalies
    }
}

/// Network connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    New,
    Established,
    CloseWait,
    TimeWait,
    Closed,
}

/// Network event type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NetworkEventType {
    Connection,
    DataTransfer,
    DnsQuery,
    Bind,
    Listen,
}

/// Network event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    pub event_type: NetworkEventType,
    pub pid: u32,
    pub remote_addr: Option<String>,
    pub local_addr: Option<String>,
    pub port: Option<u16>,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub suspicious: bool,
}

/// Suspicious network pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSuspiciousPattern {
    pub pattern_type: String,
    pub description: String,
    pub severity: u8,
    pub connections: u32,
    pub recommendation: String,
}

/// Network monitor for suspicious network behavior
pub struct NetworkMonitor {
    enabled: RwLock<bool>,
    suspicious_ports: RwLock<HashSet<u16>>,
    suspicious_domains: RwLock<HashSet<String>>,
    recent_connections: RwLock<Vec<NetworkEvent>>,
    max_connections_per_minute: u32,
}

impl Default for NetworkMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkMonitor {
    pub fn new() -> Self {
        let monitor = Self {
            enabled: RwLock::new(true),
            suspicious_ports: RwLock::new(HashSet::new()),
            suspicious_domains: RwLock::new(HashSet::new()),
            recent_connections: RwLock::new(Vec::new()),
            max_connections_per_minute: 100,
        };
        
        if let Ok(mut ports) = monitor.suspicious_ports.write() {
            ports.insert(4444);
            ports.insert(5555);
            ports.insert(6666);
            ports.insert(6667);
            ports.insert(31337);
        }
        
        monitor
    }

    pub fn enable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = true;
        }
    }

    pub fn disable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = false;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.read().map(|e| *e).unwrap_or(false)
    }

    pub fn add_suspicious_port(&self, port: u16) {
        if let Ok(mut p) = self.suspicious_ports.write() {
            p.insert(port);
        }
    }

    pub fn add_suspicious_domain(&self, domain: &str) {
        if let Ok(mut d) = self.suspicious_domains.write() {
            d.insert(domain.to_string());
        }
    }

    pub fn record_connection(&self, event: NetworkEvent) -> Option<NetworkSuspiciousPattern> {
        if !self.is_enabled() {
            return None;
        }

        if let Ok(mut connections) = self.recent_connections.write() {
            connections.push(event.clone());
            if connections.len() > 10000 {
                connections.drain(0..1000);
            }
        }

        if let Some(port) = event.port {
            if self.suspicious_ports.read().ok().map(|p| p.contains(&port)).unwrap_or(false) {
                return Some(NetworkSuspiciousPattern {
                    pattern_type: "suspicious_port".to_string(),
                    description: format!("Connection to suspicious port {}", port),
                    severity: 7,
                    connections: 1,
                    recommendation: "Block this port immediately".to_string(),
                });
            }
        }

        let recent: Vec<_> = {
            let connections = self.recent_connections.read().ok();
            let all_connections: Vec<NetworkEvent> = connections
                .map(|c| c.clone())
                .unwrap_or_default();
            all_connections
                .into_iter()
                .filter(|e| e.timestamp > chrono::Utc::now() - chrono::Duration::seconds(60))
                .collect()
        };
        
        let unique_ports: HashSet<u16> = recent.iter().filter_map(|e| e.port).collect();
        if unique_ports.len() > 50 {
            return Some(NetworkSuspiciousPattern {
                pattern_type: "port_scan".to_string(),
                description: format!("Possible port scan detected: {} unique ports", unique_ports.len()),
                severity: 9,
                connections: unique_ports.len() as u32,
                recommendation: "Block source and investigate".to_string(),
            });
        }

        None
    }

    pub fn get_recent_connections(&self, count: usize) -> Vec<NetworkEvent> {
        self.recent_connections.read().ok()
            .map(|c| c.iter().rev().take(count).cloned().collect())
            .unwrap_or_default()
    }
}

/// Sandbox escape technique type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EscapeTechnique {
    Symlink,
    RaceCondition,
    PrivilegeEscalation,
    ContainerEscape,
    CgroupEscape,
    ProcfsEscape,
    SyscallBypass,
    Unknown,
}

/// Sandbox escape attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeAttempt {
    pub technique: EscapeTechnique,
    pub pid: u32,
    pub evidence: Vec<String>,
    pub severity: u8,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub blocked: bool,
}

/// Sandbox escape detection
pub struct SandboxEscapeDetector {
    enabled: RwLock<bool>,
    escape_techniques: RwLock<HashMap<EscapeTechnique, Vec<String>>>,
    detected_attempts: RwLock<Vec<EscapeAttempt>>,
}

impl Default for SandboxEscapeDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl SandboxEscapeDetector {
    pub fn new() -> Self {
        let mut detector = Self {
            enabled: RwLock::new(true),
            escape_techniques: RwLock::new(HashMap::new()),
            detected_attempts: RwLock::new(Vec::new()),
        };
        
        if let Ok(mut techniques) = detector.escape_techniques.write() {
            techniques.insert(EscapeTechnique::ProcfsEscape, vec![
                "/proc/self/exe".to_string(),
                "/proc/self/mem".to_string(),
                "/proc/self/fd".to_string(),
                "/proc/1/exe".to_string(),
            ]);
            
            techniques.insert(EscapeTechnique::CgroupEscape, vec![
                "/proc/self/cgroup".to_string(),
                "/sys/fs/cgroup".to_string(),
                "release_agent".to_string(),
            ]);
            
            techniques.insert(EscapeTechnique::ContainerEscape, vec![
                "docker".to_string(),
                "containerd".to_string(),
                ".dockerenv".to_string(),
            ]);
            
            techniques.insert(EscapeTechnique::SyscallBypass, vec![
                "ptrace".to_string(),
                "perf_event_open".to_string(),
                "lookup_dcookie".to_string(),
            ]);
        }
        
        detector
    }

    pub fn enable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = true;
        }
    }

    pub fn disable(&self) {
        if let Ok(mut e) = self.enabled.write() {
            *e = false;
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.read().map(|e| *e).unwrap_or(false)
    }

    pub fn check_syscall(&self, event: &SyscallEvent) -> Option<EscapeAttempt> {
        if !self.is_enabled() {
            return None;
        }

        let techniques = self.escape_techniques.read().ok()?;
        
        for (technique, patterns) in techniques.iter() {
            for pattern in patterns {
                if event.args.iter().any(|a| a.contains(pattern)) {
                    let severity = match technique {
                        EscapeTechnique::ContainerEscape => 10,
                        EscapeTechnique::PrivilegeEscalation => 10,
                        EscapeTechnique::SyscallBypass => 8,
                        _ => 7,
                    };
                    
                    let attempt = EscapeAttempt {
                        technique: *technique,
                        pid: event.pid,
                        evidence: vec![format!("Matched pattern: {}", pattern)],
                        severity,
                        timestamp: event.timestamp,
                        blocked: true,
                    };
                    
                    if let Ok(mut attempts) = self.detected_attempts.write() {
                        attempts.push(attempt.clone());
                    }
                    return Some(attempt);
                }
            }
        }
        
        None
    }

    pub fn check_file_access(&self, path: &str, pid: u32) -> Option<EscapeAttempt> {
        if !self.is_enabled() {
            return None;
        }

        let techniques = self.escape_techniques.read().ok()?;
        
        if let Some(patterns) = techniques.get(&EscapeTechnique::ProcfsEscape) {
            for pattern in patterns {
                if path.contains(pattern) {
                    let attempt = EscapeAttempt {
                        technique: EscapeTechnique::ProcfsEscape,
                        pid,
                        evidence: vec![format!("Accessed: {}", path)],
                        severity: 8,
                        timestamp: chrono::Utc::now(),
                        blocked: true,
                    };
                    if let Ok(mut attempts) = self.detected_attempts.write() {
                        attempts.push(attempt.clone());
                    }
                    return Some(attempt);
                }
            }
        }
        
        if let Some(patterns) = techniques.get(&EscapeTechnique::ContainerEscape) {
            for pattern in patterns {
                if path.contains(pattern) {
                    let attempt = EscapeAttempt {
                        technique: EscapeTechnique::ContainerEscape,
                        pid,
                        evidence: vec![format!("Container indicator found: {}", path)],
                        severity: 10,
                        timestamp: chrono::Utc::now(),
                        blocked: true,
                    };
                    if let Ok(mut attempts) = self.detected_attempts.write() {
                        attempts.push(attempt.clone());
                    }
                    return Some(attempt);
                }
            }
        }
        
        None
    }

    pub fn get_detected_attempts(&self) -> Vec<EscapeAttempt> {
        self.detected_attempts.read().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn clear_attempts(&self) {
        if let Ok(mut attempts) = self.detected_attempts.write() {
            attempts.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syscall_monitor_enable_disable() {
        let monitor = SyscallMonitor::new();
        assert!(monitor.is_enabled());
        monitor.disable();
        assert!(!monitor.is_enabled());
        monitor.enable();
        assert!(monitor.is_enabled());
    }

    #[test]
    fn test_syscall_monitor_blacklist() {
        let monitor = SyscallMonitor::new();
        
        let event = SyscallEvent {
            pid: 1234,
            syscall: "ptrace".to_string(),
            category: SyscallCategory::Process,
            args: vec![],
            return_value: 0,
            timestamp: chrono::Utc::now(),
            suspicious: false,
        };
        
        let anomaly = monitor.record_syscall(event);
        assert!(anomaly.is_some());
        assert_eq!(anomaly.unwrap().severity, 8);
    }

    #[test]
    fn test_syscall_monitor_whitelist_overrides() {
        let monitor = SyscallMonitor::new();
        monitor.add_to_whitelist("ptrace");
        
        let event = SyscallEvent {
            pid: 1234,
            syscall: "ptrace".to_string(),
            category: SyscallCategory::Process,
            args: vec![],
            return_value: 0,
            timestamp: chrono::Utc::now(),
            suspicious: false,
        };
        
        let anomaly = monitor.record_syscall(event);
        assert!(anomaly.is_none());
    }

    #[test]
    fn test_syscall_monitor_get_recent() {
        let monitor = SyscallMonitor::new();
        
        for i in 0..5 {
            let event = SyscallEvent {
                pid: i,
                syscall: "read".to_string(),
                category: SyscallCategory::File,
                args: vec![],
                return_value: 0,
                timestamp: chrono::Utc::now(),
                suspicious: false,
            };
            monitor.record_syscall(event);
        }
        
        let recent = monitor.get_recent_events(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_network_monitor_enable_disable() {
        let monitor = NetworkMonitor::new();
        assert!(monitor.is_enabled());
        monitor.disable();
        assert!(!monitor.is_enabled());
    }

    #[test]
    fn test_network_monitor_suspicious_port() {
        let monitor = NetworkMonitor::new();
        
        let event = NetworkEvent {
            event_type: NetworkEventType::Connection,
            pid: 1234,
            remote_addr: Some("192.168.1.100".to_string()),
            local_addr: Some("192.168.1.1".to_string()),
            port: Some(4444),
            bytes_sent: 0,
            bytes_received: 0,
            timestamp: chrono::Utc::now(),
            suspicious: false,
        };
        
        let pattern = monitor.record_connection(event);
        assert!(pattern.is_some());
        assert_eq!(pattern.unwrap().severity, 7);
    }

    #[test]
    fn test_network_monitor_get_recent() {
        let monitor = NetworkMonitor::new();
        
        for i in 0..3 {
            let event = NetworkEvent {
                event_type: NetworkEventType::Connection,
                pid: i,
                remote_addr: None,
                local_addr: None,
                port: Some(80),
                bytes_sent: 100,
                bytes_received: 200,
                timestamp: chrono::Utc::now(),
                suspicious: false,
            };
            monitor.record_connection(event);
        }
        
        let recent = monitor.get_recent_connections(2);
        assert_eq!(recent.len(), 2);
    }

    #[test]
    fn test_sandbox_escape_detector_enable_disable() {
        let detector = SandboxEscapeDetector::new();
        assert!(detector.is_enabled());
        detector.disable();
        assert!(!detector.is_enabled());
    }

    #[test]
    fn test_sandbox_escape_detector_procfs() {
        let detector = SandboxEscapeDetector::new();
        
        let attempt = detector.check_file_access("/proc/self/mem", 1234);
        assert!(attempt.is_some());
        assert_eq!(attempt.unwrap().technique, EscapeTechnique::ProcfsEscape);
    }

    #[test]
    fn test_sandbox_escape_detector_container() {
        let detector = SandboxEscapeDetector::new();
        
        let attempt = detector.check_file_access("/.dockerenv", 1234);
        assert!(attempt.is_some());
        assert_eq!(attempt.unwrap().technique, EscapeTechnique::ContainerEscape);
    }

    #[test]
    fn test_sandbox_escape_detector_syscall() {
        let detector = SandboxEscapeDetector::new();
        
        let event = SyscallEvent {
            pid: 1234,
            syscall: "ptrace".to_string(),
            category: SyscallCategory::Process,
            args: vec!["ptrace".to_string()],
            return_value: 0,
            timestamp: chrono::Utc::now(),
            suspicious: false,
        };
        
        let attempt = detector.check_syscall(&event);
        assert!(attempt.is_some());
        assert_eq!(attempt.unwrap().technique, EscapeTechnique::SyscallBypass);
    }

    #[test]
    fn test_sandbox_escape_detector_clear() {
        let detector = SandboxEscapeDetector::new();
        
        detector.check_file_access("/proc/self/mem", 1234);
        assert_eq!(detector.get_detected_attempts().len(), 1);
        
        detector.clear_attempts();
        assert_eq!(detector.get_detected_attempts().len(), 0);
    }

    #[test]
    fn test_severity_ordering() {
        let monitor = SyscallMonitor::new();
        
        let event = SyscallEvent {
            pid: 1234,
            syscall: "perf_event_open".to_string(),
            category: SyscallCategory::Process,
            args: vec![],
            return_value: 0,
            timestamp: chrono::Utc::now(),
            suspicious: false,
        };
        
        let anomaly = monitor.record_syscall(event);
        assert!(anomaly.is_some());
        assert!(anomaly.unwrap().severity >= 7);
    }
}
