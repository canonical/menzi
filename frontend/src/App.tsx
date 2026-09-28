import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Layout } from './components/Layout';
import { ProjectsPage } from './features/projects/ProjectsPage';
import { SessionPage } from './features/sessions/SessionPage';
import { EnvironmentPage } from './features/environments/EnvironmentPage';
import { PreviewsPage } from './features/previews/PreviewsPage';
import { DesignPage } from './features/design/DesignPage';
import { AdminPage } from './features/admin/AdminPage';
import { SettingsPage } from './features/settings/SettingsPage';

function App() {
  return (
    <BrowserRouter>
      <Layout>
        <Routes>
          <Route path="/" element={<ProjectsPage />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/projects/:projectId/sessions/:sessionId" element={<SessionPage />} />
          <Route path="/workspaces/:workspaceId/environment" element={<EnvironmentPage />} />
          <Route path="/projects/:projectId/previews" element={<PreviewsPage />} />
          <Route path="/projects/:projectId/design" element={<DesignPage />} />
          <Route path="/admin" element={<AdminPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </Layout>
    </BrowserRouter>
  );
}

export default App;
