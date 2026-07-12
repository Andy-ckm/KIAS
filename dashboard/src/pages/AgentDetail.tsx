import { Link, useParams } from 'react-router-dom';

import { getAgent } from '../api/client';
import { ErrorBanner, Spinner, StatusBadge } from '../components/Common';
import { useProductContext } from '../components/ProductContext';
import { useApi } from '../hooks/useApi';
import type { Agent, ApiResponse } from '../types';

function formatDate(timestamp: string | undefined): string {
  if (!timestamp) return '—';
  try {
    return new Date(timestamp).toLocaleString();
  } catch {
    return timestamp;
  }
}

function displayResource(value: string | undefined): string {
  return value && value.trim() ? value : 'Not constrained';
}

function Definition({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-xs font-medium uppercase tracking-wide text-slate-500">{label}</dt>
      <dd className="mt-1 break-words text-sm text-slate-100">{value}</dd>
    </div>
  );
}

function ConstraintCard({ agent }: { agent: Agent }) {
  return (
    <section className="rounded-xl border border-slate-700 bg-[#1e293b] p-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-[0.16em] text-blue-400">
            Managed resource
          </p>
          <h1 className="mt-2 text-2xl font-bold text-white">{agent.spec.name}</h1>
          <p className="mt-1 font-mono text-xs text-slate-500">{agent.id}</p>
        </div>
        <StatusBadge status={agent.status} />
      </div>

      <dl className="mt-6 grid grid-cols-1 gap-5 border-t border-slate-700 pt-5 sm:grid-cols-2 xl:grid-cols-4">
        <Definition label="Runtime image" value={agent.spec.image} />
        <Definition label="Priority" value={agent.spec.priority} />
        <Definition label="Assigned node" value={agent.node_id ?? 'Not assigned'} />
        <Definition label="Restart count" value={String(agent.restart_count)} />
      </dl>
    </section>
  );
}

export default function AgentDetailPage() {
  const { id } = useParams<{ id: string }>();
  const { capabilities } = useProductContext();
  const {
    data: response,
    loading,
    error,
    refetch,
  } = useApi<ApiResponse<Agent>>(() => getAgent(id ?? ''), [id]);

  if (!id) return <ErrorBanner message="No agent ID provided" />;
  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;
  if (!response) return null;

  const agent = response.data;
  const requested = agent.spec.resource_request;
  const environmentKeys = Object.keys(agent.spec.env).sort();
  const labels = Object.entries(agent.spec.labels).sort(([left], [right]) =>
    left.localeCompare(right)
  );
  const realtimeEnabled = capabilities.capabilities.some(
    capability => capability.id === 'realtime-events' && capability.enabled
  );

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between gap-4">
        <Link to="/agents" className="text-sm text-blue-400 transition hover:text-blue-300">
          ← Agent Fleet
        </Link>
        <span className="rounded-full border border-slate-700 px-3 py-1 text-xs text-slate-400">
          Instance profile: {capabilities.profile}
        </span>
      </div>

      <ConstraintCard agent={agent} />

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <section className="rounded-xl border border-slate-700 bg-[#1e293b] p-6">
          <h2 className="text-lg font-semibold text-white">Declared execution constraints</h2>
          <p className="mt-1 text-sm text-slate-400">
            These are requested limits. Runtime enforcement evidence must come from the executor and
            scheduler, not from this declaration alone.
          </p>
          <dl className="mt-5 grid grid-cols-1 gap-5 sm:grid-cols-3">
            <Definition label="CPU" value={displayResource(requested?.cpu)} />
            <Definition label="Memory" value={displayResource(requested?.memory)} />
            <Definition label="GPU" value={displayResource(requested?.gpu)} />
          </dl>

          <div className="mt-6 border-t border-slate-700 pt-5">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Command</p>
            <code className="mt-2 block overflow-x-auto rounded-lg bg-slate-950 p-3 text-xs text-slate-300">
              {agent.spec.command.length > 0 ? agent.spec.command.join(' ') : 'No command declared'}
            </code>
          </div>
        </section>

        <section className="rounded-xl border border-slate-700 bg-[#1e293b] p-6">
          <h2 className="text-lg font-semibold text-white">Identity and metadata</h2>
          <dl className="mt-5 grid grid-cols-1 gap-5 sm:grid-cols-2">
            <Definition label="Created" value={formatDate(agent.created_at)} />
            <Definition label="Updated" value={formatDate(agent.updated_at)} />
            <Definition label="Started" value={formatDate(agent.start_time)} />
            <Definition label="Observed status" value={agent.status} />
          </dl>

          <div className="mt-6 border-t border-slate-700 pt-5">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">Labels</p>
            <div className="mt-2 flex flex-wrap gap-2">
              {labels.length > 0 ? (
                labels.map(([key, value]) => (
                  <span
                    key={key}
                    className="rounded-md border border-slate-600 bg-slate-800 px-2 py-1 text-xs text-slate-300"
                  >
                    {key}={value}
                  </span>
                ))
              ) : (
                <span className="text-sm text-slate-500">No labels declared</span>
              )}
            </div>
          </div>

          <div className="mt-6 border-t border-slate-700 pt-5">
            <p className="text-xs font-medium uppercase tracking-wide text-slate-500">
              Environment references
            </p>
            <p className="mt-1 text-sm text-slate-400">
              Values are intentionally hidden. Environment variables must not be treated as the
              product's long-term secret-management interface.
            </p>
            <div className="mt-2 flex flex-wrap gap-2">
              {environmentKeys.length > 0 ? (
                environmentKeys.map(key => (
                  <span
                    key={key}
                    className="rounded-md border border-slate-600 bg-slate-950 px-2 py-1 font-mono text-xs text-slate-300"
                  >
                    {key}=••••••
                  </span>
                ))
              ) : (
                <span className="text-sm text-slate-500">No environment references declared</span>
              )}
            </div>
          </div>
        </section>
      </div>

      <section className="rounded-xl border border-amber-500/20 bg-amber-500/5 p-6">
        <h2 className="text-lg font-semibold text-amber-100">Run evidence is not fabricated</h2>
        <p className="mt-2 max-w-4xl text-sm leading-6 text-amber-100/70">
          This pre-1.0 detail view does not yet expose a stable run-correlated timeline for policy
          decisions, tool calls, resource measurements, logs and recovery actions. Empty charts and
          synthetic log streams have therefore been removed. Realtime events are currently{' '}
          <strong>{realtimeEnabled ? 'enabled' : 'disabled'}</strong> for this instance, but the
          evidence workspace remains a release blocker until the backend contract is complete.
        </p>
      </section>
    </div>
  );
}
