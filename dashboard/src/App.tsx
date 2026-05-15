// KIAS Dashboard — Main App

import { BrowserRouter, Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import DashboardPage from './pages/Dashboard';
import AgentsPage from './pages/Agents';
import AgentDetailPage from './pages/AgentDetail';
import NodesPage from './pages/Nodes';
import ClusterPage from './pages/Cluster';
import TokenAnalyticsPage from './pages/Tokens';
import WorkflowsPage from './pages/Workflows';
import SchedulerPage from './pages/Scheduler';

export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<DashboardPage />} />
          <Route path="agents" element={<AgentsPage />} />
          <Route path="agents/:id" element={<AgentDetailPage />} />
          <Route path="nodes" element={<NodesPage />} />
          <Route path="cluster" element={<ClusterPage />} />
          <Route path="tokens" element={<TokenAnalyticsPage />} />
          <Route path="workflows" element={<WorkflowsPage />} />
          <Route path="scheduler" element={<SchedulerPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
