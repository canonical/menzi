import { useQuery } from '@tanstack/react-query';
import { Chip } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { listMessages } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import {
  isCompactionPart,
  isReasoningPart,
  isStepPart,
  isTextPart,
  isToolPart,
} from '../../lib/chat/normalise';
import { useSessionStream } from '../../lib/chat/useSessionStream';
import type { MessageInfo, MessagePart } from '../../lib/types';
import { groupableRuns } from '../../lib/chat/groupRuns';
import { ToolCallGroup } from './chat/ToolCallGroup';
import { ToolCallStep } from './chat/ToolCallStep';
import { CompactionMarker, ThinkingBlock } from './chat/ThinkingBlock';
import { S } from '../../strings/catalogue';

type ChipAppearance = 'caution' | 'information' | 'negative' | 'positive';

function errorText(info: MessageInfo): string {
  if (!info.error) return '';
  if (typeof info.error.message === 'string') return info.error.message;
  if (typeof info.error.data === 'string') return info.error.data;
  if (info.error.data && typeof info.error.data.message === 'string') {
    return info.error.data.message;
  }
  return info.error.name ?? S.chat.unknownError;
}

function finishAppearance(info: MessageInfo): ChipAppearance {
  if (info.error) return 'negative';
  if (info.finish === 'stop') return 'positive';
  if (info.finish === 'error') return 'negative';
  return 'information';
}

function modelLabel(info: MessageInfo): string {
  const model = info.model;
  if (!model) return '';
  const id = model.modelID ?? model.id ?? '';
  if (!id) return '';
  return model.providerID ? `${model.providerID}/${id}` : id;
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
          return (
            <p className="app-chat-turn__body" key={key}>
              {part.text}
            </p>
          );
        }

        if (isReasoningPart(part)) return <ThinkingBlock key={key} text={part.text} />;
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

  const visible = (messagesQuery.data?.messages ?? []).filter(
    (message) => message.info.role === 'user' || message.info.role === 'assistant',
  );

  return (
    <section className="app-section">
      <h2 className="app-panel-heading">{S.chat.title}</h2>
      <DataState
        loading={messagesQuery.isLoading}
        error={messagesQuery.error ? getErrorMessage(messagesQuery.error) : null}
        empty={!messagesQuery.isLoading && !messagesQuery.error && visible.length === 0}
        emptyIcon="comment"
        emptyTitle={S.chat.emptyTitle}
        emptyBody={S.chat.emptyBody}
        onRetry={() => messagesQuery.refetch()}
      >
        <ul className="app-chat-list">
          {visible.map((message) => {
            const info = message.info;
            const isUser = info.role === 'user';
            const label = modelLabel(info);
            const failure = errorText(info);
            return (
              <li
                key={info.id}
                className={`app-chat-turn${isUser ? ' app-chat-turn--user' : ''}`}
                data-testid={`msg-${info.role}`}
              >
                <div className="app-chat-turn__meta">
                  <span className="app-chat-turn__author">
                    {isUser ? S.chat.you : (info.agent ?? S.chat.agent)}
                  </span>
                  {label ? (
                    <Chip
                      className="u-margin--left u-no-margin--bottom"
                      value={label}
                      appearance="information"
                      isReadOnly
                      isDense
                    />
                  ) : null}
                  {info.finish ? (
                    <Chip
                      className="u-margin--left u-no-margin--bottom"
                      value={info.finish}
                      appearance={finishAppearance(info)}
                      isReadOnly
                      isDense
                    />
                  ) : null}
                </div>
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
    </section>
  );
}
