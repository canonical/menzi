import { http } from './client';
import { apiPaths } from '../routes';
import type { PromptOutcome, Workspace, WorkspaceSession } from '../types';

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

export async function runWorkspaceCommand(
  userId: string,
  projectId: string,
  spec: TerminalSpec,
): Promise<TerminalResult> {
  return http.post<TerminalResult>(apiPaths.workspaces.terminal(userId, projectId), spec);
}

function statusOf(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const status = (error as { status?: unknown }).status;
  return typeof status === 'number' ? status : undefined;
}
