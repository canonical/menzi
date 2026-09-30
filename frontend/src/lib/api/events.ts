export const OC_EVENT_STREAM = '/api/oc/event';

/**
 * The stream carries every event from one opencode, so it is narrowed to a
 * session where the proxy can route it. Without a session the proxy answers
 * from its single registered target, which is what an unbound session uses.
 */
export function eventStreamFor(sessionId: string | null | undefined): string {
  if (!sessionId) return OC_EVENT_STREAM;
  return `${OC_EVENT_STREAM}?session=${encodeURIComponent(sessionId)}`;
}

export interface OcEvent {
  id?: string;
  type?: string;
  data?: unknown;
}

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function firstString(source: Record<string, unknown>, keys: string[]): string | null {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'string' && value.length > 0) return value;
  }
  return null;
}

export interface SessionEvent {
  sessionId: string;
  messageId: string;
  partId: string;
  part: Record<string, unknown>;
  type: string;
}

export function toSessionEvent(event: OcEvent): SessionEvent | null {
  if (typeof event.type !== 'string') return null;
  if (!event.type.includes('part')) return null;

  const data = asRecord(event.data);
  const part = asRecord(data.part);
  if (typeof part.type !== 'string') return null;

  const sessionId =
    firstString(data, ['sessionID', 'sessionId', 'session_id']) ??
    firstString(part, ['sessionID', 'sessionId']);
  const messageId = firstString(part, ['messageID', 'messageId', 'message_id']);
  if (!sessionId || !messageId) return null;

  const partId = firstString(part, ['id', 'callID']) ?? `${messageId}:${part.type}`;

  return { sessionId, messageId, partId, part, type: event.type };
}
