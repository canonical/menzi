import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { NotificationProvider } from '@canonical/react-components';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WorkspaceCard } from './WorkspaceCard';
import type { Workspace } from '../../lib/types';

const userId = 'u1';
const projectId = 'p1';

function workspace(overrides: Partial<Workspace> = {}): Workspace {
  return {
    id: 'w1',
    user_id: userId,
    project_id: projectId,
    name: 'wsp-a-b',
    status: 'ready',
    instance_name: 'wsp-a-b',
    last_error: null,
    created_at: '2026-01-02T03:04:05Z',
    updated_at: '2026-01-02T03:04:05Z',
    ...overrides,
  };
}

interface Call {
  url: string;
  method: string;
}

function mockWorkspaceApi(workspaces: Workspace[], listStatus = 200) {
  const calls: Call[] = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    const method = init?.method ?? 'GET';
    calls.push({ url, method });
    if (url.includes('/start')) {
      return json(workspace({ status: 'ready' }));
    }
    if (url.includes(`/workspaces/${userId}/${projectId}`) && method === 'POST') {
      return json(workspace({ status: 'idle' }));
    }
    if (url.includes(`/workspaces/${userId}/${projectId}`) && method === 'DELETE') {
      return json({ success: true });
    }
    if (listStatus !== 200) {
      return new Response(JSON.stringify({ error: 'the listing failed' }), {
        status: listStatus,
        headers: { 'content-type': 'application/json' },
      });
    }
    return json(workspaces);
  });
  return { fetchMock, calls };
}

function json(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}

function renderCard(workspaces: Workspace[], listStatus = 200) {
  const { fetchMock, calls } = mockWorkspaceApi(workspaces, listStatus);
  vi.stubGlobal('fetch', fetchMock);
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  render(
    <QueryClientProvider client={client}>
      <NotificationProvider>
        <WorkspaceCard projectId={projectId} userId={userId} />
      </NotificationProvider>
    </QueryClientProvider>,
  );
  return { calls };
}

describe('WorkspaceCard', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('says when there is no workspace yet', async () => {
    renderCard([]);
    const card = await screen.findByTestId('workspace-card');
    expect(card).toHaveTextContent('No workspace yet');
    expect(card).toHaveTextContent('One is set up when you open a session');
  });

  it('shows only the caller their own workspace', async () => {
    renderCard([
      workspace(),
      workspace({ id: 'w2', user_id: 'someone-else', name: 'wsp-other' }),
    ]);
    const card = await screen.findByTestId('workspace-card');
    expect(card).toHaveTextContent('wsp-a-b');
    expect(card).not.toHaveTextContent('wsp-other');
  });

  it('renders the status of a ready workspace', async () => {
    renderCard([workspace()]);
    expect(await screen.findByTestId('workspace-card')).toHaveTextContent('Ready');
  });

  it('renders a paused workspace as paused', async () => {
    renderCard([workspace({ status: 'idle' })]);
    expect(await screen.findByTestId('workspace-card')).toHaveTextContent('Paused');
  });

  it('renders a workspace still being set up', async () => {
    renderCard([workspace({ status: 'provisioning' })]);
    expect(await screen.findByTestId('workspace-card')).toHaveTextContent('Setting up');
  });

  it('surfaces a failure to list rather than pretending there is none', async () => {
    renderCard([workspace()], 500);
    expect(await screen.findByTestId('workspace-card')).toHaveTextContent(
      'the listing failed',
    );
  });

  it('offers resume for a paused workspace and pause for a running one', async () => {
    const user = userEvent.setup();
    renderCard([workspace({ status: 'idle' })]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    expect(await screen.findByRole('menuitem', { name: 'Resume' })).toBeTruthy();
    expect(screen.queryByRole('menuitem', { name: 'Pause' })).toBeNull();
  });

  it('pauses a workspace when asked', async () => {
    const user = userEvent.setup();
    const { calls } = renderCard([workspace({ status: 'ready' })]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    await user.click(await screen.findByRole('menuitem', { name: 'Pause' }));
    await waitFor(() =>
      expect(
        calls.some(
          (call) =>
            call.method === 'POST' && call.url === `/api/v1/workspaces/${userId}/${projectId}`,
        ),
      ).toBe(true),
    );
  });

  it('resumes a paused workspace when asked', async () => {
    const user = userEvent.setup();
    const { calls } = renderCard([workspace({ status: 'idle' })]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    await user.click(await screen.findByRole('menuitem', { name: 'Resume' }));
    await waitFor(() =>
      expect(
        calls.some(
          (call) =>
            call.method === 'POST' && call.url === `/api/v1/workspaces/${userId}/${projectId}/start`,
        ),
      ).toBe(true),
    );
  });

  it('confirms before deleting, because a container cannot be undone', async () => {
    const user = userEvent.setup();
    renderCard([workspace()]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    await user.click(await screen.findByRole('menuitem', { name: 'Delete' }));

    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('Delete this workspace?');
    expect(dialog).toHaveTextContent('cannot be undone');
  });

  it('deletes nothing when the confirmation is dismissed', async () => {
    const user = userEvent.setup();
    const { calls } = renderCard([workspace()]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    await user.click(await screen.findByRole('menuitem', { name: 'Delete' }));
    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Keep it' }),
    );
    expect(calls.some((call) => call.method === 'DELETE')).toBe(false);
  });

  it('deletes once the confirmation is accepted', async () => {
    const user = userEvent.setup();
    const { calls } = renderCard([workspace()]);
    await user.click(await screen.findByRole('button', { name: /actions/i }));
    await user.click(await screen.findByRole('menuitem', { name: 'Delete' }));

    await user.click(
      within(await screen.findByRole('dialog')).getByRole('button', { name: 'Delete workspace' }),
    );
    await waitFor(() => expect(calls.some((call) => call.method === 'DELETE')).toBe(true));
  });
});
