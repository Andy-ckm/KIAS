// Shared dashboard presentation components.

const STATUS_COLORS: Record<string, string> = {
  Pending: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
  Scheduled: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
  Running: 'bg-green-500/20 text-green-400 border-green-500/30',
  Succeeded: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
  Failed: 'bg-red-500/20 text-red-400 border-red-500/30',
  Unknown: 'bg-slate-500/20 text-slate-400 border-slate-500/30',
  Ready: 'bg-green-500/20 text-green-400 border-green-500/30',
  NotReady: 'bg-red-500/20 text-red-400 border-red-500/30',
  healthy: 'bg-green-500/20 text-green-400 border-green-500/30',
  degraded: 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30',
};

export function StatusBadge({ status }: { status: string }) {
  const color = STATUS_COLORS[status] || STATUS_COLORS.Unknown;
  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium border ${color}`}>
      {status}
    </span>
  );
}

export function StatCard({
  label,
  value,
  icon,
  color = 'blue',
}: {
  label: string;
  value: number | string;
  icon: string;
  color?: string;
}) {
  const colorClasses: Record<string, string> = {
    blue: 'from-blue-600/20 to-blue-600/5 border-blue-500/20',
    green: 'from-green-600/20 to-green-600/5 border-green-500/20',
    yellow: 'from-yellow-600/20 to-yellow-600/5 border-yellow-500/20',
    red: 'from-red-600/20 to-red-600/5 border-red-500/20',
    purple: 'from-purple-600/20 to-purple-600/5 border-purple-500/20',
  };

  return (
    <div className={`bg-gradient-to-br ${colorClasses[color]} border rounded-xl p-5`}>
      <div className="flex items-center justify-between">
        <div>
          <p className="text-sm text-slate-400">{label}</p>
          <p className="text-2xl font-bold text-white mt-1">{value}</p>
        </div>
        <span className="text-3xl">{icon}</span>
      </div>
    </div>
  );
}

export function Spinner() {
  return (
    <div className="flex items-center justify-center p-12">
      <div className="w-8 h-8 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
    </div>
  );
}

export function ErrorBanner({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="bg-red-500/10 border border-red-500/20 rounded-xl p-4 flex items-center justify-between">
      <div className="flex items-center gap-3">
        <span className="text-red-400 text-xl">⚠️</span>
        <p className="text-red-300 text-sm">{message}</p>
      </div>
      {onRetry && (
        <button
          onClick={onRetry}
          className="px-3 py-1.5 text-xs bg-red-500/20 text-red-300 rounded-lg hover:bg-red-500/30 transition-colors"
        >
          Retry
        </button>
      )}
    </div>
  );
}

export function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center justify-center p-12 text-slate-500">
      <span className="text-4xl mb-3">📭</span>
      <p className="text-sm">{message}</p>
    </div>
  );
}
