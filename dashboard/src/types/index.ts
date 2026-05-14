// KIAS API TypeScript Types
// Mirrors the Rust backend models in crates/api-server/src/models/

// ── Agent ──────────────────────────────────────────────────────────────────

export type AgentStatus = 'Pending' | 'Scheduled' | 'Running' | 'Succeeded' | 'Failed' | 'Unknown';

export interface ResourceRequest {
  cpu?: string;
  memory?: string;
  gpu?: string;
}

export interface AgentSpec {
  name: string;
  image: string;
  command: string[];
  resource_request?: ResourceRequest;
  labels: Record<string, string>;
  priority: string;
  env: Record<string, string>;
}

export interface Agent {
  id: string;
  spec: AgentSpec;
  status: AgentStatus;
  node_id?: string;
  resource_usage: ResourceRequest;
  created_at: string;
  updated_at: string;
  start_time?: string;
  restart_count: number;
}

export interface AgentSummary {
  id: string;
  name: string;
  status: AgentStatus;
  node_id?: string;
}

// ── Node ───────────────────────────────────────────────────────────────────

export type NodeStatus = 'Ready' | 'NotReady' | 'Unknown';

export interface ResourceCapacity {
  cpu: string;
  memory: string;
  gpu: string;
}

export interface Node {
  id: string;
  name: string;
  status: NodeStatus;
  resources: ResourceCapacity;
  allocatable: ResourceCapacity;
  labels: Record<string, string>;
  created_at: string;
  last_heartbeat: string;
}

// ── Health ─────────────────────────────────────────────────────────────────

export interface ComponentHealth {
  name: string;
  status: string;
}

export interface HealthResponse {
  status: string;
  version: string;
  components: ComponentHealth[];
}

// ── Metrics ────────────────────────────────────────────────────────────────

export interface TaskStats {
  pending: number;
  scheduled: number;
  running: number;
  succeeded: number;
  failed: number;
  unknown: number;
}

export interface MetricsSummary {
  agent_count: number;
  node_count: number;
  task_stats: TaskStats;
}

export interface AgentMetrics {
  id: string;
  name: string;
  status: AgentStatus;
  node_id?: string;
  restart_count: number;
  created_at: string;
  updated_at: string;
  start_time?: string;
}

export interface NodeHealth {
  id: string;
  name: string;
  status: string;
  cpu: string;
  memory: string;
  gpu: string;
}

export interface ClusterStatus {
  overall: string;
  nodes: NodeHealth[];
  total_agents: number;
  running_agents: number;
}

// ── Token Analytics ────────────────────────────────────────────────────────

export interface TokenUsage {
  agent_id: string;
  agent_name: string;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
  estimated_cost: number;
  request_count: number;
}

export interface TokenTimeSeries {
  timestamp: string;
  input_tokens: number;
  output_tokens: number;
  total_tokens: number;
}

export interface TokenAnalytics {
  total_input_tokens: number;
  total_output_tokens: number;
  total_tokens: number;
  total_cost: number;
  total_requests: number;
  per_agent: TokenUsage[];
  time_series: TokenTimeSeries[];
}

// ── Workflows ──────────────────────────────────────────────────────────────

export type WorkflowStatus = 'Draft' | 'Running' | 'Completed' | 'Failed' | 'Cancelled';

export interface WorkflowNode {
  id: string;
  name: string;
  node_type: string;
  config: Record<string, unknown>;
  dependencies: string[];
}

export interface Workflow {
  id: string;
  name: string;
  description: string;
  status: WorkflowStatus;
  nodes: WorkflowNode[];
  created_at: string;
  updated_at: string;
  started_at?: string;
  completed_at?: string;
  execution_count: number;
}

export interface CreateWorkflowRequest {
  name: string;
  description?: string;
  nodes?: WorkflowNode[];
}

export interface WorkflowSummary {
  total: number;
  running: number;
  completed: number;
  failed: number;
  draft: number;
  workflows: Workflow[];
}

// ── Scheduler ──────────────────────────────────────────────────────────────

export interface SchedulerAlgorithm {
  name: string;
  description: string;
}

export interface QueueDepth {
  pending: number;
  scheduled: number;
  running: number;
}

export interface SchedulingThroughput {
  total_scheduled: number;
  total_completed: number;
  total_failed: number;
  success_rate: number;
  avg_restart_count: number;
}

export interface NodeUtilization {
  node_id: string;
  node_name: string;
  agent_count: number;
  running_count: number;
  status: string;
}

export interface SchedulingDecision {
  agent_id: string;
  agent_name: string;
  assigned_node: string | null;
  status: string;
  priority: string;
  timestamp: string;
}

export interface SchedulerStatus {
  current_algorithm: SchedulerAlgorithm;
  queue_depth: QueueDepth;
  throughput: SchedulingThroughput;
  node_utilization: NodeUtilization[];
  recent_decisions: SchedulingDecision[];
}

// ── API Response Envelopes ─────────────────────────────────────────────────

export interface ApiResponse<T> {
  data: T;
}

export interface ListResponse<T> {
  items: T[];
  total: number;
}

export interface ActionResponse {
  message: string;
}

export interface ApiError {
  error: {
    code: number;
    message: string;
  };
}

// ── Pagination ─────────────────────────────────────────────────────────────

export interface PaginationParams {
  page?: number;
  per_page?: number;
}
