use async_trait::async_trait;
use kias_common::{Agent, KiasError, Node, NodeStatus, ScheduleResult};
use std::collections::HashMap;

use super::SchedulingAlgorithm;

/// GPU-Aware scheduler: topology-aware placement for GPU workloads.
///
/// Inspired by Volcano scheduler's GPU-binpack and GPU-share strategies.
/// Features:
/// - GPU device type matching (A100, H100, T4, etc.)
/// - GPU memory awareness (custom resource `gpu_memory_mb`)
/// - Topology-aware placement (prefer same GPU type for multi-GPU jobs)
/// - Bin-pack strategy: pack GPUs tightly to minimize fragmentation
/// - Spread strategy: distribute across nodes for resilience
///
/// Node labels used:
/// - `gpu-type`: GPU device model (e.g., "nvidia-a100", "nvidia-h100")
/// - `gpu-memory-mb`: Total GPU memory per device in MB
/// - `gpu-interconnect`: Interconnect type (e.g., "nvlink", "pcie")
/// - `gpu-scheduler-strategy`: "binpack" or "spread" (default: "binpack")
pub struct GpuAwareScheduler {
    /// Default scheduling strategy when not specified by node
    default_strategy: GpuStrategy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuStrategy {
    /// Pack GPUs tightly to minimize fragmentation (default)
    BinPack,
    /// Spread across nodes for resilience
    Spread,
}

/// GPU vendor identification for vendor-specific scheduling decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
}

impl GpuVendor {
    /// Parse vendor from a gpu-type label string (e.g. "nvidia-a100" → Nvidia).
    pub fn from_label(label: &str) -> Option<Self> {
        let lower = label.to_lowercase();
        if lower.starts_with("nvidia") {
            Some(GpuVendor::Nvidia)
        } else if lower.starts_with("amd") || lower.starts_with("mi") {
            Some(GpuVendor::Amd)
        } else if lower.starts_with("intel") || lower.starts_with("ponte-vecchio") {
            Some(GpuVendor::Intel)
        } else {
            None
        }
    }

    /// Base scoring weight for this vendor (reflects ecosystem maturity / driver quality).
    pub fn score_weight(&self) -> f64 {
        match self {
            GpuVendor::Nvidia => 1.0,
            GpuVendor::Amd => 0.85,
            GpuVendor::Intel => 0.75,
        }
    }
}

impl GpuAwareScheduler {
    pub fn new() -> Self {
        Self {
            default_strategy: GpuStrategy::BinPack,
        }
    }

    pub fn with_strategy(strategy: GpuStrategy) -> Self {
        Self {
            default_strategy: strategy,
        }
    }
}

impl Default for GpuAwareScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU topology info extracted from node labels
#[derive(Debug, Clone)]
struct GpuTopology {
    gpu_type: Option<String>,
    gpu_memory_mb: Option<u64>,
    interconnect: Option<String>,
    vendor: Option<GpuVendor>,
    mig_enabled: bool,
    mig_profile: Option<String>,
}

impl GpuTopology {
    fn from_labels(labels: &HashMap<String, String>) -> Self {
        let gpu_type = labels.get("gpu-type").cloned();
        let vendor = gpu_type.as_deref().and_then(GpuVendor::from_label);
        Self {
            gpu_type,
            gpu_memory_mb: labels
                .get("gpu-memory-mb")
                .and_then(|v| v.parse::<u64>().ok()),
            interconnect: labels.get("gpu-interconnect").cloned(),
            vendor,
            mig_enabled: labels
                .get("gpu-mig-enabled")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            mig_profile: labels.get("gpu-mig-profile").cloned(),
        }
    }
}

