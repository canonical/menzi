import { api } from '../../lib/api';
import type { Preview } from '../../lib/types';

export async function listPreviews(projectId: string): Promise<Preview[]> {
  return api.get<Preview[]>(`/api/v1/projects/${projectId}/previews`);
}

export async function createPreview(projectId: string, commitSha?: string, branch?: string, mode: string = 'pinned'): Promise<Preview> {
  return api.post<Preview>('/api/v1/previews', { project_id: projectId, commit_sha: commitSha, branch, mode });
}

export async function getPreview(id: string): Promise<Preview> {
  return api.get<Preview>(`/api/v1/previews/${id}`);
}

export async function resetPreview(id: string): Promise<void> {
  await api.post(`/api/v1/previews/${id}/reset`, {});
}

export async function restartPreview(id: string): Promise<void> {
  await api.post(`/api/v1/previews/${id}/restart`, {});
}

export async function teardownPreview(id: string): Promise<void> {
  await api.delete(`/api/v1/previews/${id}`);
}
