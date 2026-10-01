import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Composer } from './Composer';

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
  return render(
    <QueryClientProvider client={client}>
      <Composer sessionId="ses_1" />
    </QueryClientProvider>,
  );
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

    await userEvent.click(screen.getByRole('button'));

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
    await userEvent.click(screen.getByRole('button'));

    await waitFor(() => expect(input).toHaveValue(''));
    expect(sendPrompt).toHaveBeenCalledTimes(1);
    release({ info: { id: 'msg_1' }, parts: [] });
  });

  it('puts the message back when the send failed', async () => {
    sendPrompt.mockRejectedValue(new Error('proxy refused'));
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, 'keep me');
    await userEvent.click(screen.getByRole('button'));

    await waitFor(() => expect(sendPrompt).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(input).toHaveValue('keep me'));
  });

  it('sends the trimmed text', async () => {
    sendPrompt.mockResolvedValue({ info: { id: 'msg_1' }, parts: [] });
    renderComposer();
    const input = screen.getByLabelText('Message');

    await userEvent.type(input, '  hello  ');
    await userEvent.click(screen.getByRole('button'));

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
    await userEvent.click(screen.getByRole('button'));

    await waitFor(() => expect(input).toHaveValue('spaced'));
  });

  it('asks for the models and agents of the session it is in', async () => {
    renderComposer();

    await waitFor(() => expect(listModels).toHaveBeenCalledWith('ses_1'));
    expect(listAgents).toHaveBeenCalledWith('ses_1');
  });

  it('does not send an empty message', async () => {
    renderComposer();
    await userEvent.click(screen.getByRole('button'));
    expect(sendPrompt).not.toHaveBeenCalled();
  });
});