import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { TerminalPanel } from './TerminalPanel';
import { S } from '../../strings/catalogue';

class MockSocket {
  static instances: MockSocket[] = [];
  static OPEN = 1;
  static CONNECTING = 0;
  readonly sent: string[] = [];
  readonly url: string;
  readyState = MockSocket.CONNECTING;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockSocket.instances.push(this);
    queueMicrotask(() => {
      this.readyState = MockSocket.OPEN;
      this.onopen?.(new Event('open'));
    });
  }

  send(data: string) {
    this.sent.push(data);
  }

  close() {
    this.readyState = 3;
    this.onclose?.(new Event('close') as unknown as CloseEvent);
  }

  push(text: string) {
    this.onmessage?.({ data: text } as MessageEvent);
  }
}

vi.stubGlobal('WebSocket', MockSocket as unknown as typeof WebSocket);

let currentTerminal: { data: ((text: string) => void) | null } | null = null;

vi.mock('@xterm/xterm', () => {
  class MockTerminal {
    cols = 100;
    rows = 30;
    element: HTMLElement | null = null;
    data: ((text: string) => void) | null = null;

    constructor() {
      currentTerminal = this;
    }

    loadAddon(addon: { activate?: (terminal: MockTerminal) => void }) {
      addon?.activate?.(this);
    }

    open(element: HTMLElement) {
      this.element = element;
    }

    write(data: string) {
      if (!this.element) return;
      this.element.textContent = (this.element.textContent ?? '') + data;
    }

    reset() {
      if (!this.element) return;
      this.element.textContent = '';
    }

    onData(callback: (text: string) => void) {
      this.data = callback;
      return { dispose: () => { this.data = null; } };
    }

    focus() {}

    dispose() {}
  }

  return { Terminal: MockTerminal };
});

vi.mock('@xterm/addon-fit', () => {
  class MockFitAddon {
    fit() {}
  }
  return { FitAddon: MockFitAddon };
});

vi.mock('@xterm/addon-web-links', () => {
  class MockWebLinksAddon {
    activate() {}
  }
  return { WebLinksAddon: MockWebLinksAddon };
});

vi.mock('@xterm/addon-search', () => {
  class MockSearchAddon {
    activate() {}
  }
  return { SearchAddon: MockSearchAddon };
});

function json(body: unknown) {
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function renderPanel() {
  return render(<TerminalPanel userId="u1" projectId="p1" open />);
}

describe('TerminalPanel', () => {
  beforeEach(() => {
    MockSocket.instances = [];
    currentTerminal = null;
  });

  it('opens a pty socket and renders streamed output', async () => {
    vi.stubGlobal('fetch', vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      if (url.includes('/connect-token')) {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: { ticket: 'tok_1' } }));
      }
      if (url.includes('/connect')) {
        return Promise.resolve(json({ endpoint: 'http://workspace.example:17999' }));
      }
      if (url.includes('/api/pty') && method === 'GET') {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: [] }));
      }
      if (url.includes('/api/pty') && method === 'PUT') {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: { id: 'pty_1' } }));
      }
      return Promise.resolve(json({
        location: { directory: '/workspace' },
        data: {
          id: 'pty_1',
          title: 'workspace',
          command: 'bash',
          args: ['-il'],
          cwd: '/workspace',
          status: 'running',
          pid: 10,
          size: { rows: 30, cols: 100 },
          output: { head: 0, tail: 0 },
        },
      }));
    }));

    renderPanel();
    expect(await screen.findByRole('region', { name: S.terminal.title })).toBeInTheDocument();

    await waitFor(() => {
      expect(MockSocket.instances.length).toBeGreaterThan(0);
    });
    MockSocket.instances[0].push('hello\n');
    const output = await screen.findByLabelText(S.terminal.output);
    await waitFor(() => {
      expect(output).toHaveTextContent('hello');
    });
  });

  it('renders terminal output without a separate command form', async () => {
    vi.stubGlobal('fetch', vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      if (url.includes('/connect-token')) {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: { ticket: 'tok_1' } }));
      }
      if (url.includes('/connect')) {
        return Promise.resolve(json({ endpoint: 'http://workspace.example:17999' }));
      }
      if (url.includes('/api/pty') && method === 'GET') {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: [] }));
      }
      if (url.includes('/api/pty') && method === 'PUT') {
        return Promise.resolve(json({ location: { directory: '/workspace' }, data: { id: 'pty_1' } }));
      }
      return Promise.resolve(json({
        location: { directory: '/workspace' },
        data: {
          id: 'pty_1',
          title: 'workspace',
          command: 'bash',
          args: ['-il'],
          cwd: '/workspace',
          status: 'running',
          pid: 10,
          size: { rows: 30, cols: 100 },
          output: { head: 0, tail: 0 },
        },
      }));
    }));

    renderPanel();
    await screen.findByRole('region', { name: S.terminal.title });
    await waitFor(() => {
      expect(MockSocket.instances.length).toBeGreaterThan(0);
    });
    expect(screen.queryByLabelText(S.terminal.inputLabel)).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: S.terminal.run })).not.toBeInTheDocument();
  });
});
