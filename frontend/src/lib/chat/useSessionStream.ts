import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { OC_EVENT_STREAM, toSessionEvent, type SessionEvent } from '../api/events';
import { queryKeys } from '../routes';
import type { MessageInfo, MessagePart, OpencodeMessage, ToolPart, ToolState } from '../types';

interface CacheShape {
  messages: OpencodeMessage[];
}

function partFrom(event: SessionEvent): MessagePart | null {
  const raw = event.part;
  const type = raw.type;

  if (type === 'text' || type === 'reasoning') {
    return typeof raw.text === 'string' ? { type, text: raw.text } : null;
  }

  if (type === 'tool') {
    const part: ToolPart = {
      type: 'tool',
      id: event.partId,
      sessionID: event.sessionId,
      messageID: event.messageId,
      tool: typeof raw.tool === 'string' ? raw.tool : 'tool',
    };
    if (typeof raw.callID === 'string') part.callID = raw.callID;
    if (typeof raw.state === 'object' && raw.state !== null) {
      part.state = raw.state as ToolState;
    }
    return part;
  }

  if (type === 'compaction') {
    return { type: 'compaction', status: typeof raw.status === 'string' ? raw.status : undefined };
  }

  return null;
}

function infoFor(messageId: string, sessionId: string): MessageInfo {
  return { id: messageId, sessionID: sessionId, role: 'assistant' };
}

function samePart(existing: MessagePart, next: MessagePart): boolean {
  if (existing.type !== 'tool' || next.type !== 'tool') return false;
  return existing.id === next.id;
}

function withPart(message: OpencodeMessage, part: MessagePart): OpencodeMessage {
  const parts = message.parts ?? [];
  const index = parts.findIndex((existing) => samePart(existing, part));

  if (index === -1) return { ...message, parts: [...parts, part] };

  const next = [...parts];
  next[index] = part;
  return { ...message, parts: next };
}

function ensureMessage(
  messages: OpencodeMessage[],
  messageId: string,
  sessionId: string,
): OpencodeMessage[] {
  if (messages.some((message) => message.info.id === messageId)) return messages;
  return [...messages, { info: infoFor(messageId, sessionId), parts: [] }];
}

export function applySessionEvent(
  cache: CacheShape,
  event: SessionEvent,
  sessionId: string,
): CacheShape | null {
  if (event.sessionId !== sessionId) return null;
  const part = partFrom(event);
  if (!part) return null;

  const messages = ensureMessage(cache.messages, event.messageId, event.sessionId);
  return {
    messages: messages.map((message) =>
      message.info.id === event.messageId ? withPart(message, part) : message,
    ),
  };
}

export function useSessionStream(sessionId: string | null): void {
  const queryClient = useQueryClient();
  const queryKey = sessionId ? queryKeys.opencode.messages(sessionId) : null;

  useEffect(() => {
    if (!queryKey || !sessionId) return undefined;
    if (typeof EventSource === 'undefined') return undefined;

    const source = new EventSource(OC_EVENT_STREAM);
    let stopped = false;

    const onEvent = (raw: MessageEvent<string>) => {
      if (stopped) return;
      let event = null;
      try {
        event = toSessionEvent(JSON.parse(raw.data) as Record<string, unknown>);
      } catch {
        event = null;
      }
      if (!event) return;
      queryClient.setQueryData<CacheShape>(queryKey, (previous) => {
        if (!previous) return previous;
        return applySessionEvent(previous, event, sessionId) ?? previous;
      });
    };

    const listener = onEvent as EventListener;
    source.addEventListener('message', listener);

    return () => {
      stopped = true;
      source.removeEventListener('message', listener);
      source.close();
    };
  }, [queryKey, queryClient, sessionId]);
}
