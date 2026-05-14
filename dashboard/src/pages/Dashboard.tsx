// Dashboard overview page — cluster health + metrics

import { usePolling } from '../hooks/useApi';
import { getMetricsSummary, getClusterStatus } from '../api/client';
import { StatusBadge, StatCard, Spinner, ErrorBanner } from '../components/Common';
import type { MetricsSummary, ClusterStatus } from '../types';

export default function DashboardPage() {
  const metrics = usePolling<MetricsSummary>(getMetricsSummary, 5000);
  const cluster = usePolling<ClusterStatus>(getClusterStatus, 5000);

  if (metrics.loading || cluster.loading) return <Spinner />;
  if (metrics.error) return <ErrorBanner message={metrics.error} onRetry={metrics.refetch} />;
  if (cluster.error) return <ErrorBanner message={cluster.error} onRetry={cluster.refetch} />;

  const m = metrics.data!;
  const c = cluster.data!;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Dashboard</h1>
          <p className="text-sm text-slate-400 mt-1">KIAS Cluster Overview</p>
        </div>
        <StatusBadge status={c.overall} />
      </div>

      {/* Stat cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard label="Total Agents" value={m.agent_count} icon="🤖" color="blue" />
        <StatCard label="Running" value={m.task_stats.running} icon="▶️" color="green" />
        <StatCard label="Pending" value={m.task_stats.pending} icon="⏳" color="yellow" />
        <StatCard label="Failed" value={m.task_stats.failed} icon="❌" color="red" />
      </div>

      {/* Nodes */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 overflow-hidden">
        <div className="px-5 py-4 border-b border-slate-700">
          <h2 className="text-lg font-semibold text-white">Nodes</h2>
        </div>
        <div className="divide-y divide-slate-700/50">
          {c.nodes.map(node => (
            <div key={node.id} className="px-5 py-3 flex items-center justify-between hover:bg-slate-700/20 transition-colors">
              <div className="flex items-center gap-3">
                <span className="text-lg">🖥️</span>
                <div>
                  <p className="text-sm font-medium text-white">{node.name}</p>
                  <p className="text-xs text-slate-400">CPU: {node.cpu} · Memory: {node.memory} · GPU: {node.gpu}</p>
                </div>
              </div>
              <StatusBadge status={node.status} />
            </div>
          ))}
          {c.nodes.length === 0 && (
            <div className="px-5 py-8 text-center text-slate-500 text-sm">No nodes registered</div>
          )}
        </div>
      </div>

      {/* Task distribution */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
        <h2 className="text-lg font-semibold text-white mb-4">Task Distribution</h2>
        <div className="grid grid-cols-3 sm:grid-cols-6 gap-4">
          {[
            { label: 'Pending', value: m.task_stats.pending, color: 'text-yellow-400' },
            { label: 'Scheduled', value: m.task_stats.scheduled, color: 'text-blue-400' },
            { label: 'Running', value: m.task_stats.running, color: 'text-green-400' },
            { label: 'Succeeded', value: m.task_stats.succeeded, color: 'text-emerald-400' },
            { label: 'Failed', value: m.task_stats.failed, color: 'text-red-400' },
            { label: 'Unknown', value: m.task_stats.unknown, color: 'text-slate-400' },
          ].map(item => (
            <div key={item.label} className="text-center">
              <p className={`text-2xl font-bold ${item.color}`}>{item.value}</p>
              <p className="text-xs text-slate-400 mt-1">{item.label}</p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
