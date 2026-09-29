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
  inbox: () => '/inbox',
  admin: () => '/admin',
  settings: () => '/settings',
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
  orgs: {
    list: () => '/api/v1/orgs',
    detail: (orgId: string) => `/api/v1/orgs/${orgId}`,
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
  orgs: {
    all: () => ['orgs'] as const,
    detail: (orgId: string) => ['orgs', orgId] as const,
  },
  projects: {
    all: () => ['projects'] as const,
    filtered: (input: ListProjectsInput) =>
      ['projects', input.orgId ?? null, input.search ?? null] as const,
    detail: (projectId: string) => ['projects', projectId] as const,
  },
  previews: {
    forProject: (projectId: string) => ['projects', projectId, 'previews'] as const,
    detail: (previewId: string) => ['previews', previewId] as const,
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
    messages: (sessionId: string) => ['opencode', 'messages', sessionId] as const,
    models: () => ['opencode', 'models'] as const,
    agents: () => ['opencode', 'agents'] as const,
    vcs: () => ['opencode', 'vcs', 'status'] as const,
    diff: (sessionId: string) => ['opencode', 'diff', sessionId] as const,
  },
};