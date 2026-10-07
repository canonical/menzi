import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Composer } from './Composer';
import { queryKeys } from '../../lib/routes';

const sendPrompt = vi.fn();
const listModels = vi.fn().mockResolvedValue([]);
const listAgents = vi.fn().mockResolvedValue([]);

vi.mock('../../lib/api/opencode', () => ({
  sendPrompt: (...args: unknown[]) => sendPrompt(...args),
  listModels: (sessionId?: string) => listModels(sessionId),
  listAgents: (sessionId?: string) => listAgents(sessionId),
}));

function renderComposer() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = render(
    <QueryClientProvider client={client}>
      <Composer sessionId="ses_1" />
    </QueryClientProvider>,
  );
  return { client, ...view };
}

beforeEach(() => {
  sendPrompt.mockReset();
  listModels.mockReset();
  listAgents.mockReset();
  listModels.mockResolvedValue([]);
  listAgents.mockResolvedValue([]);
});

describe('Composer', () => {
  it('clears the input as soon as the message is sent', async () => {
    sendPrompt.mockResolvedValue({ info: { id: 'msg_1' }, parts: [] });
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, 'do the thing');
    expect(input).toHaveValue('do the thing');

    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(sendPrompt).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(input).toHaveValue(''));
  });

  it('clears the input before the turn finishes', async () => {
    let release: (value: unknown) => void = () => {};
    sendPrompt.mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, 'long running turn');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(input).toHaveValue(''));
    expect(sendPrompt).toHaveBeenCalledTimes(1);
    release({ info: { id: 'msg_1' }, parts: [] });
  });

  it('shows an optimistic user message immediately', async () => {
    let release: (value: unknown) => void = () => {};
    sendPrompt.mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );
    const { client } = renderComposer();

    await userEvent.type(screen.getByLabelText('Message'), 'show now');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => {
      const cached = client.getQueryData<{ messages: Array<{ info: { role: string }; parts: Array<{ type: string; text?: string }> }> }>(
        queryKeys.opencode.messages('ses_1'),
      );
      expect(cached?.messages.some((message) =>
        message.info.role === 'user' &&
        message.parts.some((part) => part.type === 'text' && part.text === 'show now')),
      ).toBe(true);
    });

    release({ info: { id: 'msg_1' }, parts: [] });
  });

  it('removes the optimistic user message when send fails', async () => {
    sendPrompt.mockRejectedValue(new Error('proxy refused'));
    const { client } = renderComposer();

    await userEvent.type(screen.getByLabelText('Message'), 'transient');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(sendPrompt).toHaveBeenCalledTimes(1));
    await waitFor(() => {
      const cached = client.getQueryData<{ messages: Array<{ info: { role: string }; parts: Array<{ type: string; text?: string }> }> }>(
        queryKeys.opencode.messages('ses_1'),
      );
      expect(cached?.messages.some((message) =>
        message.info.role === 'user' &&
        message.parts.some((part) => part.type === 'text' && part.text === 'transient')),
      ).toBe(false);
    });
  });

  it('puts the message back when the send failed', async () => {
    sendPrompt.mockRejectedValue(new Error('proxy refused'));
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, 'keep me');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(sendPrompt).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(input).toHaveValue('keep me'));
  });

  it('sends the trimmed text', async () => {
    sendPrompt.mockResolvedValue({ info: { id: 'msg_1' }, parts: [] });
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, '  hello  ');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() =>
      expect(sendPrompt).toHaveBeenCalledWith(
        expect.objectContaining({ sessionId: 'ses_1', text: 'hello' }),
      ),
    );
  });

  it('restores the trimmed message when the send failed', async () => {
    sendPrompt.mockRejectedValue(new Error('proxy refused'));
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, '  spaced  ');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() => expect(input).toHaveValue('spaced'));
  });

  it('sends the selected model and provider', async () => {
    sendPrompt.mockResolvedValue({ info: { id: 'msg_1' }, parts: [] });
    listModels.mockResolvedValue([
      { id: 'sonnet-4', providerID: 'anthropic', name: 'Claude Sonnet 4' },
      { id: 'gpt-5.3', providerID: 'openai', name: 'GPT-5.3' },
    ]);
    renderComposer();

    await userEvent.click(await screen.findByRole('button', { name: 'Model' }));
    await userEvent.click(await screen.findByRole('option', { name: /GPT-5.3/i }));
    await userEvent.type(screen.getByLabelText('Message'), 'choose model');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() =>
      expect(sendPrompt).toHaveBeenCalledWith(
        expect.objectContaining({
          sessionId: 'ses_1',
          text: 'choose model',
          modelId: 'gpt-5.3',
          modelProvider: 'openai',
        }),
      ),
    );
  });

  it('uses the only available model automatically', async () => {
    sendPrompt.mockResolvedValue({ info: { id: 'msg_1' }, parts: [] });
    listModels.mockResolvedValue([{ id: 'space-bunny-free', providerID: 'opencode' }]);
    renderComposer();

    await userEvent.type(screen.getByLabelText('Message'), 'auto model');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    await waitFor(() =>
      expect(sendPrompt).toHaveBeenCalledWith(
        expect.objectContaining({
          sessionId: 'ses_1',
          text: 'auto model',
          modelId: 'space-bunny-free',
          modelProvider: 'opencode',
        }),
      ),
    );
  });

  it('places the model selector between message input and send button', async () => {
    listModels.mockResolvedValue([{ id: 'space-bunny-free', providerID: 'opencode' }]);
    renderComposer();

    const row = document.querySelector('.app-composer__row');
    const input = await screen.findByLabelText('Message');
    const model = await screen.findByRole('button', { name: 'Model' });
    const send = screen.getByRole('button', { name: 'Send' });

    expect(row).not.toBeNull();
    expect(row).toContainElement(input);
    expect(row).toContainElement(model);
    expect(row).toContainElement(send);
    expect(input.compareDocumentPosition(model) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(model.compareDocumentPosition(send) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it('filters available models with search', async () => {
    listModels.mockResolvedValue([
      { id: 'sonnet-4', providerID: 'anthropic', name: 'Claude Sonnet 4' },
      { id: 'gpt-5.3', providerID: 'openai', name: 'GPT-5.3' },
    ]);
    renderComposer();

    await userEvent.click(await screen.findByRole('button', { name: 'Model' }));
    await userEvent.type(screen.getByLabelText('Search models'), 'claude');

    expect(screen.getByRole('option', { name: /Claude Sonnet 4/i })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: /GPT-5.3/i })).toBeNull();
  });

  it('asks for the models and agents of the session it is in', async () => {
    renderComposer();

    await waitFor(() => expect(listModels).toHaveBeenCalledWith('ses_1'));
    expect(listAgents).toHaveBeenCalledWith('ses_1');
  });

  it('does not send an empty message', async () => {
    renderComposer();
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(sendPrompt).not.toHaveBeenCalled();
  });
});
