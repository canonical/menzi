import { http } from './client';
import { apiPaths } from '../routes';
import type { CreateProjectInput, ListProjectsInput, Project } from '../types';

export async function listProjects(input: ListProjectsInput = {}): Promise<Project[]> {
  const params = new URLSearchParams();
  if (input.search) params.set('search', input.search);
  const query = params.toString();
  return http.get<Project[]>(query ? `${apiPaths.projects.list()}?${query}` : apiPaths.projects.list());
}

export async function getProject(projectId: string): Promise<Project> {
  return http.get<Project>(apiPaths.projects.detail(projectId));
}

export async function createProject(input: CreateProjectInput): Promise<Project> {
  return http.post<Project>(apiPaths.projects.list(), {
    name: input.name,
    slug: input.slug,
    description: input.description ?? null,
    repository_url: input.repository_url ?? null,
  });
}
