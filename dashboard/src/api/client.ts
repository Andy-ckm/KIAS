// KIAS API Client
// Typed fetch wrapper for the KIAS API Server

import type {
  Agent,
  AgentSpec,
  AgentSummary,
  ApiResponse,
  ListResponse,
  ActionResponse,
  MetricsSummary,
  AgentMetrics,
  ClusterStatus,
  Node,
  HealthResponse,
  PaginationParams,
  ApiError,
} from '../types';

const BASE_URL = '';  // Use Vite proxy in dev

class KiasApiError extends Error {
  status: number;
  apiMessage: string;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'KiasApiError';
    this.status = status;
    this.apiMessage = message;
  }
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const url = `${BASE_URL}${path}`;
  const resp = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
  });

  if (!resp.ok) {
    let message = resp.statusText;
    try {
      const body: ApiError = await resp.json();
      message = body.error?.message || message;
    } catch {
      // ignore parse errors
    }
    throw new KiasApiError(resp.status, message);
  }

  return resp.json();
}

// ── Health ─────────────────────────────────────────────────────────────────

export async function getHealth(): Promise<{ status: string }> {
  return request('/health');
}

export async function getReadiness(): Promise<HealthResponse> {
  return request('/readyz');
}

// ── Agents ─────────────────────────────────────────────────────────────────

export async function listAgents(params?: PaginationParams): Promise<ListResponse<AgentSummary>> {
  const searchParams = new URLSearchParams();
  if (params?.page) searchParams.set('page', String(params.page));
  if (params?.per_page) searchParams.set('per_page', String(params.per_page));
  const qs = searchParams.toString();
  return request(`/api/v1/agents${qs ? `?${qs}` : ''}`);
}

export async function getAgent(id: string): Promise<ApiResponse<Agent>> {
  return request(`/api/v1/agents/${id}`);
}

export async function createAgent(spec: AgentSpec): Promise<ApiResponse<Agent>> {
  return request('/api/v1/agents', {
    method: 'POST',
    body: JSON.stringify(spec),
  });
}

export async function deleteAgent(id: string): Promise<ActionResponse> {
  return request(`/api/v1/agents/${id}`, { method: 'DELETE' });
}

export async function updateAgentStatus(
  id: string,
  status: string
): Promise<ApiResponse<Agent>> {
  return request(`/api/v1/agents/${id}/status`, {
    method: 'PATCH',
    body: JSON.stringify(status),
  });
}

// ── Nodes ──────────────────────────────────────────────────────────────────

export async function listNodes(params?: PaginationParams): Promise<ListResponse<Node>> {
  const searchParams = new URLSearchParams();
  if (params?.page) searchParams.set('page', String(params.page));
  if (params?.per_page) searchParams.set('per_page', String(params.per_page));
  const qs = searchParams.toString();
  return request(`/api/v1/nodes${qs ? `?${qs}` : ''}`);
}

export async function getNode(id: string): Promise<ApiResponse<Node>> {
  return request(`/api/v1/nodes/${id}`);
}

// ── Metrics ────────────────────────────────────────────────────────────────

export async function getMetricsSummary(): Promise<MetricsSummary> {
  return request('/api/v1/metrics/summary');
}

export async function getAgentMetrics(id: string): Promise<AgentMetrics> {
  return request(`/api/v1/metrics/agents/${id}`);
}

export async function getClusterStatus(): Promise<ClusterStatus> {
  return request('/api/v1/cluster/status');
}
