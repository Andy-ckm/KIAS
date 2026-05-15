// Agent Detail page — real-time status, log streaming, resource usage charts

import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useApi, usePolling } from '../hooks/useApi';
import {
  getAgent,
  getAgentStatusHistory,
  getAgentLogs,
  getAgentResourceHistory,
} from '../api/client';
import { StatusBadge, Spinner, ErrorBanner } from '../components/Common';
import type {
  ApiResponse,
  Agent,
  StatusTransition,
  LogEntry,
  AgentResourceHistory,
  ListResponse,
} from '../types';
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
  AreaChart,
  Area,
} from 'recharts';

// ── Helpers ────────────────────────────────────────────────────────────────

function formatTime(ts: string): string {
  try {
    return new Date(ts).toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    });
  } catch {
    return ts;
  }
}

function formatDate(ts: string): string {
  try {
    return new Date(ts).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return ts;
  }
}

function shortId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 8)}…${id.slice(-4)}` : id;
}

const LOG_LEVEL_COLORS: Record<string, string> = {
  debug: 'text-slate-400',
  info: 'text-blue-400',
  warn: 'text-yellow-400',
  error: 'text-red-400',
};

const LOG_LEVEL_BG: Record<string, string> = {
  debug: 'bg-slate-500/10',
  info: 'bg-blue-500/10',
  warn: 'bg-yellow-500/10',
  error: 'bg-red-500/10',
};

const STATUS_TIMELINE_COLORS: Record<string, string> = {
  Pending: 'border-yellow-500 bg-yellow-500/20',
  Scheduled: 'border-blue-500 bg-blue-500/20',
  Running: 'border-green-500 bg-green-500/20',
  Succeeded: 'border-emerald-500 bg-emerald-500/20',
  Failed: 'border-red-500 bg-red-500/20',
  Unknown: 'border-slate-500 bg-slate-500/20',
};

// ── WebSocket Log Hook ─────────────────────────────────────────────────────

function useLogStream(agentId: string | undefined) {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    if (!agentId) return;

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws`;

    let ws: WebSocket;
    let reconnectTimer: ReturnType<typeof setTimeout>;

    function connect() {
      ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        setConnected(true);
        ws.send(
          JSON.stringify({
            subscribe: ['agent_log', 'agent_status_change'],
            filter: { agent_id: agentId },
          })
        );
      };

      ws.onmessage = (event: MessageEvent) => {
        try {
          const data = JSON.parse(event.data as string) as {
            event: string;
            data: Record<string, unknown>;
            timestamp: string;
          };
          if (data.event === 'agent_log') {
            const logEntry: LogEntry = {
              timestamp: data.timestamp,
              level: (data.data['level'] as LogEntry['level']) ?? 'info',
              source: (data.data['source'] as string) ?? 'agent',
              message: (data.data['message'] as string) ?? '',
              agent_id: data.data['agent_id'] as string | undefined,
            };
            setLogs(prev => [...prev.slice(-499), logEntry]);
          }
        } catch {
          // ignore parse errors from heartbeat/ping
        }
      };

      ws.onclose = () => {
        setConnected(false);
        reconnectTimer = setTimeout(connect, 3000);
      };

      ws.onerror = () => {
        ws.close();
      };
    }

    connect();

    return () => {
      clearTimeout(reconnectTimer);
      ws.close();
    };
  }, [agentId]);

  const clearLogs = useCallback(() => setLogs([]), []);

  return { logs, connected, clearLogs };
}

// ── Sub-components ─────────────────────────────────────────────────────────

