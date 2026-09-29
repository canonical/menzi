import { FormEvent, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Form, Icon, Spinner, Textarea, useNotify } from '@canonical/react-components';
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
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.diff(sessionId) });
      queryClient.invalidateQueries({ queryKey: queryKeys.opencode.vcs() });
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
    <Form className="u-margin--bottom" onSubmit={handleSubmit}>
      <Textarea
        id="chat-composer"
        label={S.chat.label}
        placeholder={S.chat.placeholder}
        rows={3}
        disabled={disabled}
        value={text}
        onChange={(event) => setText(event.target.value)}
      />
      <div className="u-flex u-align--center u-justify--end">
        {models.length > 0 ? (
          <span className="u-off-left--small" data-testid="chat-model">
            {modelValue}
          </span>
        ) : null}
        {agents.length > 0 ? (
          <span className="u-off-left--small" data-testid="chat-agent">
            {agent || agents[0].id}
          </span>
        ) : null}
        <Button
          appearance="positive"
          type="submit"
          disabled={disabled || !text.trim() || promptMutation.isPending}
        >
          {promptMutation.isPending ? (
            <Spinner text={S.chat.sending} />
          ) : (
            <>
              <Icon name="send" />
              {S.chat.send}
            </>
          )}
        </Button>
      </div>
    </Form>
  );
}
