import { http } from './client';
import { apiPaths } from '../routes';
import type { Preview } from '../types';

export interface CreatePreviewInput {
  projectId: string;
  commitSha?: string;
  branch?: string;
  mode?: 'pinned' | 'live';
}

export async function listPreviews(projectId: string): Promise<Preview[]> {
  return http.get<Preview[]>(apiPaths.previews.list(projectId));
}

export async function createPreview(input: CreatePreviewInput): Promise<Preview> {
  return http.post<Preview>(apiPaths.previews.create(), {
    project_id: input.projectId,
    commit_sha: input.commitSha,
    branch: input.branch,
    mode: input.mode ?? 'pinned',
  });
}

export async function getPreview(previewId: string): Promise<Preview> {
  return http.get<Preview>(apiPaths.previews.detail(previewId));
}

export async function resetPreview(previewId: string): Promise<void> {
  await http.post(apiPaths.previews.reset(previewId), {});
}

export async function restartPreview(previewId: string): Promise<void> {
  await http.post(apiPaths.previews.restart(previewId), {});
}

export async function teardownPreview(previewId: string): Promise<void> {
  await http.delete(apiPaths.previews.detail(previewId));
}