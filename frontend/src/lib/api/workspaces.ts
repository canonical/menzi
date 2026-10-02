import { http } from './client';
import { ApiError } from './errors';
import { apiPaths } from '../routes';
import type {
  PromptOutcome,
  Workspace,
  WorkspaceSession,
  WorkspaceTerminalOutput,
  WorkspaceTerminalSnapshot,
} from '../types';

export interface EnsureWorkspaceInput {
  projectId: string;
  name?: string;
  branch?: string;
  commitSha?: string;
}

export interface TerminalSpec {
  command: string;
  args?: string[];
  cwd?: string;
  timeout_secs?: number;
}

export interface TerminalResult {
  exit_code: number;
  stdout: string;
  stderr: string;
  timed_out: boolean;
}

export interface TerminalInputSpec {
  input: string;
}

export interface TerminalResizeSpec {
  cols?: number;
  rows?: number;
}

export interface WorkspacePty {
  id: string;
  title: string;
  command: string;
  args: string[];
  cwd: string;
  status: 'running' | 'exited';
  pid: number;
  exitCode?: number;
  size: { rows: number; cols: number };
}

const API_BASE = import.meta.env.VITE_API_URL || '';
const WORKSPACE_ENDPOINT_HEADER = 'x-menzi-workspace-endpoint';

async function ptyRequest<T>(
  path: string,
  endpoint: string,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  body?: unknown,
): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    method,
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      [WORKSPACE_ENDPOINT_HEADER]: endpoint,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  if (!response.ok) {
    let message = response.statusText || `Request failed (${response.status})`;
    if (text.trim()) {
      try {
        const parsed = JSON.parse(text) as { error?: { message?: string } | string; message?: string };
        message = typeof parsed.error === 'string'
          ? parsed.error
          : parsed.error?.message ?? parsed.message ?? message;
      } catch {
        message = text;
      }
    }
    throw new ApiError(response.status, message);
  }
  if (!text.trim()) return undefined as T;
  return JSON.parse(text) as T;
}

export async function listWorkspacePtys(endpoint: string): Promise<WorkspacePty[]> {
  const response = await ptyRequest<{ data: WorkspacePty[] }>('/api/pty?location[directory]=/workspace', endpoint, 'GET');
  return response.data ?? [];
}

export async function createWorkspacePty(
  endpoint: string,
  spec: { command: string; args?: string[]; cwd?: string; title: string; env?: Record<string, string> },
): Promise<WorkspacePty> {
  const response = await ptyRequest<{ data: WorkspacePty }>('/api/pty?location[directory]=/workspace', endpoint, 'POST', {
    command: spec.command,
    args: spec.args ?? [],
    cwd: spec.cwd,
    title: spec.title,
    env: spec.env ?? {},
  });
  return response.data;
}

export async function workspacePtyConnectToken(endpoint: string, ptyId: string): Promise<string> {
  const response = await ptyRequest<{ data: { ticket: string } }>(`/api/pty/${encodeURIComponent(ptyId)}/connect-token?location[directory]=/workspace`, endpoint, 'POST');
  return response.data.ticket;
}

export async function resizeWorkspacePty(
  endpoint: string,
  ptyId: string,
  size: { cols: number; rows: number },
): Promise<WorkspacePty> {
  const response = await ptyRequest<{ data: WorkspacePty }>(`/api/pty/${encodeURIComponent(ptyId)}?location[directory]=/workspace`, endpoint, 'PUT', {
    size: { cols: size.cols, rows: size.rows },
  });
  return response.data;
}

