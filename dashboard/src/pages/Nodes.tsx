// Nodes page — list all cluster nodes

import { useApi } from '../hooks/useApi';
import { listNodes } from '../api/client';
import { StatusBadge, Spinner, ErrorBanner, EmptyState } from '../components/Common';
import type { Node, ListResponse } from '../types';

export default function NodesPage() {
  const { data, loading, error, refetch } = useApi<ListResponse<Node>>(
    () => listNodes({ per_page: 100 })
  );

  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;

  const nodes = data?.items ?? [];

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold text-white">Nodes</h1>
        <p className="text-sm text-slate-400 mt-1">{data?.total ?? 0} nodes in cluster</p>
      </div>

      {nodes.length === 0 ? (
        <EmptyState message="No nodes registered in the cluster." />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {nodes.map(node => (
            <div key={node.id} className="bg-[#1e293b] rounded-xl border border-slate-700 p-5 space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="text-xl">🖥️</span>
                  <h3 className="text-base font-semibold text-white">{node.name}</h3>
                </div>
                <StatusBadge status={node.status} />
              </div>

              <div className="space-y-2">
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">CPU</span>
                  <span className="text-white font-medium">{node.resources.cpu}</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">Memory</span>
                  <span className="text-white font-medium">{node.resources.memory}</span>
                </div>
                <div className="flex justify-between text-sm">
                  <span className="text-slate-400">GPU</span>
                  <span className="text-white font-medium">{node.resources.gpu}</span>
                </div>
              </div>

              <div className="pt-3 border-t border-slate-700">
                <p className="text-xs text-slate-500">
                  Last heartbeat: {new Date(node.last_heartbeat).toLocaleString()}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
