import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  connectWorkspace,
  destroyWorkspace,
  ensureWorkspace,
  getWorkspace,
  interruptWorkspaceSession,
  listWorkspaceSessions,
  listWorkspacesForProject,
  openWorkspaceSession,
  promptWorkspace,
  runWorkspaceCommand,
  startWorkspace,
  suspendWorkspace,
  workspaceFilePatch,
  workspaceTreeChanges,
  WorkspaceNotFoundError,
} from './workspaces';

function mockFetch(status: number, body: unknown) {
  return vi.fn(async () =>
    new Response(body === undefined ? '' : JSON.stringify(body), {
      status,
      headers: { 'content-type': 'application/json' },
    }),
  );
}

async function lastRequest(fetchMock: ReturnType<typeof mockFetch>) {
  const call = fetchMock.mock.calls.at(-1);
  if (!call) throw new Error('no request was made');
  const [url, init] = call as unknown as [string, RequestInit];
  return {
    url,
    method: init.method ?? 'GET',
    body: typeof init.body === 'string' ? JSON.parse(init.body) : undefined,
  };
}

describe('workspaces api', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('ensures a workspace for a project', async () => {
    const fetchMock = mockFetch(200, { id: 'w1', status: 'ready' });
    vi.stubGlobal('fetch', fetchMock);
    const workspace = await ensureWorkspace({ projectId: 'p1' });
    expect(workspace.status).toBe('ready');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces');
    expect(request.method).toBe('POST');
    expect(request.body.project_id).toBe('p1');
  });

  it('carries the optional workspace fields when given', async () => {
    const fetchMock = mockFetch(200, { id: 'w1' });
    vi.stubGlobal('fetch', fetchMock);
    await ensureWorkspace({ projectId: 'p1', name: 'ws', branch: 'main', commitSha: 'abc' });
    const request = await lastRequest(fetchMock);
    expect(request.body).toMatchObject({ name: 'ws', branch: 'main', commit_sha: 'abc' });
  });

  it('reads a workspace by user and project', async () => {
    const fetchMock = mockFetch(200, { id: 'w1' });
    vi.stubGlobal('fetch', fetchMock);
    await getWorkspace('u1', 'p1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1');
    expect(request.method).toBe('GET');
  });

  it('turns a missing workspace into a typed absence', async () => {
    vi.stubGlobal('fetch', mockFetch(404, { error: 'not found' }));
    await expect(getWorkspace('u1', 'p1')).rejects.toBeInstanceOf(WorkspaceNotFoundError);
  });

  it('passes other failures through untouched', async () => {
    vi.stubGlobal('fetch', mockFetch(403, { error: 'forbidden' }));
    await expect(getWorkspace('u1', 'p1')).rejects.toMatchObject({ status: 403 });
  });

  it('lists the workspaces of a project', async () => {
    const fetchMock = mockFetch(200, [{ id: 'w1' }]);
    vi.stubGlobal('fetch', fetchMock);
    const workspaces = await listWorkspacesForProject('p1');
    expect(workspaces).toHaveLength(1);
    expect((await lastRequest(fetchMock)).url).toBe('/api/v1/projects/p1/workspaces');
  });

  it('starts a paused workspace', async () => {
    const fetchMock = mockFetch(200, { status: 'ready' });
    vi.stubGlobal('fetch', fetchMock);
    await startWorkspace('u1', 'p1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/start');
    expect(request.method).toBe('POST');
  });

  it('suspends a workspace on the detail route', async () => {
    const fetchMock = mockFetch(200, { status: 'idle' });
    vi.stubGlobal('fetch', fetchMock);
    await suspendWorkspace('u1', 'p1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1');
    expect(request.method).toBe('POST');
  });

  it('connects and returns the endpoint', async () => {
    vi.stubGlobal('fetch', mockFetch(200, { endpoint: 'http://10.0.0.1:17999' }));
    const endpoint = await connectWorkspace('u1', 'p1');
    expect(endpoint).toBe('http://10.0.0.1:17999');
  });

  it('destroys a workspace with a delete', async () => {
    const fetchMock = mockFetch(200, { success: true });
    vi.stubGlobal('fetch', fetchMock);
    await destroyWorkspace('u1', 'p1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1');
    expect(request.method).toBe('DELETE');
  });

  it('lists the sessions in a workspace', async () => {
    const fetchMock = mockFetch(200, [{ id: 'ses_1' }]);
    vi.stubGlobal('fetch', fetchMock);
    const sessions = await listWorkspaceSessions('u1', 'p1');
    expect(sessions).toEqual([{ id: 'ses_1' }]);
    expect((await lastRequest(fetchMock)).url).toBe('/api/v1/workspaces/u1/p1/sessions');
  });

  it('opens a session with an optional title', async () => {
    const fetchMock = mockFetch(201, { id: 'ses_1' });
    vi.stubGlobal('fetch', fetchMock);
    const session = await openWorkspaceSession('u1', 'p1', 'refactor');
    expect(session.id).toBe('ses_1');
    const request = await lastRequest(fetchMock);
    expect(request.method).toBe('POST');
    expect(request.body).toEqual({ title: 'refactor' });
  });

  it('prompts a workspace session', async () => {
    const fetchMock = mockFetch(200, { session_id: 'ses_1', finish_reason: 'stop' });
    vi.stubGlobal('fetch', fetchMock);
    const outcome = await promptWorkspace('u1', 'p1', {
      sessionId: 'ses_1',
      text: 'do the thing',
    });
    expect(outcome.finish_reason).toBe('stop');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/prompt');
    expect(request.body).toMatchObject({ session_id: 'ses_1', text: 'do the thing' });
  });

  it('interrupts the open session', async () => {
    const fetchMock = mockFetch(200, { interrupted: true });
    vi.stubGlobal('fetch', fetchMock);
    await interruptWorkspaceSession('u1', 'p1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/interrupt');
    expect(request.method).toBe('POST');
  });

  it('runs a command in a workspace', async () => {
    const fetchMock = mockFetch(200, { exit_code: 0, stdout: 'ok', stderr: '', timed_out: false });
    vi.stubGlobal('fetch', fetchMock);
    const result = await runWorkspaceCommand('u1', 'p1', {
      command: 'ls',
      args: ['-la'],
      cwd: '/workspace',
    });
    expect(result.stdout).toBe('ok');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/terminal');
    expect(request.body).toEqual({ command: 'ls', args: ['-la'], cwd: '/workspace' });
  });

  it('reports a terminal that timed out as a result, not an error', async () => {
    vi.stubGlobal(
      'fetch',
      mockFetch(200, { exit_code: -1, stdout: '', stderr: 'too slow', timed_out: true }),
    );
    const result = await runWorkspaceCommand('u1', 'p1', { command: 'sleep', timeout_secs: 1 });
    expect(result.timed_out).toBe(true);
  });

  it('lists tree changes with a directory query and keeps the version', async () => {
    const fetchMock = mockFetch(200, {
      head: 'abc123',
      version: 'v1',
      changes: [{ file: 'src/a.rs', additions: 1, deletions: 1, status: 'modified', binary: false, truncated: false, patch: '' }],
    });
    vi.stubGlobal('fetch', fetchMock);
    const diff = await workspaceTreeChanges('u1', 'p1', '/workspace');
    expect(diff.version).toBe('v1');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/diff?directory=%2Fworkspace');
  });

  it('requests a file patch for a specific diff version', async () => {
    const fetchMock = mockFetch(200, { head: 'abc123', version: 'v2', changes: [] });
    vi.stubGlobal('fetch', fetchMock);
    await workspaceFilePatch('u1', 'p1', '/workspace', 'src/a.rs', 'v2');
    const request = await lastRequest(fetchMock);
    expect(request.url).toBe('/api/v1/workspaces/u1/p1/diff?path=src%2Fa.rs&directory=%2Fworkspace&version=v2');
  });
});
