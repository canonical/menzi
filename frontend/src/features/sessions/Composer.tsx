import { FormEvent, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Form, Icon, Input, Spinner, useNotify } from '@canonical/react-components';
import { listAgents, listModels, sendPrompt } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import type { OpencodeMessage } from '../../lib/types';
import { S } from '../../strings/catalogue';

interface ComposerProps {
  sessionId: string;
  disabled?: boolean;
}

interface MessagesCache {
  messages: OpencodeMessage[];
}

export function Composer({ sessionId, disabled }: ComposerProps) {
  const [text, setText] = useState('');
  const [modelId] = useState('');
  const [agent] = useState('');
  const queryClient = useQueryClient();
  const notify = useNotify();

  const modelsQuery = useQuery({
    queryKey: queryKeys.opencode.models(sessionId),
    queryFn: () => listModels(sessionId),
    retry: false,
  });
  const agentsQuery = useQuery({
    queryKey: queryKeys.opencode.agents(sessionId),
    queryFn: () => listAgents(sessionId),
    retry: false,
  });

  const models = modelsQuery.data ?? [];
  const agents = agentsQuery.data ?? [];

  const promptMutation = useMutation({
    mutationKey: ['opencode', 'prompt', sessionId],
    mutationFn: (message: string) =>
      sendPrompt({
        sessionId,
        text: message,
        modelId: modelId || undefined,
        agent: agent || undefined,
      }),
    onMutate: async (message) => {
      const queryKey = queryKeys.opencode.messages(sessionId);
      await queryClient.cancelQueries({ queryKey });
      const optimisticID = `local-user-${Date.now()}-${Math.random().toString(36).slice(2)}`;
      const optimistic: OpencodeMessage = {
        info: {
          id: optimisticID,
          sessionID: sessionId,
          role: 'user',
          time: { created: Date.now() },
        },
        parts: [{ type: 'text', text: message }],
      };
      queryClient.setQueryData<MessagesCache>(queryKey, (previous) => ({
        messages: [...(previous?.messages ?? []), optimistic],
      }));
      return { optimisticID };
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.messages(sessionId) });
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.diffs(sessionId) });
    },
    onError: (error, message, context) => {
      if (context?.optimisticID) {
        queryClient.setQueryData<MessagesCache>(queryKeys.opencode.messages(sessionId), (previous) => {
          if (!previous) return previous;
          return {
            messages: previous.messages.filter((entry) => entry.info.id !== context.optimisticID),
          };
        });
      }
      setText(message);
      notify.failure(S.chat.sendFailed, error, getErrorMessage(error));
    },
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const message = text.trim();
    if (!message || promptMutation.isPending) return;
    setText('');
    promptMutation.mutate(message);
  };

  const modelValue =
    modelId || (models.length === 1 ? `${models[0].providerID ?? ''}/${models[0].id}` : '');

  return (
    <Form className="app-composer" onSubmit={handleSubmit}>
      {models.length > 0 ? (
        <span className="u-off-screen" data-testid="chat-model">
          {modelValue}
        </span>
      ) : null}
      {agents.length > 0 ? (
        <span className="u-off-screen" data-testid="chat-agent">
          {agent || agents[0].id}
        </span>
      ) : null}
      <div className="app-composer__row">
        <Input
          id="chat-composer"
          className="app-composer__input"
          wrapperClassName="app-composer__field"
          type="text"
          label={S.chat.label}
          labelClassName="u-off-screen"
          placeholder={S.chat.placeholder}
          disabled={disabled}
          value={text}
          onChange={(event) => setText(event.target.value)}
        />
        <Button
          appearance="positive"
          aria-label={promptMutation.isPending ? S.chat.sending : S.chat.send}
          className="app-composer__send"
          type="submit"
          disabled={disabled || !text.trim() || promptMutation.isPending}
        >
          {promptMutation.isPending ? <Spinner /> : <Icon name="arrow-right" />}
        </Button>
      </div>
    </Form>
  );
}
