import type { ListProjectsInput } from './types';

export const routes = {
  home: () => '/',
  projects: {
    list: () => '/projects',
    code: (projectId: string) => `/projects/${projectId}/code`,
    codeSession: (projectId: string, sessionId: string) =>
      `/projects/${projectId}/code?session=${sessionId}`,
    design: (projectId: string) => `/projects/${projectId}/design`,
    project: (projectId: string) => `/projects/${projectId}/project`,
    previews: (projectId: string) => `/projects/${projectId}/previews`,
  },
  environment: (workspaceId: string) => `/workspaces/${workspaceId}/environment`,
  admin: () => '/admin',
  settings: () => '/settings',
  auth: {
    login: () => '/login',
    register: () => '/register',
    forgot: () => '/forgot-password',
    reset: () => '/reset-password',
    callback: () => '/auth/callback',
  },
};

export type ProjectSection = 'code' | 'design' | 'project';

export const PROJECT_SECTIONS: ProjectSection[] = ['code', 'design', 'project'];

export function isProjectSection(value: string): value is ProjectSection {
  return (PROJECT_SECTIONS as string[]).includes(value);
}

export function projectSectionPath(projectId: string, section: ProjectSection): string {
  switch (section) {
    case 'design':
      return routes.projects.design(projectId);
    case 'project':
      return routes.projects.project(projectId);
    default:
      return routes.projects.code(projectId);
  }
}

export const apiPaths = {
  auth: {
    providers: () => '/api/v1/auth/providers',
    session: () => '/api/v1/auth/session',
    login: () => '/api/v1/auth/login',
    register: () => '/api/v1/auth/register',
    logout: () => '/api/v1/auth/logout',
    forgot: () => '/api/v1/auth/password/forgot',
    reset: () => '/api/v1/auth/password/reset',
    changePassword: () => '/api/v1/auth/password/change',
    sessions: () => '/api/v1/auth/sessions',
    session_: (deviceId: string) => `/api/v1/auth/sessions/${deviceId}`,
    oidcStart: (providerId: string) => `/api/v1/auth/oidc/${providerId}/start`,
  },
  projects: {
    list: () => '/api/v1/projects',
    detail: (projectId: string) => `/api/v1/projects/${projectId}`,
  },
  previews: {
    list: (projectId: string) => `/api/v1/projects/${projectId}/previews`,
    create: () => '/api/v1/previews',
    detail: (previewId: string) => `/api/v1/previews/${previewId}`,
    reset: (previewId: string) => `/api/v1/previews/${previewId}/reset`,
    restart: (previewId: string) => `/api/v1/previews/${previewId}/restart`,
  },
  workspaces: {
    ensure: () => '/api/v1/workspaces',
    detail: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}`,
    start: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/start`,
    connect: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/connect`,
    suspend: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}`,
    sessions: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/sessions`,
    prompt: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/prompt`,
    interrupt: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/interrupt`,
    terminal: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/terminal`,
    terminalInput: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/terminal/input`,
    terminalResize: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/terminal/resize`,
    terminalOutput: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/terminal/output`,
    diff: (userId: string, projectId: string) =>
      `/api/v1/workspaces/${userId}/${projectId}/diff`,
    forProject: (projectId: string) => `/api/v1/projects/${projectId}/workspaces`,
  },
  env: {
    health: () => '/api/env/health',
    specs: () => '/api/env/specs',
    status: () => '/api/env/status',
    launch: () => '/api/env/launch',
    relaunch: () => '/api/env/relaunch',
    logs: () => '/api/env/logs',
    exec: () => '/api/env/exec',
  },
  tunnel: {
    sessions: () => '/api/tunnel/sessions',
    transcript: (sessionId: string) => `/api/tunnel/${sessionId}/transcript`,
    status: (sessionId: string) => `/api/tunnel/${sessionId}/status`,
  },
};

export const queryKeys = {
  session: () => ['session'] as const,
  authProviders: () => ['auth', 'providers'] as const,
  devices: () => ['auth', 'devices'] as const,
  projects: {
    all: () => ['projects'] as const,
    filtered: (input: ListProjectsInput) =>
      ['projects', input.search ?? null] as const,
    detail: (projectId: string) => ['projects', projectId] as const,
  },
  previews: {
    forProject: (projectId: string) => ['projects', projectId, 'previews'] as const,
    detail: (previewId: string) => ['previews', previewId] as const,
  },
  workspaces: {
    detail: (userId: string, projectId: string) =>
      ['workspaces', userId, projectId] as const,
    forProject: (projectId: string) => ['workspaces', 'project', projectId] as const,
    diff: (userId: string, projectId: string, directory: string) =>
      ['workspaces', 'diff', userId, projectId, directory] as const,
    diffPatch: (userId: string, projectId: string, directory: string, path: string, version: string) =>
      ['workspaces', 'diff', userId, projectId, directory, path, version] as const,
    terminal: {
      snapshot: (userId: string, projectId: string) =>
        ['workspaces', 'terminal', userId, projectId, 'snapshot'] as const,
      output: (userId: string, projectId: string, after: number) =>
        ['workspaces', 'terminal', userId, projectId, 'output', after] as const,
    },
  },
  env: {
    specs: () => ['env', 'specs'] as const,
    status: (sessionId: string, environmentName: string) =>
      ['env', sessionId, environmentName, 'status'] as const,
    logs: (sessionId: string, environmentName: string, component: string) =>
      ['env', sessionId, environmentName, 'logs', component] as const,
  },
  tunnel: {
    transcript: (sessionId: string) => ['tunnel', sessionId, 'transcript'] as const,
  },
  opencode: {
    sessions: () => ['opencode', 'sessions'] as const,
    workspaceSessions: (userId: string, projectId: string) =>
      ['opencode', 'workspace-sessions', userId, projectId] as const,
    messages: (sessionId: string) => ['opencode', 'messages', sessionId] as const,
    models: (sessionId: string) => ['opencode', 'models', sessionId] as const,
    agents: (sessionId: string) => ['opencode', 'agents', sessionId] as const,
    /** One entry per patch request. Invalidate with `diffs` to clear them all. */
    diff: (sessionId: string, messageId?: string) =>
      ['opencode', 'diff', sessionId, messageId ?? null] as const,
    diffs: (sessionId: string) => ['opencode', 'diff', sessionId] as const,
  },
};
