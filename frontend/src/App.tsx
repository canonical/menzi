import { ComponentType, lazy, Suspense } from 'react';
import { BrowserRouter, Outlet, Route, Routes } from 'react-router-dom';
import { Spinner } from '@canonical/react-components';
import { Layout } from './components/Layout';
import { ErrorBoundary } from './components/ErrorBoundary';
import { useCurrentUser } from './stores/useCurrentUser';
import { S } from './strings/catalogue';

function lazyPage(name: string, loader: () => Promise<Record<string, ComponentType>>) {
  return lazy(async () => ({ default: (await loader())[name] }));
}

const ProjectsPage = lazyPage('ProjectsPage', () => import('./features/projects/ProjectsPage'));
const ProjectPage = lazyPage('ProjectPage', () => import('./features/projects/ProjectPage'));
const CodePage = lazyPage('CodePage', () => import('./features/sessions/CodePage'));
const SessionPage = lazyPage('SessionPage', () => import('./features/sessions/SessionPage'));
const EnvironmentPage = lazyPage(
  'EnvironmentPage',
  () => import('./features/environments/EnvironmentPage'),
);
const PreviewsPage = lazyPage('PreviewsPage', () => import('./features/previews/PreviewsPage'));
const DesignPage = lazyPage('DesignPage', () => import('./features/design/DesignPage'));
const InboxPage = lazyPage('InboxPage', () => import('./features/inbox/InboxPage'));
const AdminPage = lazyPage('AdminPage', () => import('./features/admin/AdminPage'));
const SettingsPage = lazyPage('SettingsPage', () => import('./features/settings/SettingsPage'));
const NotFoundPage = lazyPage('NotFoundPage', () => import('./features/notfound/NotFoundPage'));

function PageFallback() {
  return (
    <div className="u-align--center u-vertically-center">
      <Spinner text={S.dataState.loading} />
    </div>
  );
}

function AppShell() {
  useCurrentUser();
  return (
    <ErrorBoundary>
      <Layout>
        <Suspense fallback={<PageFallback />}>
          <Outlet />
        </Suspense>
      </Layout>
    </ErrorBoundary>
  );
}

function WorkspaceShell() {
  return (
    <ErrorBoundary>
      <Suspense fallback={<PageFallback />}>
        <Outlet />
      </Suspense>
    </ErrorBoundary>
  );
}

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route path="/" element={<ProjectsPage />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/projects/:projectId" element={<ProjectsPage />} />
          <Route path="/projects/:projectId/code" element={<CodePage />} />
          <Route path="/projects/:projectId/design" element={<DesignPage />} />
          <Route path="/projects/:projectId/project" element={<ProjectPage />} />
          <Route
            path="/workspaces/:workspaceId/environment"
            element={<EnvironmentPage />}
          />
          <Route path="/projects/:projectId/previews" element={<PreviewsPage />} />
          <Route path="/inbox" element={<InboxPage />} />
          <Route path="/admin" element={<AdminPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<NotFoundPage />} />
        </Route>
        <Route element={<WorkspaceShell />}>
          <Route
            path="/projects/:projectId/sessions/:sessionId"
            element={<SessionPage />}
          />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}

export default App;
