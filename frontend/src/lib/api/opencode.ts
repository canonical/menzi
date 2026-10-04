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

function unwrapData<T>(value: T | DataEnvelope<T>): T {
  if (
    typeof value === 'object' &&
    value !== null &&
    'data' in (value as Record<string, unknown>)
  ) {
    return (value as DataEnvelope<T>).data;
  }
  return value as T;
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function asText(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function contentText(value: unknown): string {
  const rows = Array.isArray(value) ? value : [];
  return rows
    .map((entry) => asText(asRecord(entry).text))
    .filter((entry) => entry.length > 0)
    .join('\n');
}

function toolStateFrom(value: unknown) {
  const state = asRecord(value);
  const status = asText(state.status);
  const input = asRecord(state.input);
  if (status === 'completed') {
    return {
      status: 'completed' as const,
      input,
      output: contentText(state.content),
    };
  }
  if (status === 'running') {
    return {
      status: 'running' as const,
      input,
    };
  }
  if (status === 'error') {
    return {
      status: 'error' as const,
      input,
      error: asText(state.error) || contentText(state.content),
    };
  }
  return {
    status: 'pending' as const,
    input,
  };
}

function partsFrom(raw: Record<string, unknown>) {
  if (Array.isArray(raw.parts)) {
    return raw.parts as OpencodeMessage['parts'];
  }
  const content = Array.isArray(raw.content) ? raw.content : [];
  return content
    .map((entry): OpencodeMessage['parts'][number] | null => {
      const part = asRecord(entry);
      const kind = asText(part.type);
      if (kind === 'text') {
        const text = asText(part.text);
        return text ? { type: 'text', text } : null;
      }
      if (kind === 'reasoning') {
        const text = asText(part.text);
        return text
          ? {
              type: 'reasoning',
              text,
              time: asRecord(part.time) as { start?: number; end?: number },
            }
          : null;
      }
      if (kind === 'tool') {
        const id = asText(part.id);
        if (!id) return null;
        return {
          type: 'tool',
          id,
          tool: asText(part.name) || 'tool',
          state: toolStateFrom(part.state),
          callID: asText(part.callID) || undefined,
        };
      }
      return null;
    })
    .filter((entry): entry is OpencodeMessage['parts'][number] => entry !== null);
}

function toSession(raw: Record<string, unknown>): OpencodeSession {
  const time = typeof raw.time === 'object' && raw.time !== null
    ? raw.time as { created?: number; updated?: number }
    : undefined;
  const location = typeof raw.location === 'object' && raw.location !== null
    ? raw.location as { directory?: string }
    : undefined;
  return {
    id: String(raw.id ?? ''),
    projectID: typeof raw.projectID === 'string' ? raw.projectID : undefined,
    cost: typeof raw.cost === 'number' ? raw.cost : undefined,
    time,
    location,
  };
}

function toMessage(raw: Record<string, unknown>, sessionId: string): OpencodeMessage {
  const nestedInfo = asRecord(raw.info);
  if (Object.keys(nestedInfo).length > 0 && Array.isArray(raw.parts)) {
    const role = typeof nestedInfo.role === 'string' ? nestedInfo.role : 'assistant';
    const id = typeof nestedInfo.id === 'string' ? nestedInfo.id : `${sessionId}:${Math.random().toString(36).slice(2)}`;
    return {
      info: {
        id,
        sessionID: typeof nestedInfo.sessionID === 'string' ? nestedInfo.sessionID : sessionId,
        role,
        agent: typeof nestedInfo.agent === 'string' ? nestedInfo.agent : undefined,
        model: typeof nestedInfo.model === 'object' && nestedInfo.model !== null
          ? nestedInfo.model as { providerID?: string; modelID?: string; id?: string }
          : undefined,
        finish: typeof nestedInfo.finish === 'string' ? nestedInfo.finish : undefined,
        error: typeof nestedInfo.error === 'object' && nestedInfo.error !== null
          ? nestedInfo.error as { name?: string; data?: { message?: string } | string; message?: string }
          : undefined,
        cost: typeof nestedInfo.cost === 'number' ? nestedInfo.cost : undefined,
        tokens: typeof nestedInfo.tokens === 'object' && nestedInfo.tokens !== null
          ? nestedInfo.tokens as Record<string, number>
          : undefined,
        time: typeof nestedInfo.time === 'object' && nestedInfo.time !== null
          ? nestedInfo.time as { created?: number; completed?: number }
          : undefined,
      },
      parts: raw.parts as OpencodeMessage['parts'],
    };
  }

  const role = typeof raw.type === 'string' ? raw.type : 'assistant';
  const id = typeof raw.id === 'string' ? raw.id : `${sessionId}:${Math.random().toString(36).slice(2)}`;
  const text = typeof raw.text === 'string'
    ? raw.text
    : typeof (raw.payload as { text?: unknown } | undefined)?.text === 'string'
      ? (raw.payload as { text: string }).text
      : '';
  const parts = partsFrom(raw);
  if (parts.length === 0 && text) {
    parts.push({ type: 'text', text });
  }
  return {
    info: {
      id,
      sessionID: typeof raw.sessionID === 'string' ? raw.sessionID : sessionId,
      role,
      agent: typeof raw.agent === 'string' ? raw.agent : undefined,
      model: typeof raw.model === 'object' && raw.model !== null
        ? raw.model as { providerID?: string; modelID?: string; id?: string }
        : undefined,
      finish: typeof raw.finish === 'string' ? raw.finish : undefined,
      error: typeof raw.error === 'object' && raw.error !== null
        ? raw.error as { name?: string; data?: { message?: string } | string; message?: string }
        : undefined,
      cost: typeof raw.cost === 'number' ? raw.cost : undefined,
      tokens: typeof raw.tokens === 'object' && raw.tokens !== null
        ? raw.tokens as Record<string, number>
        : undefined,
      time: typeof raw.time === 'object' && raw.time !== null
        ? raw.time as { created?: number; completed?: number }
        : undefined,
    },
    parts,
  };
}

export async function createSession(): Promise<OpencodeSession> {
  const response = await http.post<OpencodeSession | DataEnvelope<OpencodeSession>>('/api/session', {});
  return toSession(unwrapData(response) as unknown as Record<string, unknown>);
}

export async function listSessions(): Promise<OpencodeSession[]> {
  const response = await http.get<OpencodeSession[] | DataEnvelope<OpencodeSession[]>>('/api/session');
  return unwrapData(response).map((entry) => toSession(entry as unknown as Record<string, unknown>));
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
  const response = await http.get<OpencodeMessage[] | DataEnvelope<OpencodeMessage[]>>(
    `/api/session/${sessionId}/message`,
  );
  const list = unwrapData(response) as unknown as Record<string, unknown>[];
  const messages = list
    .map((entry) => toMessage(entry, sessionId))
    .sort((a, b) => {
      const at = a.info.time?.created ?? 0;
      const bt = b.info.time?.created ?? 0;
      return at - bt;
    });
  return { messages };
}

export async function sendPrompt(input: {
  sessionId: string;
  text: string;
  modelId?: string;
  modelProvider?: string;
  agent?: string;
}): Promise<OpencodeMessage> {
  const body: Record<string, unknown> = { text: input.text };
  if (input.modelId) {
    body.model = { modelID: input.modelId, providerID: input.modelProvider };
  }
  if (input.agent) body.agent = input.agent;
  const response = await http.post<OpencodeMessage | DataEnvelope<OpencodeMessage>>(
    `/api/session/${input.sessionId}/prompt`,
    body,
  );
  return toMessage(unwrapData(response) as unknown as Record<string, unknown>, input.sessionId);
}

export async function interruptSession(sessionId: string): Promise<void> {
  await http.post(`/api/session/${sessionId}/interrupt`, {});
}

export function eventStreamUrl(): string {
  return '/api/event';
}
