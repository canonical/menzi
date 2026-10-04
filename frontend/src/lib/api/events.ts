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
  created?: number;
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
  mode?: 'append';
}

export function toSessionEvent(event: OcEvent): SessionEvent | null {
  if (typeof event.type !== 'string') return null;
  const data = asRecord(event.data);

  if (event.type === 'session.reasoning.started') {
    const sessionId = firstString(data, ['sessionID', 'sessionId', 'session_id']);
    const messageId = firstString(data, ['assistantMessageID', 'assistantMessageId', 'messageID', 'messageId']);
    if (!sessionId || !messageId) return null;
    const ordinal = typeof data.ordinal === 'number' ? data.ordinal : 0;
    return {
      sessionId,
      messageId,
      partId: `${messageId}:reasoning:${ordinal}`,
      part: {
        type: 'reasoning',
        text: '',
        time: typeof event.created === 'number' ? { start: event.created } : undefined,
      },
      type: event.type,
    };
  }

  if (event.type === 'session.text.delta' || event.type === 'session.text.ended') {
    const sessionId = firstString(data, ['sessionID', 'sessionId', 'session_id']);
    const messageId = firstString(data, ['assistantMessageID', 'assistantMessageId', 'messageID', 'messageId']);
    if (!sessionId || !messageId) return null;
    const ordinal = typeof data.ordinal === 'number' ? data.ordinal : 0;
    const text = firstString(data, ['delta', 'text']) ?? '';
    return {
      sessionId,
      messageId,
      partId: `${messageId}:text:${ordinal}`,
      part: { type: 'text', text },
      type: event.type,
      mode: event.type === 'session.text.delta' ? 'append' : undefined,
    };
  }

  if (event.type === 'session.reasoning.delta' || event.type === 'session.reasoning.ended') {
    const sessionId = firstString(data, ['sessionID', 'sessionId', 'session_id']);
    const messageId = firstString(data, ['assistantMessageID', 'assistantMessageId', 'messageID', 'messageId']);
    if (!sessionId || !messageId) return null;
    const ordinal = typeof data.ordinal === 'number' ? data.ordinal : 0;
    const text = firstString(data, ['delta', 'text']) ?? '';
    return {
      sessionId,
      messageId,
      partId: `${messageId}:reasoning:${ordinal}`,
      part: {
        type: 'reasoning',
        text,
        ...(event.type === 'session.reasoning.ended' && typeof event.created === 'number'
          ? { time: { end: event.created } }
          : {}),
      },
      type: event.type,
      mode: event.type === 'session.reasoning.delta' ? 'append' : undefined,
    };
  }

  if (
    event.type === 'session.tool.input.started' ||
    event.type === 'session.tool.input.ended' ||
    event.type === 'session.tool.called' ||
    event.type === 'session.tool.progress' ||
    event.type === 'session.tool.success' ||
    event.type === 'session.tool.error'
  ) {
    const sessionId = firstString(data, ['sessionID', 'sessionId', 'session_id']);
    const messageId = firstString(data, ['assistantMessageID', 'assistantMessageId', 'messageID', 'messageId']);
    const callID = firstString(data, ['id', 'callID', 'callId']);
    if (!sessionId || !messageId || !callID) return null;

    const name = firstString(data, ['name', 'tool']) ?? 'tool';
    const input = asRecord(data.input);
    const metadata = asRecord(data.metadata);
    const content = Array.isArray(data.content) ? data.content : [];
    const output = content
      .map((entry) => firstString(asRecord(entry), ['text']) ?? '')
      .filter((entry) => entry.length > 0)
      .join('\n');

    if (event.type === 'session.tool.success') {
      return {
        sessionId,
        messageId,
        partId: callID,
        part: {
          type: 'tool',
          id: callID,
          callID,
          tool: name,
          state: { status: 'completed', input, output, metadata },
        },
        type: event.type,
      };
    }

    if (event.type === 'session.tool.error') {
      const error = firstString(data, ['error', 'message']) ?? output;
      return {
        sessionId,
        messageId,
        partId: callID,
        part: {
          type: 'tool',
          id: callID,
          callID,
          tool: name,
          state: { status: 'error', input, error, metadata },
        },
        type: event.type,
      };
    }

    if (event.type === 'session.tool.input.ended') {
      const parsed = firstString(data, ['text']);
      let parsedInput = input;
      if (typeof parsed === 'string' && parsed.trim().startsWith('{')) {
        try {
          const value = JSON.parse(parsed) as Record<string, unknown>;
          parsedInput = asRecord(value);
        } catch {
          parsedInput = input;
        }
      }
      return {
        sessionId,
        messageId,
        partId: callID,
        part: {
          type: 'tool',
          id: callID,
          callID,
          tool: name,
          state: { status: 'running', input: parsedInput },
        },
        type: event.type,
      };
    }

    return {
      sessionId,
      messageId,
      partId: callID,
      part: {
        type: 'tool',
        id: callID,
        callID,
        tool: name,
        state: {
          status: 'running',
          input,
          ...(Object.keys(metadata).length > 0 ? { metadata } : {}),
        },
      },
      type: event.type,
    };
  }

  if (!event.type.includes('part')) return null;

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
