// Layout component — sidebar + main content area

import { NavLink, Outlet } from 'react-router-dom';
import { useProductContext } from './ProductContext';

const NAV_ITEMS = [
  { to: '/', label: 'Overview', icon: '📊' },
  { to: '/agents', label: 'Agent Fleet', icon: '🤖' },
  { to: '/nodes', label: 'Infrastructure', icon: '🖥️' },
  { to: '/cluster', label: 'Health & Recovery', icon: '🛟' },
  { to: '/tokens', label: 'Cost & Usage', icon: '🔤' },
  { to: '/workflows', label: 'Workflows', icon: '🔄' },
  { to: '/scheduler', label: 'Scheduling', icon: '⚙️' },
];

function Sidebar() {
  const { capabilities, disconnect } = useProductContext();
  const enabledCount = capabilities.capabilities.filter(capability => capability.enabled).length;

  return (
    <aside className="w-64 bg-[#1e293b] border-r border-slate-700 flex flex-col min-h-screen">
      <div className="p-6 border-b border-slate-700">
        <h1 className="text-xl font-bold text-blue-400">KIAS</h1>
        <p className="text-xs text-slate-400 mt-1">Agent Operations Control Plane</p>
        <div className="mt-3 flex items-center gap-2">
          <span className="rounded-full border border-blue-500/30 bg-blue-500/10 px-2 py-0.5 text-[11px] font-medium text-blue-300">
            {capabilities.profile}
          </span>
          <span className="text-[11px] text-slate-500">{enabledCount} enabled</span>
        </div>
      </div>

      <nav className="flex-1 p-4 space-y-1">
        {NAV_ITEMS.map(item => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === '/'}
            className={({ isActive }) =>
              `flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-colors ${
                isActive
                  ? 'bg-blue-600/20 text-blue-400'
                  : 'text-slate-300 hover:bg-slate-700/50 hover:text-white'
              }`
            }
          >
            <span className="text-lg">{item.icon}</span>
            {item.label}
          </NavLink>
        ))}
      </nav>

      <div className="p-4 border-t border-slate-700 space-y-3">
        <div>
          <p className="text-xs text-slate-400">KIAS v{capabilities.version}</p>
          <p className="mt-1 text-[11px] text-slate-500">Control · Evidence · Recovery</p>
        </div>
        <button
          type="button"
          onClick={disconnect}
          className="w-full rounded-lg border border-slate-600 px-3 py-2 text-xs font-medium text-slate-300 transition hover:border-slate-500 hover:text-white"
        >
          Disconnect operator session
        </button>
      </div>
    </aside>
  );
}

export default function Layout() {
  return (
    <div className="flex min-h-screen">
      <Sidebar />
      <main className="flex-1 p-6 overflow-auto">
        <Outlet />
      </main>
    </div>
  );
}
