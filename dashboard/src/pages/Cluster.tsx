// Cluster page — detailed cluster status

import { usePolling } from '../hooks/useApi';
import { getClusterStatus } from '../api/client';
import { StatusBadge, Spinner, ErrorBanner, StatCard } from '../components/Common';
import type { ClusterStatus } from '../types';

export default function ClusterPage() {
  const { data, loading, error, refetch } = usePolling<ClusterStatus>(getClusterStatus, 5000);

  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;

  const cluster = data!;

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Cluster</h1>
          <p className="text-sm text-slate-400 mt-1">Cluster topology and health</p>
        </div>
        <StatusBadge status={cluster.overall} />
      </div>

      {/* Stats */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <StatCard label="Total Nodes" value={cluster.nodes.length} icon="🖥️" color="blue" />
        <StatCard label="Total Agents" value={cluster.total_agents} icon="🤖" color="purple" />
        <StatCard label="Running Agents" value={cluster.running_agents} icon="▶️" color="green" />
      </div>

      {/* Node details */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 overflow-hidden">
        <div className="px-5 py-4 border-b border-slate-700">
          <h2 className="text-lg font-semibold text-white">Node Details</h2>
        </div>
        <table className="w-full">
          <thead>
            <tr className="border-b border-slate-700">
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Node</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Status</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">CPU</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Memory</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">GPU</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-700/50">
            {cluster.nodes.map(node => (
              <tr key={node.id} className="hover:bg-slate-700/20 transition-colors">
                <td className="px-5 py-3">
                  <span className="text-sm font-medium text-white">{node.name}</span>
                </td>
                <td className="px-5 py-3">
                  <StatusBadge status={node.status} />
                </td>
                <td className="px-5 py-3">
                  <span className="text-sm text-slate-300">{node.cpu}</span>
                </td>
                <td className="px-5 py-3">
                  <span className="text-sm text-slate-300">{node.memory}</span>
                </td>
                <td className="px-5 py-3">
                  <span className="text-sm text-slate-300">{node.gpu}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
