import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { act, render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ChatPanel } from './ChatPanel';

const listMessages = vi.fn();

vi.mock('../../lib/api/opencode', () => ({
  listMessages: () => listMessages(),
}));

vi.mock('../../lib/chat/useSessionStream', () => ({
  useSessionStream: () => {},
}));

function user(text: string) {
  return {
    info: { id: 'msg_u', sessionID: 'ses_1', role: 'user' },
    parts: [{ type: 'text', text }],
  };
}

function assistant(over: Record<string, unknown> = {}) {
  return {
    info: { id: 'msg_a', sessionID: 'ses_1', role: 'assistant', ...over },
    parts: [{ type: 'text', text: 'partial answer' }],
  };
}

function renderPanel(messages: unknown[]) {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  listMessages.mockResolvedValue({ messages });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ChatPanel sessionId="ses_1" />
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  listMessages.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('ChatPanel working label', () => {
  it('shows the label under a turn that has not finished', async () => {
    renderPanel([user('go'), assistant()]);
    expect(await screen.findByTestId('working-label')).toBeInTheDocument();
  });

  it('hides the label once the turn reports a finish', async () => {
    renderPanel([user('go'), assistant({ finish: 'stop' })]);
    expect(await screen.findByTestId('msg-assistant')).toBeInTheDocument();
    expect(screen.queryByTestId('working-label')).not.toBeInTheDocument();
  });

  it('sits below the transcript rather than inside a turn', async () => {
    renderPanel([user('go'), assistant()]);
    const turn = await screen.findByTestId('msg-assistant');
    const label = await screen.findByTestId('working-label');

    expect(turn).not.toContainElement(label);
    expect(label.closest('li')).toBeNull();
    expect(
      (turn as HTMLElement).compareDocumentPosition(label) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it('keeps the same instance when the turn adds another assistant message', async () => {
    const first = assistant({ id: 'msg_a' });
    renderPanel([user('go'), first]);

    const label = await screen.findByTestId('working-label');

    listMessages.mockResolvedValue({
      messages: [user('go'), first, assistant({ id: 'msg_b' })],
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(16_000);
    });

    expect(screen.getAllByTestId('msg-assistant')).toHaveLength(2);
    expect(screen.getByTestId('working-label')).toBe(label);
  });

  it('advances the language once across a multi message turn', async () => {
    const first = assistant({ id: 'msg_a' });
    renderPanel([user('go'), first]);
    expect((await screen.findByTestId('working-label')).textContent).toContain('Working');

    listMessages.mockResolvedValue({
      messages: [user('go'), first, assistant({ id: 'msg_b' })],
    });
    await act(async () => {
      vi.advanceTimersByTime(1_800);
    });

    expect(screen.getByTestId('working-label').textContent).toContain(
      'En train de travailler',
    );
  });

  it('is not shown when the transcript is empty', async () => {
    renderPanel([]);
    expect(await screen.findByTestId('chat-scroll')).toBeInTheDocument();
    expect(screen.queryByTestId('working-label')).not.toBeInTheDocument();
  });

  it('is not shown when only the prompt has been sent', async () => {
    renderPanel([user('go')]);
    expect(await screen.findByTestId('msg-user')).toBeInTheDocument();
    expect(screen.queryByTestId('working-label')).not.toBeInTheDocument();
  });

  it('appears after a tool call that is still running', async () => {
    renderPanel([
      user('go'),
      {
        info: { id: 'msg_a', sessionID: 'ses_1', role: 'assistant' },
        parts: [
          {
            type: 'tool',
            id: 'prt_1',
            tool: 'bash',
            state: { status: 'running', input: {} },
          },
        ],
      },
    ]);
    expect(await screen.findByTestId('working-label')).toBeInTheDocument();
  });

  it('is not shown after a running tool call completed', async () => {
    renderPanel([
      user('go'),
      {
        info: { id: 'msg_a', sessionID: 'ses_1', role: 'assistant', finish: 'stop' },
        parts: [
          {
            type: 'tool',
            id: 'prt_1',
            tool: 'bash',
            state: { status: 'completed', input: {}, output: 'ok' },
          },
        ],
      },
    ]);
    expect(await screen.findByTestId('msg-assistant')).toBeInTheDocument();
    expect(screen.queryByTestId('working-label')).not.toBeInTheDocument();
  });
});