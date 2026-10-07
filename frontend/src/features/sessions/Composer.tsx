import { FormEvent, useEffect, useMemo, useRef, useState } from 'react';
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
  const [modelKey, setModelKey] = useState('');
  const [modelOpen, setModelOpen] = useState(false);
  const [modelSearch, setModelSearch] = useState('');
  const [agent] = useState('');
  const modelSelectorRef = useRef<HTMLDivElement | null>(null);
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

  const modelOptions = useMemo(() => {
    return models.map((model) => {
      const provider = model.providerID?.trim() ?? '';
      const id = model.id;
      const key = provider ? `${provider}/${id}` : id;
      const title = model.name?.trim() || id;
      const description = provider ? `${provider} · ${id}` : id;
      return {
        key,
        modelId: id,
        providerId: provider || undefined,
        title,
        description,
      };
    });
  }, [models]);

  const selectedModel = modelOptions.find((model) => model.key === modelKey) ?? null;
  const effectiveModel = selectedModel ?? (modelOptions.length === 1 ? modelOptions[0] : null);

  const filteredModels = useMemo(() => {
    const term = modelSearch.trim().toLowerCase();
    if (!term) return modelOptions;
    return modelOptions.filter((model) =>
      `${model.title} ${model.description}`.toLowerCase().includes(term),
    );
  }, [modelOptions, modelSearch]);

  useEffect(() => {
    if (!modelOpen) return;
    const close = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || modelSelectorRef.current?.contains(target)) return;
      setModelOpen(false);
    };
    document.addEventListener('mousedown', close);
    return () => document.removeEventListener('mousedown', close);
  }, [modelOpen]);

  const promptMutation = useMutation({
    mutationKey: ['opencode', 'prompt', sessionId],
    mutationFn: (message: string) =>
      sendPrompt({
        sessionId,
        text: message,
        modelId: effectiveModel?.modelId,
        modelProvider: effectiveModel?.providerId,
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

  const modelValue = effectiveModel?.key ?? '';

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
        {modelOptions.length > 0 ? (
          <div className="app-composer__model-selector" ref={modelSelectorRef}>
            <Button
              appearance="base"
              type="button"
              className="app-composer__model-toggle"
              aria-label={S.chat.model}
              aria-expanded={modelOpen}
              aria-haspopup="listbox"
              disabled={disabled || promptMutation.isPending}
              onClick={() => {
                setModelOpen((value) => !value);
                setModelSearch('');
              }}
            >
              <span className="app-composer__model-value">{effectiveModel?.title ?? S.chat.modelAuto}</span>
              <Icon name="chevron" />
            </Button>
            {modelOpen ? (
              <div className="app-composer__model-dropdown">
                <Input
                  type="search"
                  className="app-composer__model-search"
                  label={S.chat.modelSearch}
                  labelClassName="u-off-screen"
                  placeholder={S.chat.modelSearch}
                  autoFocus
                  value={modelSearch}
                  onChange={(event) => setModelSearch(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Escape') setModelOpen(false);
                  }}
                />
                <div className="app-composer__model-list" role="listbox" aria-label={S.chat.model}>
                  <Button
                    appearance="base"
                    type="button"
                    className={`app-composer__model-option${modelKey === '' ? ' is-selected' : ''}`}
                    role="option"
                    aria-selected={modelKey === ''}
                    onClick={() => {
                      setModelKey('');
                      setModelOpen(false);
                    }}
                  >
                    {S.chat.modelAuto}
                  </Button>
                  {filteredModels.map((model) => (
                    <Button
                      appearance="base"
                      key={model.key}
                      type="button"
                      className={`app-composer__model-option${model.key === modelKey ? ' is-selected' : ''}`}
                      role="option"
                      aria-selected={model.key === modelKey}
                      onClick={() => {
                        setModelKey(model.key);
                        setModelOpen(false);
                      }}
                    >
                      <span className="app-composer__model-option-title">{model.title}</span>
                      <span className="app-composer__model-option-meta">{model.description}</span>
                    </Button>
                  ))}
                  {filteredModels.length === 0 ? (
                    <p className="app-composer__model-empty">{S.chat.modelEmpty}</p>
                  ) : null}
                </div>
              </div>
            ) : null}
          </div>
        ) : null}
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
