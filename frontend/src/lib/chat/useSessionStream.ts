import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { eventStreamFor, toSessionEvent, type SessionEvent } from '../api/events';
import { queryKeys } from '../routes';
import { formsKey } from '../api/forms';
import type { MessageInfo, MessagePart, OpencodeMessage, ToolPart, ToolState } from '../types';

interface CacheShape {
  messages: OpencodeMessage[];
}

function partFrom(event: SessionEvent): MessagePart | null {
  const raw = event.part;
  const type = raw.type;

  if (type === 'text' || type === 'reasoning') {
    if (typeof raw.text !== 'string') return null;
    const time =
      type === 'reasoning' && typeof raw.time === 'object' && raw.time !== null
        ? (raw.time as { start?: number; end?: number })
        : undefined;
    return { type, text: raw.text, ...(time ? { time } : {}), __partId: event.partId } as MessagePart;
  }

  if (type === 'tool') {
    const part: ToolPart = {
      type: 'tool',
      id: event.partId,
      sessionID: event.sessionId,
      messageID: event.messageId,
      tool: typeof raw.tool === 'string' ? raw.tool : typeof raw.name === 'string' ? raw.name : 'tool',
    };
    if (typeof raw.callID === 'string') part.callID = raw.callID;
    if (typeof raw.state === 'object' && raw.state !== null) {
      part.state = raw.state as ToolState;
    }
    return { ...part, __partId: event.partId } as MessagePart;
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
  const currentId = (existing as unknown as { __partId?: string }).__partId;
  const nextId = (next as unknown as { __partId?: string }).__partId;
  if (currentId && nextId) {
    return currentId === nextId;
  }
  if (existing.type !== 'tool' || next.type !== 'tool') return false;
  return existing.id === next.id;
}

function withPart(message: OpencodeMessage, part: MessagePart): OpencodeMessage {
  const parts = message.parts ?? [];
  const index = parts.findIndex((existing) => samePart(existing, part));

  if (index === -1) return { ...message, parts: [...parts, part] };

  const next = [...parts];
  const current = next[index];
  if (current.type === 'tool' && part.type === 'tool') {
    const currentState = current.state as Record<string, unknown> | undefined;
    const nextState = part.state as Record<string, unknown> | undefined;
    const currentInput =
      currentState && typeof currentState.input === 'object' && currentState.input !== null
        ? (currentState.input as Record<string, unknown>)
        : {};
    const nextInput =
      nextState && typeof nextState.input === 'object' && nextState.input !== null
        ? (nextState.input as Record<string, unknown>)
        : {};
    next[index] = {
      ...current,
      ...part,
      tool: part.tool || current.tool,
      state:
        part.state && Object.keys(nextInput).length === 0 && Object.keys(currentInput).length > 0
          ? ({ ...nextState, input: currentInput } as ToolState)
          : part.state ?? current.state,
      callID: part.callID ?? current.callID,
    };
  } else if (current.type === 'reasoning' && part.type === 'reasoning') {
    next[index] = {
      ...current,
      ...part,
      time: {
        ...(current.time ?? {}),
        ...(part.time ?? {}),
      },
    };
  } else {
    next[index] = part;
  }
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
  const mergeText = event.mode === 'append' && event.part.type === 'text';
  const mergeReasoning = event.mode === 'append' && event.part.type === 'reasoning';
  return {
    messages: messages.map((message) =>
      message.info.id === event.messageId
        ? (() => {
            if (!mergeText && !mergeReasoning) return withPart(message, part);
            const parts = message.parts ?? [];
            const index = parts.findIndex((existing) =>
              samePart(existing, part),
            );
            if (index === -1) return withPart(message, part);
            const next = [...parts];
            const current = next[index];
            if (mergeText && current.type === 'text' && part.type === 'text') {
              next[index] = { ...current, text: `${current.text}${part.text}` };
            } else if (
              mergeReasoning &&
              current.type === 'reasoning' &&
              part.type === 'reasoning'
            ) {
              next[index] = { ...current, text: `${current.text}${part.text}` };
            }
            return { ...message, parts: next };
          })()
        : message,
    ),
  };
}

export function useSessionStream(sessionId: string | null): void {
  const queryClient = useQueryClient();
  const queryKey = sessionId ? queryKeys.opencode.messages(sessionId) : null;

  useEffect(() => {
    if (!queryKey || !sessionId) return undefined;
    if (typeof EventSource === 'undefined') return undefined;

    const source = new EventSource(eventStreamFor(sessionId));
    let stopped = false;

    const onEvent = (raw: MessageEvent<string>) => {
      if (stopped) return;
      let event = null;
      let payload: Record<string, unknown> | null = null;
      try {
        payload = JSON.parse(raw.data) as Record<string, unknown>;
        const type = typeof payload.type === 'string' ? payload.type : '';
        if (type.startsWith('form.') || type.includes('.form.') || type === 'location.shutdown') {
          queryClient.invalidateQueries({ queryKey: formsKey(sessionId) });
        }
        event = toSessionEvent(payload);
      } catch {
        event = null;
      }
      queryClient.setQueryData<CacheShape>(queryKey, (previous) => {
        if (!previous) return previous;
        let next = event ? applySessionEvent(previous, event, sessionId) ?? previous : previous;
        const type = typeof payload?.type === 'string' ? payload.type : '';
        if (type === 'session.step.ended') {
          const data =
            typeof payload?.data === 'object' && payload.data !== null
              ? (payload.data as Record<string, unknown>)
              : {};
          const streamSession =
            typeof data.sessionID === 'string'
              ? data.sessionID
              : typeof data.sessionId === 'string'
                ? data.sessionId
                : undefined;
          const messageId =
            typeof data.assistantMessageID === 'string'
              ? data.assistantMessageID
              : typeof data.assistantMessageId === 'string'
                ? data.assistantMessageId
                : undefined;
          const finish = typeof data.finish === 'string' ? data.finish : undefined;
          if (streamSession === sessionId && messageId && finish) {
            next = {
              messages: next.messages.map((message) =>
                message.info.id === messageId
                  ? {
                      ...message,
                      info: {
                        ...message.info,
                        finish,
                        time: {
                          ...(message.info.time ?? {}),
                          completed:
                            typeof data.completed === 'number'
                              ? data.completed
                              : typeof data.time === 'object' && data.time !== null && typeof (data.time as Record<string, unknown>).completed === 'number'
                                ? (data.time as Record<string, unknown>).completed as number
                                : message.info.time?.completed,
                        },
                      },
                    }
                  : message,
              ),
            };
          }
        }
        return next;
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
