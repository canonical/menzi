import { render, screen } from '@testing-library/react';
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
});
