import { FormEvent, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Form, Icon, Input, Spinner, useNotify } from '@canonical/react-components';
import { listAgents, listModels, sendPrompt } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { S } from '../../strings/catalogue';

interface ComposerProps {
  sessionId: string;
  disabled?: boolean;
}

export function Composer({ sessionId, disabled }: ComposerProps) {
  const [text, setText] = useState('');
  const [modelId] = useState('');
  const [agent] = useState('');
  const queryClient = useQueryClient();
  const notify = useNotify();

  const modelsQuery = useQuery({ queryKey: queryKeys.opencode.models(), queryFn: listModels, retry: false });
  const agentsQuery = useQuery({ queryKey: queryKeys.opencode.agents(), queryFn: listAgents, retry: false });

  const models = modelsQuery.data ?? [];
  const agents = agentsQuery.data ?? [];

  const promptMutation = useMutation({
    mutationFn: () =>
      sendPrompt({
        sessionId,
        text: text.trim(),
        modelId: modelId || undefined,
        agent: agent || undefined,
      }),
    onSuccess: () => {
      setText('');
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.messages(sessionId) });
      // The review list comes from the message summaries and each patch is
      // cached per message, so clear every diff entry for this session.
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.diffs(sessionId) });
    },
    onError: (error) => notify.failure(S.chat.sendFailed, error, getErrorMessage(error)),
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!text.trim() || promptMutation.isPending) return;
    promptMutation.mutate();
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
