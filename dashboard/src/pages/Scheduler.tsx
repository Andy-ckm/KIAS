// Scheduler status page — algorithm, queue depth, throughput, node utilization

import { usePolling } from '../hooks/useApi';
import { getSchedulerStatus } from '../api/client';
import { StatCard, StatusBadge, Spinner, ErrorBanner } from '../components/Common';
import type { SchedulerStatus } from '../types';
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  Legend,
} from 'recharts';

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6'];

const PRIORITY_COLORS: Record<string, string> = {
  low: 'text-slate-400',
  medium: 'text-blue-400',
  high: 'text-yellow-400',
  critical: 'text-red-400',
};

function formatPercent(v: number): string {
  return `${v.toFixed(1)}%`;
}

export default function SchedulerPage() {
  const { data, loading, error, refetch } = usePolling<SchedulerStatus>(getSchedulerStatus, 5000);

  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;
  if (!data) return null;

  const queuePieData = [
    { name: 'Pending', value: data.queue_depth.pending, color: '#f59e0b' },
    { name: 'Scheduled', value: data.queue_depth.scheduled, color: '#3b82f6' },
    { name: 'Running', value: data.queue_depth.running, color: '#10b981' },
  ].filter(d => d.value > 0);

  const nodeBarData = data.node_utilization.map(n => ({
    name: n.node_name.length > 10 ? n.node_name.slice(0, 10) + '…' : n.node_name,
    agents: n.agent_count,
    running: n.running_count,
  }));

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-white">Scheduler</h1>
        <p className="text-sm text-slate-400 mt-1">Scheduling algorithm, queue depth, and throughput</p>
      </div>

      {/* Algorithm info */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
        <div className="flex items-center gap-3 mb-2">
          <span className="text-2xl">⚙️</span>
          <div>
            <h2 className="text-lg font-semibold text-white">{data.current_algorithm.name}</h2>
            <p className="text-sm text-slate-400">{data.current_algorithm.description}</p>
          </div>
        </div>
      </div>

      {/* Queue + Throughput stats */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard label="Pending Queue" value={data.queue_depth.pending} icon="⏳" color="yellow" />
        <StatCard label="Running" value={data.queue_depth.running} icon="▶️" color="green" />
        <StatCard label="Success Rate" value={formatPercent(data.throughput.success_rate)} icon="✅" color="blue" />
        <StatCard label="Avg Restarts" value={data.throughput.avg_restart_count.toFixed(1)} icon="🔄" color="purple" />
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Queue depth pie chart */}
        <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
          <h2 className="text-lg font-semibold text-white mb-4">Queue Distribution</h2>
          {queuePieData.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <PieChart>
                <Pie
                  data={queuePieData}
                  cx="50%"
                  cy="50%"
                  innerRadius={50}
                  outerRadius={90}
                  paddingAngle={5}
                  dataKey="value"
                >
                  {queuePieData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: '8px' }}
                />
                <Legend formatter={(value: string) => <span style={{ color: '#94a3b8' }}>{value}</span>} />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex items-center justify-center h-[250px] text-slate-500 text-sm">
              No tasks in queue
            </div>
          )}
        </div>

        {/* Node utilization bar chart */}
        <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
          <h2 className="text-lg font-semibold text-white mb-4">Node Utilization</h2>
          {nodeBarData.length > 0 ? (
            <ResponsiveContainer width="100%" height={250}>
              <BarChart data={nodeBarData}>
                <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                <XAxis dataKey="name" stroke="#94a3b8" fontSize={12} />
                <YAxis stroke="#94a3b8" fontSize={12} />
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: '8px' }}
                />
                <Legend formatter={(value: string) => <span style={{ color: '#94a3b8' }}>{value === 'agents' ? 'Total Agents' : 'Running'}</span>} />
                <Bar dataKey="agents" fill="#3b82f6" radius={[4, 4, 0, 0]} />
                <Bar dataKey="running" fill="#10b981" radius={[4, 4, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex items-center justify-center h-[250px] text-slate-500 text-sm">
              No nodes available
            </div>
          )}
        </div>
      </div>

      {/* Throughput details */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
        <h2 className="text-lg font-semibold text-white mb-4">Throughput Summary</h2>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          <div className="text-center">
            <p className="text-2xl font-bold text-blue-400">{data.throughput.total_scheduled}</p>
            <p className="text-xs text-slate-400 mt-1">Total Scheduled</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-green-400">{data.throughput.total_completed}</p>
            <p className="text-xs text-slate-400 mt-1">Completed</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-red-400">{data.throughput.total_failed}</p>
            <p className="text-xs text-slate-400 mt-1">Failed</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-purple-400">{formatPercent(data.throughput.success_rate)}</p>
            <p className="text-xs text-slate-400 mt-1">Success Rate</p>
          </div>
        </div>
      </div>

      {/* Recent scheduling decisions */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 overflow-hidden">
        <div className="px-5 py-4 border-b border-slate-700">
          <h2 className="text-lg font-semibold text-white">Recent Scheduling Decisions</h2>
        </div>
        <table className="w-full">
          <thead>
            <tr className="border-b border-slate-700">
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Agent</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Node</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Status</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Priority</th>
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Time</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-700/50">
            {data.recent_decisions.map(decision => (
              <tr key={decision.agent_id} className="hover:bg-slate-700/20 transition-colors">
                <td className="px-5 py-3">
                  <span className="text-sm font-medium text-white">{decision.agent_name}</span>
                </td>
                <td className="px-5 py-3">
                  <span className="text-sm text-slate-300">{decision.assigned_node || '—'}</span>
                </td>
                <td className="px-5 py-3">
                  <StatusBadge status={decision.status} />
                </td>
                <td className="px-5 py-3">
                  <span className={`text-sm font-medium ${PRIORITY_COLORS[decision.priority] || 'text-slate-400'}`}>
                    {decision.priority}
                  </span>
                </td>
                <td className="px-5 py-3">
                  <span className="text-xs text-slate-500">
                    {new Date(decision.timestamp).toLocaleString()}
                  </span>
                </td>
              </tr>
            ))}
            {data.recent_decisions.length === 0 && (
              <tr>
                <td colSpan={5} className="px-5 py-8 text-center text-slate-500 text-sm">
                  No scheduling decisions yet
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
