import { api } from '../../lib/api';
import type { Environment } from '../../lib/types';

export async function getEnvironment(workspaceId: string): Promise<Environment> {
  return api.get<Environment>(`/api/v1/workspaces/${workspaceId}/environment`);
}

export async function launchEnvironment(workspaceId: string, name: string): Promise<Environment> {
  return api.post<Environment>(`/api/v1/workspaces/${workspaceId}/environment/launch`, { name });
}

export async function relaunchEnvironment(workspaceId: string, resetData: boolean): Promise<Environment> {
  return api.post<Environment>(`/api/v1/workspaces/${workspaceId}/environment/relaunch`, { reset_data: resetData });
}

export async function teardownEnvironment(workspaceId: string): Promise<void> {
  await api.post(`/api/v1/workspaces/${workspaceId}/environment/teardown`, {});
}
