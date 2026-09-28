import { api } from '../../lib/api';
import type { Project } from '../../lib/types';

export async function listProjects(): Promise<Project[]> {
  return api.get<Project[]>('/api/v1/projects');
}

export async function getProject(id: string): Promise<Project> {
  return api.get<Project>(`/api/v1/projects/${id}`);
}

export async function createProject(name: string, slug: string): Promise<Project> {
  return api.post<Project>('/api/v1/projects', { name, slug });
}
