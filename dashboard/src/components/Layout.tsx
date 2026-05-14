// Layout component — sidebar + main content area

import { NavLink, Outlet } from 'react-router-dom';

const NAV_ITEMS = [
  { to: '/', label: 'Dashboard', icon: '📊' },
  { to: '/agents', label: 'Agents', icon: '🤖' },
  { to: '/nodes', label: 'Nodes', icon: '🖥️' },
  { to: '/cluster', label: 'Cluster', icon: '🌐' },
  { to: '/tokens', label: 'Token Analytics', icon: '🔤' },
  { to: '/workflows', label: 'Workflows', icon: '🔄' },
  { to: '/scheduler', label: 'Scheduler', icon: '⚙️' },
];

function Sidebar() {
  return (
    <aside className="w-64 bg-[#1e293b] border-r border-slate-700 flex flex-col min-h-screen">
      {/* Logo */}
      <div className="p-6 border-b border-slate-700">
        <h1 className="text-xl font-bold text-blue-400">KIAS</h1>
        <p className="text-xs text-slate-400 mt-1">Intelligent Agent Scheduler</p>
      </div>

      {/* Navigation */}
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

      {/* Footer */}
      <div className="p-4 border-t border-slate-700">
        <p className="text-xs text-slate-500">KIAS v0.1.0</p>
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
