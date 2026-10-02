import { useLayoutEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { listMessages } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { useStickyScroll } from '../../hooks/useStickyScroll';
import {
  isCompactionPart,
  isReasoningPart,
  isStepPart,
  isTextPart,
  isToolPart,
} from '../../lib/chat/normalise';
import { useSessionStream } from '../../lib/chat/useSessionStream';
import { isWorking } from '../../lib/chat/isWorking';
import type { MessageInfo, MessagePart } from '../../lib/types';
import { groupableRuns } from '../../lib/chat/groupRuns';
import { ToolCallGroup } from './chat/ToolCallGroup';
import { ToolCallStep } from './chat/ToolCallStep';
import { CompactionMarker, ThinkingBlock } from './chat/ThinkingBlock';
import { WorkingLabel } from './WorkingLabel';
import { Markdown } from './Markdown';
import { S } from '../../strings/catalogue';
import { useSessionForms } from '../../lib/chat/useSessionForms';
import { QuestionCard } from './chat/QuestionCard';

function errorText(info: MessageInfo): string {
  if (!info.error) return '';
  if (typeof info.error.message === 'string') return info.error.message;
  if (typeof info.error.data === 'string') return info.error.data;
  if (info.error.data && typeof info.error.data.message === 'string') {
    return info.error.data.message;
  }
  return info.error.name ?? S.chat.unknownError;
}

function MessageParts({ parts }: { parts: MessagePart[] }) {
  const runs = groupableRuns(parts.filter(isToolPart));
  let cursor = 0;

  return (
    <>
      {parts.map((part, index) => {
        const key = isToolPart(part) ? part.id : `${part.type}-${index}`;

        if (isToolPart(part)) {
          const run = runs[cursor];
          cursor += 1;
          if (run.kind === 'group') {
            return <ToolCallGroup key={key} parts={run.parts} />;
          }
          return <ToolCallStep key={key} part={part} />;
        }

        if (isTextPart(part)) {
          if (!part.text.trim()) return null;
          return <Markdown key={key} text={part.text} />;
        }

        if (isReasoningPart(part)) return <ThinkingBlock key={key} part={part} />;
        if (isCompactionPart(part)) return <CompactionMarker key={key} part={part} />;
        if (isStepPart(part)) return null;
        return null;
      })}
    </>
  );
}

export function ChatPanel({ sessionId }: { sessionId: string }) {
  useSessionStream(sessionId);

  const messagesQuery = useQuery({
    queryKey: queryKeys.opencode.messages(sessionId),
    queryFn: () => listMessages(sessionId),
    retry: false,
    refetchInterval: 15000,
  });

  const { ref: scrollRef, stuck, handleScroll, follow, jumpToLatest } = useStickyScroll<HTMLDivElement>();

  const visible = (messagesQuery.data?.messages ?? []).filter(
    (message) => message.info.role === 'user' || message.info.role === 'assistant',
  );

  const working = isWorking(messagesQuery.data?.messages ?? []);
  const { forms, pending, submit, dismiss, error: formsError, refetch: refetchForms } = useSessionForms(sessionId);

  // Streamed parts grow the transcript without changing this component's props,
  // so follow the bottom after every render rather than on a message count.
  useLayoutEffect(() => {
    follow();
  });

  return (
    <section className="app-section app-code__chat-panel" aria-label={S.chat.title}>
      <div
        className="app-chat-scroll"
        ref={scrollRef}
        onScroll={handleScroll}
        tabIndex={-1}
        data-testid="chat-scroll"
      >
        <DataState
          loading={messagesQuery.isLoading}
          error={messagesQuery.error ? getErrorMessage(messagesQuery.error) : null}
          empty={!messagesQuery.isLoading && !messagesQuery.error && visible.length === 0}
          emptyIcon="quote"
          emptyTitle={S.chat.emptyTitle}
          emptyBody={S.chat.emptyBody}
          onRetry={() => messagesQuery.refetch()}
        >
          <ul className="app-chat-list">
            {visible.map((message) => {
              const info = message.info;
              const isUser = info.role === 'user';
              const failure = errorText(info);
              return (
                <li
                  key={info.id}
                  className={`app-chat-turn${isUser ? ' app-chat-turn--user' : ''}`}
                  data-testid={`msg-${info.role}`}
                >
                  {isUser ? (
                    <div className="app-chat-turn__meta">
                      <span className="app-chat-turn__author">{S.chat.you}</span>
                    </div>
                  ) : null}
                  <MessageParts parts={message.parts ?? []} />
                  {failure ? (
                    <p className="p-form-validation__message" data-testid="msg-error">
                      {failure}
                    </p>
                  ) : null}
                </li>
              );
            })}
          </ul>
        </DataState>
          {formsError && pending.length > 0 ? (
            <div role="alert">
              <p>{getErrorMessage(formsError)}</p>
              <Button onClick={() => refetchForms()}>{S.questions.retry}</Button>
            </div>
          ) : null}
          {forms.map((request) => (
            <QuestionCard key={request.id} request={request} onSubmit={(answer) => submit(request, answer)} onDismiss={() => dismiss(request)} />
          ))}
          {pending.length > 0 ? <p role="status" className="app-chat-working">{S.questions.waiting}</p> : working ? (
            <div className="app-chat-working">
              <WorkingLabel />
            </div>
          ) : null}
      </div>
      {stuck ? null : (
        <Button
          className="app-jump-latest"
          dense
          onClick={jumpToLatest}
        >
          {S.chat.jumpToLatest}
        </Button>
      )}
    </section>
  );
}
