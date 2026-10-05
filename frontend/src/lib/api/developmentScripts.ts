import { http } from './client';
import { apiPaths } from '../routes';
import type { DevelopmentScript } from '../types';

export interface CreateDevelopmentScriptInput {
  name: string;
  slug: string;
  body: string;
  relative_path?: string;
}

export interface UpdateDevelopmentScriptInput {
  name?: string;
  slug?: string;
  body?: string;
  relative_path?: string;
}

export function listDevelopmentScripts(projectId: string): Promise<DevelopmentScript[]> {
  return http.get<DevelopmentScript[]>(apiPaths.projects.developmentScripts(projectId));
}

export function createDevelopmentScript(
  projectId: string,
  input: CreateDevelopmentScriptInput,
): Promise<DevelopmentScript> {
  return http.post<DevelopmentScript>(apiPaths.projects.developmentScripts(projectId), input);
}

export function updateDevelopmentScript(
  projectId: string,
  scriptId: string,
  input: UpdateDevelopmentScriptInput,
): Promise<DevelopmentScript> {
  return http.put<DevelopmentScript>(apiPaths.projects.developmentScript_(projectId, scriptId), input);
}

export function deleteDevelopmentScript(projectId: string, scriptId: string): Promise<void> {
  return http.delete<void>(apiPaths.projects.developmentScript_(projectId, scriptId));
}
