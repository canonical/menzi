import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CodePage } from './CodePage';
import { S } from '../../strings/catalogue';

const PROJECT_ID = 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa';
const SESSION_A = 'ses_aaaabbbbcccc';
const SESSION_B = 'ses_dddddeeeeffff';
const CREATED = 'ses_newlymade01';

const MESSAGES = [
  {
    info: { id: 'm1', sessionID: SESSION_A, role: 'user' },
    parts: [{ type: 'text', text: 'hello agent' }],
  },
];

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function mockApi(sessions: { id: string; time?: { updated?: number } }[]) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      const isSessionCollection = /\/session\/?$/.test(url);
      if (isSessionCollection && method === 'POST') {
        return Promise.resolve(jsonResponse(200, { id: CREATED }));
      }
      if (isSessionCollection) {
        return Promise.resolve(jsonResponse(200, sessions));
      }
      if (url.includes('/message')) {
        return Promise.resolve(jsonResponse(200, MESSAGES));
      }
      if (url.includes('/vcs/status')) return Promise.resolve(jsonResponse(200, { data: [] }));
      if (url.includes('/diff')) return Promise.resolve(jsonResponse(200, { data: [] }));
      if (url.includes('/api/model')) {
        return Promise.resolve(
          jsonResponse(200, { data: [{ id: 'space-bunny-free', providerID: 'opencode' }] }),
        );
      }
      if (url.includes('/agent')) {
        return Promise.resolve(jsonResponse(200, [{ name: 'build', description: 'Build' }]));
      }
      return Promise.resolve(jsonResponse(404, { error: 'no route' }));
    }),
  );
}

function renderPage(path: string) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <MemoryRouter initialEntries={[path]}>
          <Routes>
            <Route path="/projects/:projectId/code" element={<CodePage />} />
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

describe('CodePage', () => {
  it('asks for a session when opencode has none', async () => {
    mockApi([]);
    renderPage(`/projects/${PROJECT_ID}/code`);
    expect(await screen.findByText(S.code.emptyTitle)).toBeInTheDocument();
  });

  it('renders a tab per opencode session with chat and code review for the active one', async () => {
    mockApi([
      { id: SESSION_A, time: { updated: 1790000000000 } },
      { id: SESSION_B, time: { updated: 1790000001000 } },
    ]);
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    expect(
      await screen.findByRole('tab', { name: new RegExp(SESSION_A.replace('ses_', '').slice(0, 8)) }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('tab', { name: new RegExp(SESSION_B.replace('ses_', '').slice(0, 8)) }),
    ).toBeInTheDocument();
    expect(await screen.findByText('hello agent')).toBeInTheDocument();
    expect(screen.getByText(S.review.title)).toBeInTheDocument();
    expect(screen.getByLabelText(S.chat.label)).toBeInTheDocument();
  });

  it('switches the active session when another tab is chosen', async () => {
    mockApi([
      { id: SESSION_A, time: { updated: 1790000000000 } },
      { id: SESSION_B, time: { updated: 1790000001000 } },
    ]);
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);
    const second = await screen.findByRole('tab', {
      name: new RegExp(SESSION_B.replace('ses_', '').slice(0, 8)),
    });

    await user.click(second);

    expect(await screen.findByText('hello agent')).toBeInTheDocument();
  });

  it('creates a session through the opencode api and opens it', async () => {
    mockApi([]);
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code`);
    await screen.findByText(S.code.emptyTitle);

    await user.click(screen.getByRole('button', { name: S.code.newSession }));

    expect(await screen.findByText('hello agent')).toBeInTheDocument();
  });
});
