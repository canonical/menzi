import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ChatPanel } from './ChatPanel';
import type { OpencodeMessage } from '../../lib/types';

const SESSION = 'ses_abc';

const ORDERED: OpencodeMessage[] = [
  {
    info: { id: 'm1', sessionID: SESSION, role: 'user' },
    parts: [{ type: 'text', text: 'read the config' }],
  },
  {
    info: {
      id: 'm2',
      sessionID: SESSION,
      role: 'assistant',
      agent: 'build',
      model: { providerID: 'opencode', modelID: 'space-bunny-free' },
      finish: 'stop',
    },
    parts: [
      { type: 'step-start', id: 's1' },
      { type: 'text', text: 'Here is what I found.' },
      {
        type: 'tool',
        id: 'prt_1',
        messageID: 'm2',
        tool: 'read',
        state: {
          status: 'completed',
          input: { filePath: 'src/config.ts' },
          output: 'export const port = 8080',
        },
      },
      { type: 'step-finish', id: 's2' },
      { type: 'reasoning', text: 'the port lives in config' },
      { type: 'text', text: 'The port is 8080.' },
    ],
  },
];

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function mockApi(messages: unknown[]) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/message')) return Promise.resolve(jsonResponse(200, messages));
      return Promise.resolve(jsonResponse(404, { error: 'no route' }));
    }),
  );
}

function renderPanel() {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: 0 } } });
  return render(
    <QueryClientProvider client={queryClient}>
      <ChatPanel sessionId={SESSION} />
    </QueryClientProvider>,
  );
}

function labelTexts(): string[] {
  return Array.from(document.querySelectorAll('.app-tc__label')).map(
    (node) => node.textContent ?? '',
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('ChatPanel parts', () => {
  it('renders parts in the order opencode sends them', async () => {
    mockApi(ORDERED);
    const { container } = renderPanel();

    expect(await screen.findByText('Here is what I found.')).toBeInTheDocument();
    expect(labelTexts()).toEqual(['Reading: src/config.ts']);
    expect(screen.getByText('Thinking')).toBeInTheDocument();
    expect(screen.getByText('The port is 8080.')).toBeInTheDocument();

    const order = Array.from(
      container.querySelectorAll(
        'li[data-testid="msg-assistant"] .app-chat-turn__body, li[data-testid="msg-assistant"] .app-tc, li[data-testid="msg-assistant"] .app-thinking',
      ),
    ).map((node) => node.className.split(' ')[0]);

    expect(order).toEqual([
      'app-chat-turn__body',
      'app-tc',
      'app-thinking',
      'app-chat-turn__body',
    ]);
  });

  it('drops step markers instead of rendering them', async () => {
    mockApi(ORDERED);
    const { container } = renderPanel();
    await screen.findByText('Here is what I found.');
    const assistant = container.querySelector('li[data-testid="msg-assistant"]');
    expect(assistant?.querySelectorAll('.app-chat-turn__body')).toHaveLength(2);
  });

  it('shows the model and finish chips from message info', async () => {
    mockApi(ORDERED);
    renderPanel();
    expect(await screen.findByText('opencode/space-bunny-free')).toBeInTheDocument();
    expect(screen.getByText('stop')).toBeInTheDocument();
  });

  it('keeps reasoning folded rather than inline', async () => {
    mockApi(ORDERED);
    renderPanel();
    const thinking = await screen.findByText('Thinking');
    expect(thinking.closest('details')?.hasAttribute('open')).toBe(false);
  });

  it('ignores system messages', async () => {
    mockApi([{ info: { id: 's', sessionID: SESSION, role: 'system' }, parts: [] }, ...ORDERED]);
    renderPanel();
    await screen.findByText('Here is what I found.');
    expect(document.querySelectorAll('li')).toHaveLength(2);
  });

  it('surfaces a provider error from message info', async () => {
    mockApi([
      { info: { id: 'e1', sessionID: SESSION, role: 'user' }, parts: [{ type: 'text', text: 'go' }] },
      {
        info: {
          id: 'e2',
          sessionID: SESSION,
          role: 'assistant',
          finish: 'error',
          error: { name: 'ProviderAuthError', data: { message: 'budget exceeded' } },
        },
        parts: [],
      },
    ]);
    renderPanel();
    expect(await screen.findByTestId('msg-error')).toHaveTextContent('budget exceeded');
  });

  it('renders two completed tool calls from a tool-calls turn', async () => {
    mockApi([
      {
        info: { id: 't', sessionID: SESSION, role: 'assistant', finish: 'tool-calls' },
        parts: [
          { type: 'text', text: "I'll run both commands." },
          {
            type: 'tool',
            id: 'prt_a',
            tool: 'bash',
            state: {
              status: 'completed',
              input: { command: 'echo menzi-tool-probe' },
              output: 'menzi-tool-probe\n',
            },
          },
          {
            type: 'tool',
            id: 'prt_b',
            tool: 'read',
            state: { status: 'completed', input: { filePath: '/etc/hostname' }, output: 'host' },
          },
        ],
      },
    ]);
    renderPanel();
    expect(await screen.findByText("I'll run both commands.")).toBeInTheDocument();
    expect(labelTexts()).toEqual(['Running: echo menzi-tool-probe', 'Reading: hostname']);
    expect(screen.getByText('tool-calls')).toBeInTheDocument();
  });
});
