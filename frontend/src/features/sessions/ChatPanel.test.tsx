import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ChatPanel } from './ChatPanel';
import { Composer } from './Composer';
import { S } from '../../strings/catalogue';
import type { OpencodeMessage } from '../../lib/types';

const SESSION = 'ses_abc123';

const MESSAGES: OpencodeMessage[] = [
  {
    info: { id: 'm1', sessionID: SESSION, role: 'user' },
    parts: [{ type: 'text', text: 'Add a health check endpoint' }],
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
    parts: [{ type: 'text', text: 'Added GET /health returning ok.' }],
  },
];

const FAILED: OpencodeMessage[] = [
  {
    info: { id: 'm1', sessionID: SESSION, role: 'user' },
    parts: [{ type: 'text', text: 'do a thing' }],
  },
  {
    info: {
      id: 'm2',
      sessionID: SESSION,
      role: 'assistant',
      finish: 'error',
      error: { name: 'ProviderAuthError', data: { message: 'provider budget exceeded' } },
    },
    parts: [],
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

function mockApi(messages: unknown[], onPrompt?: (body: unknown) => void) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      if (url.includes('/message') && method === 'POST') {
        onPrompt?.(JSON.parse(String(init?.body ?? '{}')));
        return Promise.resolve(jsonResponse(200, messages[0]));
      }
      if (url.includes('/message')) {
        return Promise.resolve(jsonResponse(200, messages));
      }
      if (url.includes('/form')) {
        return Promise.resolve(jsonResponse(200, { data: [] }));
      }
      if (url.includes('/api/model')) {
        return Promise.resolve(
          jsonResponse(200, {
            data: [
              {
                id: 'space-bunny-free',
                providerID: 'opencode',
                name: 'Space Bunny Free',
                status: 'active',
              },
            ],
          }),
        );
      }
      if (url.includes('/agent')) {
        return Promise.resolve(jsonResponse(200, [{ name: 'build', description: 'Build' }]));
      }
      return Promise.resolve(jsonResponse(404, { error: 'no route' }));
    }),
  );
}

function renderWithClient(node: React.ReactNode) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <NotificationProvider>
      <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>
    </NotificationProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('ChatPanel', () => {
  it('renders user and assistant turns with the reply text', async () => {
    mockApi(MESSAGES);
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    expect(await screen.findByText('Added GET /health returning ok.')).toBeInTheDocument();
    expect(screen.getByText('Add a health check endpoint')).toBeInTheDocument();
  });

  it('surfaces a provider error from the assistant turn', async () => {
    mockApi(FAILED);
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    expect(await screen.findByTestId('msg-error')).toHaveTextContent('provider budget exceeded');
  });

  it('shows an empty state when there are no messages', async () => {
    mockApi([]);
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    expect(await screen.findByText(S.chat.emptyTitle)).toBeInTheDocument();
  });

  it('offers a jump to the latest chip once the reader scrolls up', async () => {
    mockApi(MESSAGES);
    const user = userEvent.setup();
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    await screen.findByText('Added GET /health returning ok.');

    const scroller = screen.getByTestId('chat-scroll');
    Object.defineProperties(scroller, {
      scrollHeight: { value: 1000, configurable: true },
      clientHeight: { value: 400, configurable: true },
    });
    expect(screen.queryByRole('button', { name: S.chat.jumpToLatest })).not.toBeInTheDocument();

    act(() => {
      scroller.scrollTop = 0;
      fireEvent.scroll(scroller);
    });
    await user.click(screen.getByRole('button', { name: S.chat.jumpToLatest }));

    expect(scroller.scrollTop).toBe(1000);
    expect(screen.queryByRole('button', { name: S.chat.jumpToLatest })).not.toBeInTheDocument();
  });

  it('shows a forms error even when no pending form is loaded', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        if (url.includes('/message')) return Promise.resolve(jsonResponse(200, MESSAGES));
        if (url.includes('/form')) return Promise.resolve(jsonResponse(500, { error: 'forms unavailable' }));
        return Promise.resolve(jsonResponse(404, { error: 'no route' }));
      }),
    );
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    expect(await screen.findByText('forms unavailable')).toBeInTheDocument();
  });

  it('renders a fallback question card from a running question tool call', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        if (url.includes('/message')) {
          return Promise.resolve(
            jsonResponse(200, [
              {
                info: { id: 'm1', sessionID: SESSION, role: 'assistant' },
                parts: [
                  {
                    type: 'tool',
                    id: 'prt_1',
                    tool: 'question',
                    state: {
                      status: 'running',
                      input: {
                        questions: [
                          {
                            question: 'Which fruit would you like to choose?',
                            options: [
                              { label: 'Banana', description: 'Yellow' },
                              { label: 'Apple', description: 'Red' },
                            ],
                          },
                        ],
                      },
                    },
                  },
                ],
              },
            ]),
          );
        }
        if (url.includes('/form')) return Promise.resolve(new Response('not json', { status: 200 }));
        return Promise.resolve(jsonResponse(404, { error: 'no route' }));
      }),
    );
    renderWithClient(<ChatPanel sessionId={SESSION} />);
    expect(await screen.findByText('Banana')).toBeInTheDocument();
    expect(screen.queryByText(/not valid JSON|unexpected character/i)).not.toBeInTheDocument();
  });
});

