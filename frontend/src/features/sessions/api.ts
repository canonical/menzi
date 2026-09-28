import { api } from '../../lib/api';
import type { Session, Message } from '../../lib/types';

export async function listSessions(projectId: string): Promise<Session[]> {
  return api.get<Session[]>(`/api/v1/projects/${projectId}/sessions`);
}

export async function startSession(projectId: string, branch?: string, task?: string): Promise<Session> {
  return api.post<Session>(`/api/v1/projects/${projectId}/sessions`, { branch, task });
}

export async function getSession(id: string): Promise<Session> {
  return api.get<Session>(`/api/v1/sessions/${id}`);
}

export async function sendPrompt(sessionId: string, prompt: string): Promise<void> {
  await api.post(`/api/v1/sessions/${sessionId}/prompts`, { prompt });
}

export async function getMessages(sessionId: string): Promise<Message[]> {
  return api.get<Message[]>(`/api/v1/sessions/${sessionId}/messages`);
}
