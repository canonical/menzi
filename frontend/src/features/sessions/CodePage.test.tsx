import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CodePage } from './CodePage';
import { useAuthStore } from '../../stores/auth';
import type { Workspace, WorkspaceStatus } from '../../lib/types';
import { S } from '../../strings/catalogue';

const PROJECT_ID = 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa';
const USER_ID = 'dddddddd-dddd-dddd-dddd-dddddddddddd';
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

function workspace(status: WorkspaceStatus, lastError: string | null = null): Workspace {
  return {
    id: 'w1',
    user_id: USER_ID,
    project_id: PROJECT_ID,
    name: 'wsp-a-b',
    status,
    instance_name: 'wsp-a-b',
    endpoint: 'http://10.0.0.1:17999',
    last_error: lastError,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

interface MockOptions {
  workspace?: Workspace | 'none' | 'missing' | 'forbidden';
  sessions?: { id: string; title?: string; time?: { updated?: number } }[];
}

function mockApi(options: MockOptions = {}) {
  const { workspace: workspaceState = workspace('ready'), sessions = [] } = options;
  const calls: string[] = [];

  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      calls.push(`${method} ${url}`);

      if (url.includes(`/workspaces/${USER_ID}/${PROJECT_ID}/sessions`)) {
        if (method === 'POST') return Promise.resolve(jsonResponse(201, { id: CREATED }));
        return Promise.resolve(jsonResponse(200, sessions));
      }
      if (/\/api\/v1\/workspaces$/.test(url)) {
        return Promise.resolve(
          jsonResponse(200, workspaceState === 'none' ? null : workspace('provisioning')),
        );
      }
      if (url.includes(`/api/v1/workspaces/${USER_ID}/${PROJECT_ID}`)) {
        if (workspaceState === 'missing') {
          return Promise.resolve(jsonResponse(404, { error: 'not found' }));
        }
        if (workspaceState === 'forbidden') {
          return Promise.resolve(jsonResponse(403, { error: 'forbidden' }));
        }
        return Promise.resolve(jsonResponse(200, workspaceState));
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

  return { calls };
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

beforeEach(() => {
  localStorage.clear();
  useAuthStore.setState({
    user: {
      id: USER_ID,
      kind: 'user',
      email: 'ada@acme.example',
      name: 'Ada Lovelace',
    },
    recorded: true,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  localStorage.clear();
});

describe('CodePage', () => {
  it('asks for a session when the workspace has none', async () => {
    mockApi({ sessions: [] });
    renderPage(`/projects/${PROJECT_ID}/code`);
    expect(await screen.findByText(S.code.emptyTitle)).toBeInTheDocument();
    expect(screen.getByText(S.workspace.sessions.emptyBody)).toBeInTheDocument();
  });

  it('renders a tab per session in this workspace, with the chat filling the page', async () => {
    mockApi({
      sessions: [
        { id: SESSION_A, time: { updated: 1790000000000 } },
        { id: SESSION_B, time: { updated: 1790000001000 } },
      ],
    });
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    expect(
      await screen.findByRole('tab', { name: new RegExp(SESSION_A.replace('ses_', '').slice(0, 8)) }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('tab', { name: new RegExp(SESSION_B.replace('ses_', '').slice(0, 8)) }),
    ).toBeInTheDocument();
    expect(await screen.findByText('hello agent')).toBeInTheDocument();
    expect(screen.getByLabelText(S.chat.label)).toBeInTheDocument();
  });

  it('drops the visible code and conversation labels from the chat page', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    await screen.findByText('hello agent');
    expect(screen.queryByText(S.review.title)).not.toBeInTheDocument();
    expect(document.querySelector('.app-panel-heading')).toBeNull();
    const heading = screen.getByRole('heading', { level: 1, name: S.sections.code });
    expect(heading).toHaveClass('u-off-screen');
  });

  it('keeps the review panel closed until it is asked for', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    await screen.findByText('hello agent');
    expect(screen.queryByText(S.review.title)).not.toBeInTheDocument();
    expect(screen.getByTestId('code-split')).toHaveAttribute('data-review', 'closed');
    expect(screen.getByRole('button', { name: S.code.showReview })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });

  it('opens the review panel from the header button and records it in the url', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    await screen.findByText('hello agent');
    await user.click(screen.getByRole('button', { name: S.code.showReview }));

    expect(await screen.findByText(S.review.title)).toBeInTheDocument();
    expect(screen.getByTestId('code-split')).toHaveAttribute('data-review', 'open');
    expect(screen.getByRole('button', { name: S.code.hideReview })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });

  it('opens the review panel straight from a shared link', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}&panel=review`);

    expect(await screen.findByText(S.review.title)).toBeInTheDocument();
    expect(screen.getByTestId('code-split')).toHaveAttribute('data-review', 'open');
  });

  it('closes the review panel again and returns the chat to the full page', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}&panel=review`);

    await screen.findByText(S.review.title);
    await user.click(screen.getByRole('button', { name: S.code.hideReview }));

    await waitFor(() => {
      expect(screen.queryByText(S.review.title)).not.toBeInTheDocument();
    });
    expect(screen.getByTestId('code-split')).toHaveAttribute('data-review', 'closed');
  });

  it('keeps the session when the review panel is toggled', async () => {
    mockApi({ sessions: [{ id: SESSION_A, time: { updated: 1790000000000 } }] });
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    await screen.findByText('hello agent');
    await user.click(screen.getByRole('button', { name: S.code.showReview }));

    expect(await screen.findByText('hello agent')).toBeInTheDocument();
    expect(
      screen.getByRole('tab', { name: new RegExp(SESSION_A.replace('ses_', '').slice(0, 8)) }),
    ).toBeInTheDocument();
  });

  it('labels a tab with its title when it has one', async () => {
    mockApi({ sessions: [{ id: SESSION_A, title: 'move the retry policy' }] });
    renderPage(`/projects/${PROJECT_ID}/code`);
    expect(
      await screen.findByRole('tab', { name: /move the retry policy/ }),
    ).toBeInTheDocument();
  });

  it('falls back to a short id when a session has no title', async () => {
    mockApi({ sessions: [{ id: SESSION_A }] });
    renderPage(`/projects/${PROJECT_ID}/code`);
    expect(
      await screen.findByRole('tab', { name: new RegExp('aaaabbbb') }),
    ).toBeInTheDocument();
  });

  it('switches the active session when another tab is chosen', async () => {
    mockApi({
      sessions: [
        { id: SESSION_A, time: { updated: 1790000000000 } },
        { id: SESSION_B, time: { updated: 1790000001000 } },
      ],
    });
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);
    const second = await screen.findByRole('tab', {
      name: new RegExp(SESSION_B.replace('ses_', '').slice(0, 8)),
    });

    await user.click(second);

    expect(await screen.findByText('hello agent')).toBeInTheDocument();
  });

  it('creates a session in the workspace and opens it', async () => {
    const { calls } = mockApi({ sessions: [] });
    const user = userEvent.setup();
    renderPage(`/projects/${PROJECT_ID}/code`);
    await screen.findByText(S.code.emptyTitle);

    await user.click(screen.getByRole('button', { name: S.code.newSession }));

    await waitFor(() =>
      expect(
        calls.some(
          (call) => call === `POST /api/v1/workspaces/${USER_ID}/${PROJECT_ID}/sessions`,
        ),
      ).toBe(true),
    );
  });

  it('starts a workspace by itself when the project has none', async () => {
    const { calls } = mockApi({ workspace: 'missing', sessions: [] });
    renderPage(`/projects/${PROJECT_ID}/code`);

    await waitFor(() =>
      expect(calls.some((call) => call === 'POST /api/v1/workspaces')).toBe(true),
    );
  });

  it('explains that a workspace is being set up and blocks the session button', async () => {
    mockApi({ workspace: workspace('provisioning'), sessions: [] });
    renderPage(`/projects/${PROJECT_ID}/code`);

    await waitFor(() =>
      expect(screen.getByTestId('workspace-status')).toHaveTextContent(
        'Setting up your workspace',
      ),
    );
    await waitFor(() =>
      expect(screen.getByRole('button', { name: S.code.newSession })).toHaveAttribute(
        'aria-disabled',
        'true',
      ),
    );
    expect(screen.getByTestId('session-blocked-reason')).toHaveTextContent(
      'still being set up',
    );
  });

  it('shows the reason a setup failed and offers a retry', async () => {
    mockApi({ workspace: workspace('requested', 'lxd: the image could not be copied') });
    renderPage(`/projects/${PROJECT_ID}/code`);

    await waitFor(() =>
      expect(screen.getByTestId('workspace-status')).toHaveTextContent('Workspace setup failed'),
    );
    await waitFor(() =>
      expect(screen.getByTestId('workspace-status')).toHaveTextContent(
        'lxd: the image could not be copied',
      ),
    );
    expect(screen.getByRole('button', { name: S.workspace.retry })).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByRole('button', { name: S.code.newSession })).toHaveAttribute(
        'aria-disabled',
        'true',
      ),
    );
  });

  it('treats missing access as a permission, not a fault', async () => {
    mockApi({ workspace: 'forbidden' });
    renderPage(`/projects/${PROJECT_ID}/code`);

    await waitFor(() =>
      expect(screen.getByTestId('workspace-status')).toHaveTextContent(
        'No workspace for you in this project',
      ),
    );
    expect(screen.getByTestId('workspace-status')).not.toHaveTextContent(S.workspace.retry);
    await waitFor(() =>
      expect(screen.getByRole('button', { name: S.code.newSession })).toHaveAttribute(
        'aria-disabled',
        'true',
      ),
    );
  });

  it('lets a session start against a paused workspace, and says it will be slower', async () => {
    mockApi({ workspace: workspace('idle'), sessions: [] });
    renderPage(`/projects/${PROJECT_ID}/code`);

    await waitFor(() =>
      expect(screen.getByTestId('workspace-status')).toHaveTextContent(
        'Starting a session will wake it',
      ),
    );
    expect(screen.getByRole('button', { name: S.code.newSession })).not.toHaveAttribute(
      'aria-disabled',
      'true',
    );
  });

  it('adds no chrome at all on the happy path', async () => {
    mockApi({ sessions: [{ id: SESSION_A }] });
    renderPage(`/projects/${PROJECT_ID}/code?session=${SESSION_A}`);

    await screen.findByRole('tab', { name: /aaaabbbb/ });
    await waitFor(() =>
      expect(screen.getByRole('button', { name: S.code.newSession })).not.toHaveAttribute(
        'aria-disabled',
        'true',
      ),
    );
    expect(screen.queryByTestId('session-blocked-reason')).toBeNull();
    expect(screen.queryByTestId('workspace-status')).toBeNull();
  });
});

async function waitFor(assertion: () => void): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    try {
      assertion();
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
  }
  assertion();
}


