// KIAS Dashboard — Main App

import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import DashboardPage from './pages/Dashboard';
import AgentsPage from './pages/Agents';
import NodesPage from './pages/Nodes';
import ClusterPage from './pages/Cluster';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<DashboardPage />} />
          <Route path="agents" element={<AgentsPage />} />
          <Route path="nodes" element={<NodesPage />} />
          <Route path="cluster" element={<ClusterPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