function AgentInfoCard({ agent }: { agent: Agent }) {
  return (
    <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-6">
      <div className="flex items-start justify-between mb-4">
        <div>
          <h2 className="text-xl font-bold text-white">{agent.spec.name}</h2>
          <p className="text-xs font-mono text-slate-500 mt-1">{agent.id}</p>
        </div>
        <StatusBadge status={agent.status} />
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <div>
          <p className="text-xs text-slate-400">Image</p>
          <p className="text-sm text-white font-medium mt-0.5">{agent.spec.image}</p>
        </div>
        <div>
          <p className="text-xs text-slate-400">Priority</p>
          <p className="text-sm text-white font-medium mt-0.5 capitalize">{agent.spec.priority}</p>
        </div>
        <div>
          <p className="text-xs text-slate-400">Node</p>
          <p className="text-sm text-white font-medium mt-0.5">{agent.node_id ?? '—'}</p>
        </div>
        <div>
          <p className="text-xs text-slate-400">Restarts</p>
          <p className="text-sm text-white font-medium mt-0.5">{agent.restart_count}</p>
        </div>
        <div>
          <p className="text-xs text-slate-400">CPU</p>
          <p className="text-sm text-white font-medium mt-0.5">
            {agent.resource_usage.cpu ?? '—'}
          </p>
        </div>
        <div>
          <p className="text-xs text-slate-400">Memory</p>
          <p className="text-sm text-white font-medium mt-0.5">
            {agent.resource_usage.memory ?? '—'}
          </p>
        </div>
        <div>
          <p className="text-xs text-slate-400">GPU</p>
          <p className="text-sm text-white font-medium mt-0.5">
            {agent.resource_usage.gpu ?? '—'}
          </p>
        </div>
        <div>
          <p className="text-xs text-slate-400">Started</p>
          <p className="text-sm text-white font-medium mt-0.5">
            {agent.start_time ? formatDate(agent.start_time) : '—'}
          </p>
        </div>
      </div>

      {/* Labels */}
      {Object.keys(agent.spec.labels).length > 0 && (
        <div className="mt-4 pt-4 border-t border-slate-700">
          <p className="text-xs text-slate-400 mb-2">Labels</p>
          <div className="flex flex-wrap gap-2">
            {Object.entries(agent.spec.labels).map(([key, value]) => (
              <span
                key={key}
                className="inline-flex items-center px-2 py-0.5 rounded-md text-xs bg-slate-700/50 text-slate-300 border border-slate-600"
              >
                {key}={value}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Environment */}
      {Object.keys(agent.spec.env).length > 0 && (
        <div className="mt-4 pt-4 border-t border-slate-700">
          <p className="text-xs text-slate-400 mb-2">Environment</p>
          <div className="flex flex-wrap gap-2">
            {Object.entries(agent.spec.env).map(([key, value]) => (
              <span
                key={key}
                className="inline-flex items-center px-2 py-0.5 rounded-md text-xs bg-slate-700/50 text-slate-300 border border-slate-600 font-mono"
              >
                {key}={value}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function StatusTimeline({ history }: { history: StatusTransition[] }) {
  if (history.length === 0) {
    return (
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Status Timeline</h3>
        <p className="text-sm text-slate-500">No status transitions recorded yet.</p>
      </div>
    );
  }

  return (
    <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-6">
      <h3 className="text-lg font-semibold text-white mb-4">Status Timeline</h3>
      <div className="relative">
        {/* Vertical line */}
        <div className="absolute left-3 top-0 bottom-0 w-px bg-slate-700" />

        <div className="space-y-4">
          {history.map((transition, i) => {
            const color = STATUS_TIMELINE_COLORS[transition.to] ?? STATUS_TIMELINE_COLORS.Unknown;
            return (
              <div key={i} className="relative flex items-start gap-4 pl-8">
                {/* Dot */}
                <div
                  className={`absolute left-1.5 top-1 w-3 h-3 rounded-full border-2 ${color}`}
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-slate-400">{transition.from}</span>
                    <span className="text-slate-600">→</span>
                    <span className="text-sm font-medium text-white">{transition.to}</span>
                  </div>
                  <div className="flex items-center gap-3 mt-0.5">
                    <span className="text-xs text-slate-500">{formatDate(transition.timestamp)}</span>
                    {transition.reason && (
                      <span className="text-xs text-slate-400">— {transition.reason}</span>
                    )}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function LogStreamPanel({
  initialLogs,
  wsLogs,
  connected,
  onClear,
}: {
  initialLogs: LogEntry[];
  wsLogs: LogEntry[];
  connected: boolean;
  onClear: () => void;
}) {
  const logContainerRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);
  const [filterLevel, setFilterLevel] = useState<string>('all');

  const allLogs = [...initialLogs, ...wsLogs];
  const filteredLogs =
    filterLevel === 'all'
      ? allLogs
      : allLogs.filter(l => l.level === filterLevel);

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoScroll && logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
    }
  }, [filteredLogs.length, autoScroll]);

  return (
    <div className="bg-[#1e293b] rounded-xl border border-slate-700 flex flex-col h-[500px]">
      {/* Header */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-slate-700">
        <div className="flex items-center gap-3">
          <h3 className="text-lg font-semibold text-white">Log Stream</h3>
          <span
            className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-xs ${
              connected
                ? 'bg-green-500/20 text-green-400'
                : 'bg-red-500/20 text-red-400'
            }`}
          >
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                connected ? 'bg-green-400' : 'bg-red-400'
              }`}
            />
            {connected ? 'Live' : 'Disconnected'}
          </span>
          <span className="text-xs text-slate-500">{filteredLogs.length} entries</span>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={filterLevel}
            onChange={e => setFilterLevel(e.target.value)}
            className="bg-slate-800 border border-slate-600 rounded-lg px-2 py-1 text-xs text-slate-300 focus:outline-none focus:border-blue-500"
          >
            <option value="all">All levels</option>
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
          <button
            onClick={() => setAutoScroll(prev => !prev)}
            className={`px-2 py-1 text-xs rounded-lg border transition-colors ${
              autoScroll
                ? 'bg-blue-600/20 border-blue-500/30 text-blue-400'
                : 'bg-slate-800 border-slate-600 text-slate-400'
            }`}
          >
            {autoScroll ? 'Auto-scroll' : 'Manual'}
          </button>
          <button
            onClick={onClear}
            className="px-2 py-1 text-xs bg-slate-800 border border-slate-600 text-slate-400 rounded-lg hover:text-white transition-colors"
          >
            Clear
          </button>
        </div>
      </div>

      {/* Log entries */}
      <div
        ref={logContainerRef}
        className="flex-1 overflow-y-auto overflow-x-hidden p-4 font-mono text-xs leading-relaxed"
      >
        {filteredLogs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-slate-500">
            Waiting for logs…
          </div>
        ) : (
          filteredLogs.map((log, i) => (
            <div
              key={i}
              className={`flex items-start gap-2 py-1 px-2 rounded ${LOG_LEVEL_BG[log.level] ?? ''}`}
            >
              <span className="text-slate-500 shrink-0 w-20">{formatTime(log.timestamp)}</span>
              <span
                className={`shrink-0 w-12 uppercase font-semibold ${LOG_LEVEL_COLORS[log.level] ?? 'text-slate-400'}`}
              >
                {log.level}
              </span>
              <span className="text-slate-400 shrink-0 w-16">[{log.source}]</span>
              <span className="text-slate-200 break-all">{log.message}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

function ResourceCharts({ data }: { data: AgentResourceHistory }) {
  const chartData = data.points.map(p => ({
    ...p,
    time: formatTime(p.timestamp),
  }));

  const tooltipStyle = {
    backgroundColor: '#1e293b',
    border: '1px solid #475569',
    borderRadius: '8px',
  };

  if (chartData.length === 0) {
    return (
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-6">
        <h3 className="text-lg font-semibold text-white mb-4">Resource Usage</h3>
        <div className="flex items-center justify-center h-48 text-slate-500 text-sm">
          No resource data available yet
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* CPU & Memory chart */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
        <h3 className="text-lg font-semibold text-white mb-4">CPU & Memory Usage</h3>
        <ResponsiveContainer width="100%" height={260}>
          <AreaChart data={chartData}>
            <defs>
              <linearGradient id="cpuGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
                <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
              </linearGradient>
              <linearGradient id="memGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
              </linearGradient>
            </defs>
            <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
            <XAxis dataKey="time" stroke="#94a3b8" fontSize={11} />
            <YAxis yAxisId="cpu" stroke="#94a3b8" fontSize={11} domain={[0, 100]} tickFormatter={(v: number) => `${v}%`} />
            <YAxis yAxisId="mem" orientation="right" stroke="#94a3b8" fontSize={11} tickFormatter={(v: number) => `${v}MB`} />
            <Tooltip
              contentStyle={tooltipStyle}
              labelStyle={{ color: '#e2e8f0' }}
              formatter={(value: number, name: string) => [
                name === 'cpu_percent' ? `${value.toFixed(1)}%` : `${value.toFixed(0)} MB`,
                name === 'cpu_percent' ? 'CPU' : 'Memory',
              ]}
            />
            <Legend formatter={(value: string) => <span style={{ color: '#94a3b8' }}>{value === 'cpu_percent' ? 'CPU %' : 'Memory MB'}</span>} />
            <Area yAxisId="cpu" type="monotone" dataKey="cpu_percent" stroke="#3b82f6" fill="url(#cpuGrad)" strokeWidth={2} name="cpu_percent" />
            <Area yAxisId="mem" type="monotone" dataKey="memory_mb" stroke="#10b981" fill="url(#memGrad)" strokeWidth={2} name="memory_mb" />
          </AreaChart>
        </ResponsiveContainer>
      </div>

      {/* Token usage chart */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
        <h3 className="text-lg font-semibold text-white mb-4">Token Usage Over Time</h3>
        <ResponsiveContainer width="100%" height={220}>
          <LineChart data={chartData}>
            <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
            <XAxis dataKey="time" stroke="#94a3b8" fontSize={11} />
            <YAxis stroke="#94a3b8" fontSize={11} />
            <Tooltip
              contentStyle={tooltipStyle}
              labelStyle={{ color: '#e2e8f0' }}
              formatter={(value: number) => [`${value.toLocaleString()} tokens`, 'Tokens']}
            />
            <Line type="monotone" dataKey="token_count" stroke="#f59e0b" strokeWidth={2} dot={false} name="Tokens" />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

// ── Main Page ──────────────────────────────────────────────────────────────

export default function AgentDetailPage() {
  const { id } = useParams<{ id: string }>();

  // Fetch agent detail
  const {
    data: agentResp,
    loading: agentLoading,
    error: agentError,
    refetch: refetchAgent,
  } = useApi<ApiResponse<Agent>>(() => getAgent(id!), [id]);

  // Fetch status history
  const { data: statusHistory } = usePolling<StatusTransition[]>(
    () => getAgentStatusHistory(id!),
    10000,
    [id]
  );

  // Fetch initial logs (REST)
  const { data: logsResp } = useApi<ListResponse<LogEntry>>(
    () => getAgentLogs(id!, { limit: 200 }),
    [id]
  );

  // Fetch resource history
  const { data: resourceHistory } = usePolling<AgentResourceHistory>(
    () => getAgentResourceHistory(id!),
    8000,
    [id]
  );

  // WebSocket live log stream
  const { logs: wsLogs, connected, clearLogs } = useLogStream(id);

  if (!id) {
    return <ErrorBanner message="No agent ID provided" />;
  }
  if (agentLoading) return <Spinner />;
  if (agentError) return <ErrorBanner message={agentError} onRetry={refetchAgent} />;
  if (!agentResp) return null;

  const agent = agentResp.data;

  return (
    <div className="space-y-6">
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm">
        <Link to="/agents" className="text-blue-400 hover:text-blue-300 transition-colors">
          ← Agents
        </Link>
        <span className="text-slate-600">/</span>
        <span className="text-slate-400">{agent.spec.name}</span>
      </div>

      {/* Agent Info Card */}
      <AgentInfoCard agent={agent} />

      {/* Middle row: Status Timeline + Resource Charts */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-1">
          <StatusTimeline history={statusHistory ?? []} />
        </div>
        <div className="lg:col-span-2">
          <ResourceCharts data={resourceHistory ?? { agent_id: id, points: [] }} />
        </div>
      </div>

      {/* Log Stream */}
      <LogStreamPanel
        initialLogs={logsResp?.items ?? []}
        wsLogs={wsLogs}
        connected={connected}
        onClear={clearLogs}
      />

      {/* Metadata footer */}
      <div className="flex items-center justify-between text-xs text-slate-500 pt-2 pb-4">
        <span>Created: {formatDate(agent.created_at)}</span>
        <span>Last Updated: {formatDate(agent.updated_at)}</span>
        <span>Agent ID: {shortId(agent.id)}</span>
      </div>
    </div>
  );
}
