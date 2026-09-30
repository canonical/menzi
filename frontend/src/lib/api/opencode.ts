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

export async function listAgents(): Promise<OpencodeAgent[]> {
  const response = await http.get<OpencodeAgent[]>('/agent');
  return response;
}

export async function listModels(): Promise<OpencodeModel[]> {
  const response = await http.get<DataEnvelope<OpencodeModel[]>>('/api/model');
  return response.data.filter((model) => model.status !== 'deprecated');
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

export interface FileDiff {
  file: string;
  additions: number;
  deletions: number;
  status?: string;
  /** The unified diff for the file, in the format `git diff` produces. */
  patch: string;
}

/**
 * The changes a session made. `messageID` narrows it to a single turn, which
 * is how a file that is missing its patch gets one: the session level response
 * is only the union of the per turn diffs.
 */
export async function getSessionDiff(sessionId: string, messageId?: string): Promise<FileDiff[]> {
  const query = messageId ? `?messageID=${encodeURIComponent(messageId)}` : '';
  const response = await http.get<Record<string, unknown>[]>(
    `/session/${sessionId}/diff${query}`,
  );
  return response.map(normaliseDiff);
}

function count(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

function normaliseDiff(raw: Record<string, unknown>): FileDiff {
  return {
    file: String(raw.file ?? raw.path ?? ''),
    additions: count(raw.additions),
    deletions: count(raw.deletions),
    status: typeof raw.status === 'string' ? raw.status : undefined,
    patch: typeof raw.patch === 'string' ? raw.patch : '',
  };
}

export function eventStreamUrl(): string {
  return '/api/event';
}
