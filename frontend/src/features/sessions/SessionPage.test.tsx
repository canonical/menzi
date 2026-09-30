import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { SessionPage } from './SessionPage';
import { axeOptions } from '../../testing/axe';
import { S } from '../../strings/catalogue';

const SESSION = '11111111-2222-3333-4444-555555555555';

const STATE = {
  name: 'dev',
  status: 'degraded',
  components: [
    { name: 'db', status: 'running', health: 'healthy' },
    { name: 'api', status: 'unknown', health: 'degraded' },
  ],
  exposures: [{ name: 'api-web', url: 'http://api-web.dev.local', as_type: 'http' }],
};

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function mockOrchestrator(handlers: Record<string, () => ReturnType<typeof jsonResponse>>) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      const key = Object.keys(handlers).find((candidate) => url.includes(candidate));
      if (!key) {
        return Promise.resolve(jsonResponse(404, { success: false, message: 'no route' }));
      }
      return Promise.resolve(handlers[key]());
    }),
  );
}

const specsResponse = (specs: string[]) =>
  jsonResponse(200, { success: true, message: 'ok', data: { specs } });

const statusResponse = (status: number, body: unknown) =>
  jsonResponse(status, { success: status < 400, message: 'status retrieved', data: body });

function renderPage(sessionId = SESSION) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <MemoryRouter initialEntries={[`/projects/proj-1/sessions/${sessionId}`]}>
          <Routes>
            <Route path="/projects/:projectId/sessions/:sessionId" element={<SessionPage />} />
          </Routes>
        </MemoryRouter>
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('SessionPage', () => {
  it('renders the three-column workspace with the env tab', async () => {
    mockOrchestrator({
      '/api/env/specs': () => specsResponse(['dev']),
      '/api/env/status': () => statusResponse(200, STATE),
    });
    const { container } = renderPage();
    expect(screen.getByText(S.env.title)).toBeInTheDocument();
    expect(await screen.findByText('db')).toBeInTheDocument();
    expect(screen.getByText('api')).toBeInTheDocument();
    expect(screen.getByText('api-web')).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('explains when the orchestrator has no environment specs', async () => {
    mockOrchestrator({ '/api/env/specs': () => specsResponse([]) });
    renderPage();
    expect(await screen.findByText(S.env.specMissingTitle)).toBeInTheDocument();
    expect(screen.queryByLabelText(S.env.environmentLabel)).not.toBeInTheDocument();
  });

  it('surfaces a spec that disappears between listing and querying', async () => {
    mockOrchestrator({
      '/api/env/specs': () => specsResponse(['dev']),
      '/api/env/status': () =>
        jsonResponse(404, {
          success: false,
          message: "environment spec 'dev' not found",
          data: null,
        }),
    });
    renderPage();
    expect(await screen.findByText(S.env.specMissingTitle)).toBeInTheDocument();
    expect(screen.queryByText('db')).not.toBeInTheDocument();
  });

  it('keeps tabs that have no backend in an unavailable state', async () => {
    mockOrchestrator({
      '/api/env/specs': () => specsResponse(['dev']),
      '/api/env/status': () => statusResponse(200, STATE),
    });
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole('tab', { name: 'Files' }));
    expect(screen.getByText('Files are not available yet')).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: S.env.title }));
    expect(await screen.findByLabelText(S.env.environmentLabel)).toBeInTheDocument();
  });

  it('does not query the orchestrator for a session id that is not a uuid', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      if (String(input).includes('/api/v1/auth/session')) {
        return Promise.resolve(
          jsonResponse(200, { user: { id: 'u1', kind: 'user', email: 'a@b' } }),
        );
      }
      return Promise.resolve(jsonResponse(200, { success: true, data: STATE }));
    });
    vi.stubGlobal('fetch', fetchMock);
    renderPage('not-a-uuid');
    expect(screen.queryByLabelText(S.env.environmentLabel)).not.toBeInTheDocument();
    const orchestratorCalls = fetchMock.mock.calls.filter((call) =>
      String(call[0]).includes('/api/env'),
    );
    expect(orchestratorCalls).toHaveLength(0);
  });
});
