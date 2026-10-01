import { http } from './client';
import type {
  OpencodeAgent,
  OpencodeMessage,
  OpencodeModel,
  OpencodeSession,
} from '../types';

interface DataEnvelope<T> {
  location?: { directory?: string };
  data: T;
}

export async function createSession(): Promise<OpencodeSession> {
  const response = await http.post<OpencodeSession>('/session', {});
  return response;
}

export async function listSessions(): Promise<OpencodeSession[]> {
  const response = await http.get<OpencodeSession[]>('/session');
  return response;
}

export async function listAgents(sessionId?: string): Promise<OpencodeAgent[]> {
  const query = sessionQuery(sessionId);
  const response = await http.get<OpencodeAgent[]>('/agent' + query);
  return response;
}

export async function listModels(sessionId?: string): Promise<OpencodeModel[]> {
  const query = sessionQuery(sessionId);
  const response = await http.get<DataEnvelope<OpencodeModel[]>>('/api/model' + query);
  return response.data.filter((model) => model.status !== 'deprecated');
}

function sessionQuery(sessionId?: string): string {
  return sessionId ? `?session=${encodeURIComponent(sessionId)}` : '';
}

export async function listMessages(
  sessionId: string,
): Promise<{ messages: OpencodeMessage[] }> {
  const response = await http.get<OpencodeMessage[]>(`/session/${sessionId}/message`);
  return { messages: response };
}

export async function sendPrompt(input: {
  sessionId: string;
  text: string;
  modelId?: string;
  modelProvider?: string;
  agent?: string;
}): Promise<OpencodeMessage> {
  const body: Record<string, unknown> = { parts: [{ type: 'text', text: input.text }] };
  if (input.modelId) {
    body.model = { modelID: input.modelId, providerID: input.modelProvider };
  }
  if (input.agent) body.agent = input.agent;
  const response = await http.post<OpencodeMessage>(
    `/session/${input.sessionId}/message`,
    body,
  );
  return response;
}

export async function interruptSession(sessionId: string): Promise<void> {
  await http.post(`/session/${sessionId}/interrupt`, {});
}

export function eventStreamUrl(): string {
  return '/api/event';
}