/// Score a node for GPU scheduling.
/// Returns (score, reason) or None if the node cannot satisfy the request.
fn score_gpu_node(
    node: &Node,
    request: &kias_common::Resources,
    agent: &Agent,
    strategy: GpuStrategy,
) -> Option<(f64, String)> {
    // Basic eligibility: must be Ready and have enough resources
    if node.status != NodeStatus::Ready {
        return None;
    }
    if !node.available_resources.can_satisfy(request) {
        return None;
    }

    // Must have GPUs available
    if node.available_resources.gpu == 0 {
        return None;
    }

    let topology = GpuTopology::from_labels(&node.labels);
    let mut score = 0.0;
    let mut reasons = Vec::new();

    // ── 1. GPU type matching (via agent affinity or labels) ──
    if let Some(required_type) = agent
        .affinity
        .as_ref()
        .and_then(|a| a.required.get("gpu-type"))
    {
        match topology.gpu_type {
            Some(ref t) if *t == *required_type => {
                score += 30.0;
                reasons.push("gpu-type-match".into());
            }
            _ => return None, // Hard constraint: must match
        }
    }

    // ── 2. GPU memory fitness ──
    // Check if agent requests gpu_memory_mb via custom resources
    let requested_gpu_mem = request.custom.get("gpu_memory_mb").copied().unwrap_or(0.0);
    if requested_gpu_mem > 0.0 {
        if let Some(node_gpu_mem) = topology.gpu_memory_mb {
            // Per-device check: each GPU must have enough memory
            if (node_gpu_mem as f64) < requested_gpu_mem {
                return None;
            }
            let total_gpu_mem = node_gpu_mem * node.total_resources.gpu as u64;
            // Prefer tighter fit (more memory requested vs available = better packing)
            let mem_ratio = requested_gpu_mem / total_gpu_mem as f64;
            score += mem_ratio * 20.0;
            reasons.push("gpu-mem-fit".into());
        }
    }

    // ── 3. GPU count fitness ──
    let requested_gpus = request.gpu;
    let avail_gpus = node.available_resources.gpu;

    match strategy {
        GpuStrategy::BinPack => {
            // Prefer nodes where we use most of the GPUs (tight packing)
            let gpu_utilization = requested_gpus as f64 / avail_gpus as f64;
            score += gpu_utilization * 25.0;
            reasons.push(format!("binpack({}/{})", requested_gpus, avail_gpus));
        }
        GpuStrategy::Spread => {
            // Prefer nodes with more available GPUs (spread load)
            let spread_score = avail_gpus as f64 / node.total_resources.gpu as f64;
            score += spread_score * 25.0;
            reasons.push(format!("spread({} avail)", avail_gpus));
        }
    }

    // ── 4. NVLink preference for multi-GPU ──
    if requested_gpus > 1 {
        if let Some(ref conn) = topology.interconnect {
            if conn.contains("nvlink") || conn.contains("NVLink") {
                score += 15.0;
                reasons.push("nvlink-preferred".into());
            }
        }
    }

    // ── 5. Load balancing factor ──
    let load = node.load_factor();
    score += (1.0 - load) * 10.0;
    reasons.push(format!("load={:.1}%", load * 100.0));

    // ── 5b. Vendor-specific scoring ──
    if let Some(vendor) = topology.vendor {
        let vendor_weight = vendor.score_weight();
        let vendor_score = vendor_weight * 10.0;
        score += vendor_score;
        reasons.push(format!("vendor({:?}, w={:.2})", vendor, vendor_weight));
    }

    // ── 5c. MIG (Multi-Instance GPU) preference ──
    // Agents requesting fractional GPU resources benefit from MIG-capable nodes.
    let wants_mig = agent
        .affinity
        .as_ref()
        .and_then(|a| a.preferred.iter().find(|p| p.label == "gpu-mig-enabled"))
        .map(|p| p.value == "true" || p.value == "1")
        .unwrap_or(false);
    if wants_mig && topology.mig_enabled {
        score += 8.0;
        reasons.push("mig-enabled".into());
        if topology.mig_profile.is_some() {
            score += 2.0;
            reasons.push("mig-profiled".into());
        }
    }

    // ── 6. CPU/GPU co-scheduling fitness ──
    // Prefer nodes where CPU/GPU ratio matches request
    if node.total_resources.gpu > 0 {
        let node_cpu_per_gpu = node.total_resources.cpu / node.total_resources.gpu as f64;
        let request_cpu_per_gpu = if request.gpu > 0 {
            request.cpu / request.gpu as f64
        } else {
            request.cpu
        };
        // Closer ratio = better fit
        let ratio_diff = (node_cpu_per_gpu - request_cpu_per_gpu).abs();
        let ratio_score = 10.0 / (1.0 + ratio_diff);
        score += ratio_score;
        reasons.push("cpu-gpu-ratio".into());
    }

    Some((score, reasons.join(", ")))
}