describe('Composer', () => {
  it('sends the typed prompt and clears the box', async () => {
    const sent: unknown[] = [];
    mockApi(MESSAGES, (body) => sent.push(body));
    const user = userEvent.setup();
    renderWithClient(<Composer sessionId={SESSION} />);

    await user.type(screen.getByRole('textbox'), 'hello agent');
    await user.click(screen.getByRole('button', { name: S.session.composer.send }));

    expect(sent).toHaveLength(1);
    expect(sent[0]).toEqual({ parts: [{ type: 'text', text: 'hello agent' }] });
    expect(screen.getByRole('textbox')).toHaveValue('');
  });

  it('sends on Enter so the single line input is enough', async () => {
    const sent: unknown[] = [];
    mockApi(MESSAGES, (body) => sent.push(body));
    const user = userEvent.setup();
    renderWithClient(<Composer sessionId={SESSION} />);

    await user.type(screen.getByRole('textbox'), 'ship it{Enter}');

    expect(sent).toEqual([{ parts: [{ type: 'text', text: 'ship it' }] }]);
    expect(screen.getByRole('textbox')).toHaveValue('');
  });

  it('shows a spinner and the sending name while the prompt is in flight', async () => {
    // never resolves, so the pending state stays observable
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        if (url.includes('/message') && (init?.method ?? 'GET') === 'POST') {
          return new Promise(() => undefined);
        }
        if (url.includes('/message')) return Promise.resolve(jsonResponse(200, MESSAGES));
        return Promise.resolve(jsonResponse(404, { error: 'no route' }));
      }),
    );
    const user = userEvent.setup();
    renderWithClient(<Composer sessionId={SESSION} />);

    await user.type(screen.getByRole('textbox'), 'long running{Enter}');

    const send = await screen.findByRole('button', { name: S.chat.sending });
    expect(send).toHaveAttribute('aria-disabled', 'true');
    expect(send).not.toHaveTextContent(S.chat.send);
  });

  it('keeps the send button named for assistive technology', async () => {
    mockApi(MESSAGES);
    renderWithClient(<Composer sessionId={SESSION} />);

    // The button shows an icon only, so the name has to come from a label.
    const send = screen.getByRole('button', { name: S.chat.send });
    expect(send).toHaveTextContent('');
    expect(screen.getByLabelText(S.chat.label)).toBe(
      screen.getByRole('textbox', { name: S.chat.label }),
    );
  });
});
