// Workflows page — list, create, and manage DAG workflows.

import { useState } from 'react';
import { useApi } from '../hooks/useApi';
import { listWorkflows, createWorkflow, deleteWorkflow } from '../api/client';
import { StatCard, Spinner, ErrorBanner, EmptyState } from '../components/Common';
import type { WorkflowSummary, Workflow, CreateWorkflowRequest } from '../types';

function CreateWorkflowModal({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim()) {
      setError('Name is required');
      return;
    }

    setSaving(true);
    setError(null);
    try {
      const request: CreateWorkflowRequest = {
        name: name.trim(),
        description: description.trim(),
      };
      await createWorkflow(request);
      onCreated();
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Failed to create workflow');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50" onClick={onClose}>
      <div
        className="bg-[#1e293b] border border-slate-700 rounded-xl p-6 w-full max-w-md"
        onClick={event => event.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-white mb-4">Create Workflow</h2>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="workflow-name" className="block text-sm text-slate-400 mb-1">
              Name
            </label>
            <input
              id="workflow-name"
              type="text"
              value={name}
              onChange={event => setName(event.target.value)}
              className="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-blue-500"
              placeholder="my-workflow"
              autoFocus
            />
          </div>
          <div>
            <label htmlFor="workflow-description" className="block text-sm text-slate-400 mb-1">
              Description
            </label>
            <textarea
              id="workflow-description"
              value={description}
              onChange={event => setDescription(event.target.value)}
              className="w-full bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-white text-sm focus:outline-none focus:border-blue-500"
              placeholder="Workflow description..."
              rows={3}
            />
          </div>
          {error && <p className="text-sm text-red-400">{error}</p>}
          <div className="flex justify-end gap-3">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm text-slate-300 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              {saving ? 'Creating…' : 'Create'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

const STATUS_COLORS: Record<string, string> = {
  Draft: 'text-slate-400',
  Running: 'text-blue-400',
  Completed: 'text-green-400',
  Failed: 'text-red-400',
  Cancelled: 'text-yellow-400',
};

function WorkflowCard({ workflow, onDelete }: { workflow: Workflow; onDelete: (id: string) => void }) {
  const statusColor = STATUS_COLORS[workflow.status] || 'text-slate-400';

  return (
    <div className="bg-[#1e293b] rounded-xl border border-slate-700 p-5 space-y-3 hover:border-slate-600 transition-colors">
      <div className="flex items-start justify-between">
        <div>
          <h3 className="text-base font-semibold text-white">{workflow.name}</h3>
          <p className="text-xs text-slate-500 mt-1">ID: {workflow.id.slice(0, 8)}…</p>
        </div>
        <span className={`text-xs font-medium ${statusColor}`}>{workflow.status}</span>
      </div>

      {workflow.description && <p className="text-sm text-slate-400">{workflow.description}</p>}

      <div className="flex items-center gap-4 text-xs text-slate-500">
        <span>
          📐 {workflow.nodes.length} node{workflow.nodes.length !== 1 ? 's' : ''}
        </span>
        <span>
          🔄 {workflow.execution_count} run{workflow.execution_count !== 1 ? 's' : ''}
        </span>
        <span>📅 {new Date(workflow.created_at).toLocaleDateString()}</span>
      </div>

      {workflow.nodes.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {workflow.nodes.slice(0, 5).map(node => (
            <span
              key={node.id}
              className="inline-flex items-center px-2 py-1 rounded-md text-xs bg-slate-800 text-slate-300 border border-slate-700"
            >
              {node.name} ({node.node_type})
            </span>
          ))}
          {workflow.nodes.length > 5 && (
            <span className="text-xs text-slate-500">+{workflow.nodes.length - 5} more</span>
          )}
        </div>
      )}

      <div className="pt-2 border-t border-slate-700/50 flex justify-end">
        <button
          onClick={() => onDelete(workflow.id)}
          className="px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
        >
          Delete
        </button>
      </div>
    </div>
  );
}

export default function WorkflowsPage() {
  const { data, loading, error, refetch } = useApi<WorkflowSummary>(listWorkflows);
  const [showCreate, setShowCreate] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleDelete = async (id: string) => {
    setDeleteError(null);
    try {
      await deleteWorkflow(id);
      refetch();
    } catch (caught) {
      setDeleteError(caught instanceof Error ? caught.message : 'Failed to delete workflow');
    }
  };

  if (loading) return <Spinner />;
  if (error) return <ErrorBanner message={error} onRetry={refetch} />;
  if (!data) return null;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Workflows</h1>
          <p className="text-sm text-slate-400 mt-1">DAG workflow definitions and executions</p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="px-4 py-2 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
        >
          + Create Workflow
        </button>
      </div>

      {deleteError && <ErrorBanner message={deleteError} onRetry={refetch} />}

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        <StatCard label="Total" value={data.total} icon="📋" color="blue" />
        <StatCard label="Draft" value={data.draft} icon="📝" color="purple" />
        <StatCard label="Running" value={data.running} icon="▶️" color="green" />
        <StatCard label="Completed" value={data.completed} icon="✅" color="green" />
        <StatCard label="Failed" value={data.failed} icon="❌" color="red" />
      </div>

      {data.workflows.length === 0 ? (
        <EmptyState message="No workflows created yet. Click 'Create Workflow' to get started." />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {data.workflows.map(workflow => (
            <WorkflowCard key={workflow.id} workflow={workflow} onDelete={handleDelete} />
          ))}
        </div>
      )}

      {showCreate && (
        <CreateWorkflowModal onClose={() => setShowCreate(false)} onCreated={refetch} />
      )}
    </div>
  );
}