#[async_trait]
impl SchedulingAlgorithm for GpuAwareScheduler {
    fn name(&self) -> &str {
        "gpu-aware"
    }

    async fn schedule(&self, agent: &Agent, nodes: &[Node]) -> Result<ScheduleResult, KiasError> {
        // Determine strategy: check agent labels first, then default
        let strategy = agent
            .affinity
            .as_ref()
            .and_then(|a| {
                a.preferred
                    .iter()
                    .find(|p| p.label == "gpu-scheduler-strategy")
            })
            .and_then(|p| match p.value.as_str() {
                "spread" => Some(GpuStrategy::Spread),
                "binpack" => Some(GpuStrategy::BinPack),
                _ => None,
            })
            .unwrap_or(self.default_strategy);

        let mut best_node: Option<&Node> = None;
        let mut best_score = f64::NEG_INFINITY;
        let mut best_reason = String::new();

        for node in nodes {
            if let Some((score, reason)) =
                score_gpu_node(node, &agent.resource_request, agent, strategy)
            {
                if score > best_score {
                    best_score = score;
                    best_node = Some(node);
                    best_reason = reason;
                }
            }
        }

        let selected = best_node.ok_or_else(|| {
            KiasError::InsufficientResources(format!(
                "No GPU node can satisfy agent {} request: gpu={}, cpu={}, mem={}",
                agent.id,
                agent.resource_request.gpu,
                agent.resource_request.cpu,
                agent.resource_request.memory_bytes
            ))
        })?;

        tracing::info!(
            agent_id = %agent.id,
            node_id = %selected.id,
            score = best_score,
            reason = %best_reason,
            strategy = ?strategy,
            algorithm = "gpu-aware",
            "Agent scheduled to GPU node"
        );

        Ok(ScheduleResult {
            agent_id: agent.id.clone(),
            node_id: selected.id.clone(),
            algorithm: "gpu-aware".to_string(),
            score: best_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kias_common::{Affinity, Resources};

    fn make_gpu_node(
        id: &str,
        total_gpu: u32,
        avail_gpu: u32,
        cpu: f64,
        mem: u64,
        labels: HashMap<String, String>,
    ) -> Node {
        Node {
            id: id.to_string(),
            status: NodeStatus::Ready,
            total_resources: Resources {
                cpu,
                memory_bytes: mem,
                gpu: total_gpu,
                ..Default::default()
            },
            available_resources: Resources {
                cpu,
                memory_bytes: mem,
                gpu: avail_gpu,
                ..Default::default()
            },
            allocated_agents: vec![],
            labels,
        }
    }

    fn make_gpu_agent(id: &str, gpu: u32, cpu: f64, mem: u64) -> Agent {
        Agent {
            id: id.to_string(),
            name: id.to_string(),
            resource_request: Resources {
                cpu,
                memory_bytes: mem,
                gpu,
                ..Default::default()
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        }
    }

    #[tokio::test]
    async fn test_selects_gpu_node() {
        let mut labels = HashMap::new();
        labels.insert("gpu-type".into(), "nvidia-a100".into());

        let nodes = vec![
            make_gpu_node(
                "cpu-only",
                0,
                0,
                16.0,
                32 * 1024 * 1024 * 1024,
                HashMap::new(),
            ),
            make_gpu_node("gpu-node", 4, 4, 32.0, 64 * 1024 * 1024 * 1024, labels),
        ];

        let scheduler = GpuAwareScheduler::new();
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        assert_eq!(result.node_id, "gpu-node");
        assert_eq!(result.algorithm, "gpu-aware");
    }

    #[tokio::test]
    async fn test_rejects_no_gpu_nodes() {
        let nodes = vec![
            make_gpu_node(
                "cpu-only-1",
                0,
                0,
                16.0,
                32 * 1024 * 1024 * 1024,
                HashMap::new(),
            ),
            make_gpu_node(
                "cpu-only-2",
                0,
                0,
                8.0,
                16 * 1024 * 1024 * 1024,
                HashMap::new(),
            ),
        ];

        let scheduler = GpuAwareScheduler::new();
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await;

        assert!(matches!(result, Err(KiasError::InsufficientResources(_))));
    }

    #[tokio::test]
    async fn test_binpack_prefers_tight_fit() {
        let mut labels_a = HashMap::new();
        labels_a.insert("gpu-type".into(), "nvidia-a100".into());

        let mut labels_b = HashMap::new();
        labels_b.insert("gpu-type".into(), "nvidia-a100".into());

        // Node A: 8 GPUs available (would use 1/8)
        // Node B: 2 GPUs available (would use 1/2 — tighter pack)
        let nodes = vec![
            make_gpu_node("loose", 8, 8, 64.0, 128 * 1024 * 1024 * 1024, labels_a),
            make_gpu_node("tight", 2, 2, 16.0, 32 * 1024 * 1024 * 1024, labels_b),
        ];

        let scheduler = GpuAwareScheduler::with_strategy(GpuStrategy::BinPack);
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        // BinPack should prefer the tighter fit
        assert_eq!(result.node_id, "tight");
    }

    #[tokio::test]
    async fn test_spread_prefers_more_available() {
        let mut labels_a = HashMap::new();
        labels_a.insert("gpu-type".into(), "nvidia-h100".into());

        let mut labels_b = HashMap::new();
        labels_b.insert("gpu-type".into(), "nvidia-h100".into());

        // Node A: 1 GPU available out of 4 (more loaded)
        // Node B: 3 GPUs available out of 4 (more free)
        let nodes = vec![
            make_gpu_node("loaded", 4, 1, 32.0, 64 * 1024 * 1024 * 1024, labels_a),
            make_gpu_node("free", 4, 3, 32.0, 64 * 1024 * 1024 * 1024, labels_b),
        ];

        let scheduler = GpuAwareScheduler::with_strategy(GpuStrategy::Spread);
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        // Spread should prefer the node with more free GPUs
        assert_eq!(result.node_id, "free");
    }

    #[tokio::test]
    async fn test_gpu_type_affinity_hard_constraint() {
        let mut labels_a = HashMap::new();
        labels_a.insert("gpu-type".into(), "nvidia-t4".into());

        let mut labels_b = HashMap::new();
        labels_b.insert("gpu-type".into(), "nvidia-a100".into());

        let nodes = vec![
            make_gpu_node("t4-node", 4, 4, 16.0, 32 * 1024 * 1024 * 1024, labels_a),
            make_gpu_node("a100-node", 8, 8, 64.0, 128 * 1024 * 1024 * 1024, labels_b),
        ];

        let mut required = HashMap::new();
        required.insert("gpu-type".to_string(), "nvidia-a100".to_string());

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 1,
                ..Default::default()
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required,
                preferred: vec![],
            }),
            anti_affinity: None,
            tenant_id: None,
        };

        let scheduler = GpuAwareScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        // Must select a100-node due to hard affinity
        assert_eq!(result.node_id, "a100-node");
    }

    #[tokio::test]
    async fn test_multi_gpu_prefers_nvlink() {
        let mut labels_nvlink = HashMap::new();
        labels_nvlink.insert("gpu-type".into(), "nvidia-a100".into());
        labels_nvlink.insert("gpu-interconnect".into(), "nvlink".into());

        let mut labels_pcie = HashMap::new();
        labels_pcie.insert("gpu-type".into(), "nvidia-a100".into());
        labels_pcie.insert("gpu-interconnect".into(), "pcie".into());

        let nodes = vec![
            make_gpu_node(
                "pcie-node",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                labels_pcie,
            ),
            make_gpu_node(
                "nvlink-node",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                labels_nvlink,
            ),
        ];

        let scheduler = GpuAwareScheduler::new();
        // Request 2 GPUs — should prefer NVLink
        let agent = make_gpu_agent("a1", 2, 8.0, 16 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        assert_eq!(result.node_id, "nvlink-node");
    }

    #[tokio::test]
    async fn test_insufficient_gpu_memory() {
        let mut labels = HashMap::new();
        labels.insert("gpu-type".into(), "nvidia-t4".into());
        labels.insert("gpu-memory-mb".into(), "16000".into()); // 16GB per GPU

        let nodes = vec![make_gpu_node(
            "t4-node",
            4,
            4,
            16.0,
            32 * 1024 * 1024 * 1024,
            labels,
        )];

        let scheduler = GpuAwareScheduler::new();
        // Request 40GB GPU memory but only 16GB per T4
        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 1,
                custom: {
                    let mut m = HashMap::new();
                    m.insert("gpu_memory_mb".to_string(), 40000.0);
                    m
                },
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: None,
            anti_affinity: None,
            tenant_id: None,
        };

        let result = scheduler.schedule(&agent, &nodes).await;
        assert!(matches!(result, Err(KiasError::InsufficientResources(_))));
    }

    #[test]
    fn test_gpu_vendor_from_label() {
        assert_eq!(
            GpuVendor::from_label("nvidia-a100"),
            Some(GpuVendor::Nvidia)
        );
        assert_eq!(
            GpuVendor::from_label("NVIDIA-H100"),
            Some(GpuVendor::Nvidia)
        );
        assert_eq!(GpuVendor::from_label("amd-mi250"), Some(GpuVendor::Amd));
        assert_eq!(GpuVendor::from_label("mi300x"), Some(GpuVendor::Amd));
        assert_eq!(
            GpuVendor::from_label("intel-max-1550"),
            Some(GpuVendor::Intel)
        );
        assert_eq!(
            GpuVendor::from_label("ponte-vecchio"),
            Some(GpuVendor::Intel)
        );
        assert_eq!(GpuVendor::from_label("unknown-gpu"), None);
    }

    #[test]
    fn test_gpu_vendor_score_weights() {
        // Nvidia has highest weight, AMD middle, Intel lowest
        assert!(GpuVendor::Nvidia.score_weight() > GpuVendor::Amd.score_weight());
        assert!(GpuVendor::Amd.score_weight() > GpuVendor::Intel.score_weight());
        assert_eq!(GpuVendor::Nvidia.score_weight(), 1.0);
    }

    #[tokio::test]
    async fn test_nvidia_preferred_over_amd_by_vendor_weight() {
        let mut nvidia_labels = HashMap::new();
        nvidia_labels.insert("gpu-type".into(), "nvidia-a100".into());

        let mut amd_labels = HashMap::new();
        amd_labels.insert("gpu-type".into(), "amd-mi250".into());

        let nodes = vec![
            make_gpu_node(
                "nvidia-node",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                nvidia_labels,
            ),
            make_gpu_node("amd-node", 4, 4, 32.0, 64 * 1024 * 1024 * 1024, amd_labels),
        ];

        let scheduler = GpuAwareScheduler::new();
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();

        // Nvidia vendor weight (1.0) > AMD (0.85) → Nvidia preferred
        assert_eq!(result.node_id, "nvidia-node");
    }

    #[tokio::test]
    async fn test_mig_enabled_node_preferred() {
        let mut labels_mig = HashMap::new();
        labels_mig.insert("gpu-type".into(), "nvidia-a100".into());
        labels_mig.insert("gpu-mig-enabled".into(), "true".into());

        let mut labels_no_mig = HashMap::new();
        labels_no_mig.insert("gpu-type".into(), "nvidia-a100".into());

        let nodes = vec![
            make_gpu_node(
                "no-mig-node",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                labels_no_mig,
            ),
            make_gpu_node("mig-node", 4, 4, 32.0, 64 * 1024 * 1024 * 1024, labels_mig),
        ];

        let preferred = vec![kias_common::LabelPreference {
            label: "gpu-mig-enabled".to_string(),
            value: "true".to_string(),
            weight: 1.0,
        }];

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 1,
                ..Default::default()
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: HashMap::new(),
                preferred,
            }),
            anti_affinity: None,
            tenant_id: None,
        };

        let scheduler = GpuAwareScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "mig-node");
    }

    #[tokio::test]
    async fn test_mig_profile_adds_extra_score() {
        let mut labels_mig_profile = HashMap::new();
        labels_mig_profile.insert("gpu-type".into(), "nvidia-a100".into());
        labels_mig_profile.insert("gpu-mig-enabled".into(), "true".into());
        labels_mig_profile.insert("gpu-mig-profile".into(), "1g.5gb".into());

        let mut labels_mig_no_profile = HashMap::new();
        labels_mig_no_profile.insert("gpu-type".into(), "nvidia-a100".into());
        labels_mig_no_profile.insert("gpu-mig-enabled".into(), "true".into());

        let nodes = vec![
            make_gpu_node(
                "mig-basic",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                labels_mig_no_profile,
            ),
            make_gpu_node(
                "mig-profiled",
                4,
                4,
                32.0,
                64 * 1024 * 1024 * 1024,
                labels_mig_profile,
            ),
        ];

        let preferred = vec![kias_common::LabelPreference {
            label: "gpu-mig-enabled".to_string(),
            value: "true".to_string(),
            weight: 1.0,
        }];

        let agent = Agent {
            id: "a1".to_string(),
            name: "a1".to_string(),
            resource_request: Resources {
                cpu: 4.0,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                gpu: 1,
                ..Default::default()
            },
            priority: Default::default(),
            system_prompt_hash: None,
            affinity: Some(Affinity {
                required: HashMap::new(),
                preferred,
            }),
            anti_affinity: None,
            tenant_id: None,
        };

        let scheduler = GpuAwareScheduler::new();
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "mig-profiled");
    }

    #[tokio::test]
    async fn test_amd_mi_series_vendor_parsing() {
        let mut labels = HashMap::new();
        labels.insert("gpu-type".into(), "mi300x".into());

        let nodes = vec![make_gpu_node(
            "amd-node",
            4,
            4,
            32.0,
            64 * 1024 * 1024 * 1024,
            labels,
        )];

        let scheduler = GpuAwareScheduler::new();
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "amd-node");
    }

    #[tokio::test]
    async fn test_intel_ponte_vecchio_scheduling() {
        let mut labels = HashMap::new();
        labels.insert("gpu-type".into(), "ponte-vecchio".into());

        let nodes = vec![make_gpu_node(
            "intel-node",
            2,
            2,
            16.0,
            32 * 1024 * 1024 * 1024,
            labels,
        )];

        let scheduler = GpuAwareScheduler::new();
        let agent = make_gpu_agent("a1", 1, 4.0, 8 * 1024 * 1024 * 1024);
        let result = scheduler.schedule(&agent, &nodes).await.unwrap();
        assert_eq!(result.node_id, "intel-node");
    }
}
