import { http } from './client';
import type {
  OpencodeAgent,
  OpencodeMessage,
  OpencodeModel,
  OpencodeSession,
  OpencodeVcsFile,
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

export async function getVcsStatus(): Promise<OpencodeVcsFile[]> {
  const response = await http.get<Record<string, unknown>[]>('/vcs/status');
  return response.map((entry) => ({
    file: String(entry.file ?? entry.path ?? ''),
    status: typeof entry.status === 'string' ? entry.status : undefined,
    additions: typeof entry.additions === 'number' ? entry.additions : undefined,
    deletions: typeof entry.deletions === 'number' ? entry.deletions : undefined,
  }));
}

export interface FileDiff {
  path: string;
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface DiffLine {
  kind: 'context' | 'add' | 'remove';
  text: string;
}

export async function getSessionDiff(sessionId: string): Promise<FileDiff[]> {
  const response = await http.get<Record<string, unknown>[]>(`/session/${sessionId}/diff`);
  return response.map(normaliseDiff);
}

function normaliseDiff(raw: Record<string, unknown>): FileDiff {
  const additions = numberOf(raw, 'additions');
  const deletions = numberOf(raw, 'deletions');
  const hunks: DiffHunk[] = [];
  const rawHunks = Array.isArray(raw.hunks) ? raw.hunks : [];
  for (const entry of rawHunks) {
    if (typeof entry !== 'object' || entry === null) continue;
    const hunk = entry as Record<string, unknown>;
    const lines: DiffLine[] = [];
    const rawLines = Array.isArray(hunk.lines) ? hunk.lines : [];
    for (const rawLine of rawLines) {
      if (typeof rawLine !== 'object' || rawLine === null) continue;
      const line = rawLine as Record<string, unknown>;
      const kind = line.kind ?? line.type;
      if (kind !== 'add' && kind !== 'remove' && kind !== 'context') continue;
      lines.push({ kind, text: String(line.text ?? '') });
    }
    hunks.push({ header: String(hunk.header ?? ''), lines });
  }
  return { path: String(raw.path ?? raw.file ?? ''), additions, deletions, hunks };
}

function numberOf(raw: Record<string, unknown>, key: string): number {
  const value = raw[key];
  return typeof value === 'number' ? value : 0;
}

export function eventStreamUrl(): string {
  return '/api/event';
}