export function workspacePtySocketUrl(ptyId: string, ticket?: string): string {
  const base = (import.meta.env.VITE_API_URL || '').trim();
  const origin = base ? new URL(base, window.location.origin).origin : window.location.origin;
  const protocol = origin.startsWith('https://') ? 'wss://' : 'ws://';
  const host = origin.replace(/^https?:\/\//, '');
  const query = new URLSearchParams({ 'location[directory]': '/workspace' });
  if (ticket) query.set('ticket', ticket);
  return `${protocol}${host}/api/pty/${encodeURIComponent(ptyId)}/connect?${query.toString()}`;
}

export class WorkspaceNotFoundError extends Error {
  readonly status = 404;

  constructor() {
    super('no workspace');
    this.name = 'WorkspaceNotFoundError';
  }
}

export async function ensureWorkspace(input: EnsureWorkspaceInput): Promise<Workspace> {
  return http.post<Workspace>(apiPaths.workspaces.ensure(), {
    project_id: input.projectId,
    name: input.name,
    branch: input.branch,
    commit_sha: input.commitSha,
  });
}

export async function getWorkspace(userId: string, projectId: string): Promise<Workspace> {
  try {
    return await http.get<Workspace>(apiPaths.workspaces.detail(userId, projectId));
  } catch (error) {
    if (statusOf(error) === 404) throw new WorkspaceNotFoundError();
    throw error;
  }
}

export async function listWorkspacesForProject(projectId: string): Promise<Workspace[]> {
  return http.get<Workspace[]>(apiPaths.workspaces.forProject(projectId));
}

export async function startWorkspace(
  userId: string,
  projectId: string,
): Promise<Workspace> {
  return http.post<Workspace>(apiPaths.workspaces.start(userId, projectId), {});
}

export async function suspendWorkspace(
  userId: string,
  projectId: string,
): Promise<Workspace> {
  return http.post<Workspace>(apiPaths.workspaces.suspend(userId, projectId), {});
}

export async function connectWorkspace(
  userId: string,
  projectId: string,
): Promise<string> {
  const response = await http.post<{ endpoint: string }>(
    apiPaths.workspaces.connect(userId, projectId),
    {},
  );
  return response.endpoint;
}

export async function destroyWorkspace(userId: string, projectId: string): Promise<void> {
  await http.delete(apiPaths.workspaces.detail(userId, projectId));
}

export async function listWorkspaceSessions(
  userId: string,
  projectId: string,
): Promise<WorkspaceSession[]> {
  return http.get<WorkspaceSession[]>(apiPaths.workspaces.sessions(userId, projectId));
}

export async function openWorkspaceSession(
  userId: string,
  projectId: string,
  title?: string,
): Promise<WorkspaceSession> {
  return http.post<WorkspaceSession>(apiPaths.workspaces.sessions(userId, projectId), {
    title,
  });
}

export async function promptWorkspace(
  userId: string,
  projectId: string,
  input: { sessionId?: string; text: string; title?: string },
): Promise<PromptOutcome> {
  return http.post<PromptOutcome>(apiPaths.workspaces.prompt(userId, projectId), {
    session_id: input.sessionId,
    text: input.text,
    title: input.title,
  });
}

export async function interruptWorkspaceSession(
  userId: string,
  projectId: string,
): Promise<void> {
  await http.post(apiPaths.workspaces.interrupt(userId, projectId), {});
}

export interface TreeChange {
  file: string;
  previous: string | null;
  additions: number;
  deletions: number;
  status: string;
  binary: boolean;
  truncated: boolean;
  patch: string;
}

export interface TreeDiff {
  head: string;
  version: string;
  changes: TreeChange[];
}

export async function workspaceTreeChanges(
  userId: string,
  projectId: string,
  directory?: string,
): Promise<TreeDiff> {
  const query = directory ? `?directory=${encodeURIComponent(directory)}` : '';
  return http.get<TreeDiff>(`${apiPaths.workspaces.diff(userId, projectId)}${query}`);
}

export async function workspaceFilePatch(
  userId: string,
  projectId: string,
  directory: string | undefined,
  path: string,
  version?: string,
): Promise<TreeDiff> {
  const query = new URLSearchParams({ path });
  if (directory) query.set('directory', directory);
  if (version) query.set('version', version);
  return http.get<TreeDiff>(`${apiPaths.workspaces.diff(userId, projectId)}?${query.toString()}`);
}

export async function runWorkspaceCommand(
  userId: string,
  projectId: string,
  spec: TerminalSpec,
): Promise<TerminalResult> {
  return http.post<TerminalResult>(apiPaths.workspaces.terminal(userId, projectId), spec);
}

export async function getWorkspaceTerminal(
  userId: string,
  projectId: string,
): Promise<WorkspaceTerminalSnapshot> {
  return http.get<WorkspaceTerminalSnapshot>(apiPaths.workspaces.terminal(userId, projectId));
}

export async function sendWorkspaceTerminalInput(
  userId: string,
  projectId: string,
  spec: TerminalInputSpec,
): Promise<WorkspaceTerminalOutput> {
  return http.post<WorkspaceTerminalOutput>(apiPaths.workspaces.terminalInput(userId, projectId), spec);
}

export async function resizeWorkspaceTerminal(
  userId: string,
  projectId: string,
  spec: TerminalResizeSpec,
): Promise<WorkspaceTerminalSnapshot> {
  return http.post<WorkspaceTerminalSnapshot>(apiPaths.workspaces.terminalResize(userId, projectId), spec);
}

export async function readWorkspaceTerminalOutput(
  userId: string,
  projectId: string,
  after: number,
): Promise<WorkspaceTerminalOutput> {
  return http.get<WorkspaceTerminalOutput>(`${apiPaths.workspaces.terminalOutput(userId, projectId)}?after=${after}`);
}

function statusOf(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const status = (error as { status?: unknown }).status;
  return typeof status === 'number' ? status : undefined;
}
