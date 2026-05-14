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
