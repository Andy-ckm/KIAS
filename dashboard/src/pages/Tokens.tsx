// Token Analytics page — token usage charts with Recharts

import { usePolling } from '../hooks/useApi';
import { getTokenAnalytics } from '../api/client';
import { StatCard, Spinner, ErrorBanner } from '../components/Common';
import type { TokenAnalytics } from '../types';
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  PieChart,
  Pie,
  Cell,
  BarChart,
  Bar,
  Legend,
} from 'recharts';

const COLORS = [
  '#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6',
  '#06b6d4', '#ec4899', '#f97316', '#14b8a6', '#6366f1',
];

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export default function TokenAnalyticsPage() {
  const { data, loading, error, refetch } = usePolling<TokenAnalytics>(getTokenAnalytics, 10000);

  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;
  if (!data) return null;

  // Prepare pie chart data from per_agent
  const pieData = data.per_agent.slice(0, 8).map((a, i) => ({
    name: a.agent_name,
    value: a.total_tokens,
    color: COLORS[i % COLORS.length],
  }));

  // Prepare bar chart data: input vs output per agent
  const barData = data.per_agent.slice(0, 8).map(a => ({
    name: a.agent_name.length > 12 ? a.agent_name.slice(0, 12) + '…' : a.agent_name,
    input: a.input_tokens,
    output: a.output_tokens,
  }));

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-white">Token Analytics</h1>
        <p className="text-sm text-slate-400 mt-1">LLM token usage and cost breakdown</p>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        <StatCard label="Total Tokens" value={formatTokens(data.total_tokens)} icon="🔤" color="blue" />
        <StatCard label="Input Tokens" value={formatTokens(data.total_input_tokens)} icon="📥" color="green" />
        <StatCard label="Output Tokens" value={formatTokens(data.total_output_tokens)} icon="📤" color="purple" />
        <StatCard label="Total Requests" value={formatTokens(data.total_requests)} icon="📡" color="yellow" />
        <StatCard label="Est. Cost" value={`$${data.total_cost.toFixed(4)}`} icon="💰" color="red" />
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Time series area chart — spans 2 cols */}
        <div className="lg:col-span-2 bg-[#1e293b] rounded-xl border border-slate-700 p-5">
          <h2 className="text-lg font-semibold text-white mb-4">Token Usage Over Time (24h)</h2>
          <ResponsiveContainer width="100%" height={300}>
            <AreaChart data={data.time_series}>
              <defs>
                <linearGradient id="colorInput" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
                </linearGradient>
                <linearGradient id="colorOutput" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="5%" stopColor="#10b981" stopOpacity={0.3} />
                  <stop offset="95%" stopColor="#10b981" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
              <XAxis dataKey="timestamp" stroke="#94a3b8" fontSize={12} />
              <YAxis stroke="#94a3b8" fontSize={12} tickFormatter={formatTokens} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: '8px' }}
                labelStyle={{ color: '#e2e8f0' }}
                formatter={(value: number, name: string) => [formatTokens(value), name === 'input_tokens' ? 'Input' : 'Output']}
              />
              <Area type="monotone" dataKey="input_tokens" stroke="#3b82f6" fill="url(#colorInput)" strokeWidth={2} />
              <Area type="monotone" dataKey="output_tokens" stroke="#10b981" fill="url(#colorOutput)" strokeWidth={2} />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Pie chart — token distribution by agent */}
        <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
          <h2 className="text-lg font-semibold text-white mb-4">Distribution by Agent</h2>
          {pieData.length > 0 ? (
            <ResponsiveContainer width="100%" height={300}>
              <PieChart>
                <Pie
                  data={pieData}
                  cx="50%"
                  cy="50%"
                  innerRadius={60}
                  outerRadius={100}
                  paddingAngle={3}
                  dataKey="value"
                >
                  {pieData.map((entry, index) => (
                    <Cell key={`cell-${index}`} fill={entry.color} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: '8px' }}
                  formatter={(value: number) => formatTokens(value)}
                />
                <Legend
                  verticalAlign="bottom"
                  height={36}
                  formatter={(value: string) => <span style={{ color: '#94a3b8', fontSize: '11px' }}>{value}</span>}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="flex items-center justify-center h-[300px] text-slate-500 text-sm">
              No agent data available
            </div>
          )}
        </div>
      </div>

      {/* Bar chart — input vs output per agent */}
      {barData.length > 0 && (
        <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5">
          <h2 className="text-lg font-semibold text-white mb-4">Input vs Output by Agent</h2>
          <ResponsiveContainer width="100%" height={300}>
            <BarChart data={barData}>
              <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
              <XAxis dataKey="name" stroke="#94a3b8" fontSize={12} />
              <YAxis stroke="#94a3b8" fontSize={12} tickFormatter={formatTokens} />
              <Tooltip
                contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: '8px' }}
                formatter={(value: number, name: string) => [formatTokens(value), name === 'input' ? 'Input' : 'Output']}
              />
              <Legend formatter={(value: string) => <span style={{ color: '#94a3b8' }}>{value === 'input' ? 'Input' : 'Output'}</span>} />
              <Bar dataKey="input" fill="#3b82f6" radius={[4, 4, 0, 0]} />
              <Bar dataKey="output" fill="#10b981" radius={[4, 4, 0, 0]} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      {/* Per-agent table */}
      <div className="bg-[#1e293b] rounded-xl border border-slate-700 overflow-hidden">
        <div className="px-5 py-4 border-b border-slate-700">
          <h2 className="text-lg font-semibold text-white">Per-Agent Token Usage</h2>
        </div>
        <table className="w-full">
          <thead>
            <tr className="border-b border-slate-700">
              <th className="text-left px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Agent</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Input</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Output</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Total</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Requests</th>
              <th className="text-right px-5 py-3 text-xs font-medium text-slate-400 uppercase tracking-wider">Cost</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-700/50">
            {data.per_agent.map(agent => (
              <tr key={agent.agent_id} className="hover:bg-slate-700/20 transition-colors">
                <td className="px-5 py-3">
                  <span className="text-sm font-medium text-white">{agent.agent_name}</span>
                </td>
                <td className="px-5 py-3 text-right">
                  <span className="text-sm text-blue-400">{formatTokens(agent.input_tokens)}</span>
                </td>
                <td className="px-5 py-3 text-right">
                  <span className="text-sm text-green-400">{formatTokens(agent.output_tokens)}</span>
                </td>
                <td className="px-5 py-3 text-right">
                  <span className="text-sm text-white font-medium">{formatTokens(agent.total_tokens)}</span>
                </td>
                <td className="px-5 py-3 text-right">
                  <span className="text-sm text-slate-300">{formatTokens(agent.request_count)}</span>
                </td>
                <td className="px-5 py-3 text-right">
                  <span className="text-sm text-yellow-400">${agent.estimated_cost.toFixed(4)}</span>
                </td>
              </tr>
            ))}
            {data.per_agent.length === 0 && (
              <tr>
                <td colSpan={6} className="px-5 py-8 text-center text-slate-500 text-sm">
                  No token usage data — create and run agents to see analytics
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
