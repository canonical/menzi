import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TranscriptView } from './TranscriptView';
import { S } from '../../strings/catalogue';

const SESSION = '11111111-2222-3333-4444-555555555555';

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;

  constructor() {
    FakeEventSource.instances.push(this);
  }

  close() {}

  open() {
    this.onopen?.();
  }

  emit(payload: unknown) {
    this.onmessage?.({ data: JSON.stringify(payload) } as MessageEvent);
  }
}

function jsonResponse(body: unknown) {
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal('EventSource', FakeEventSource as unknown as typeof EventSource);
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

function event(sequence: number) {
  return {
    message_type: 'event',
    session_id: SESSION,
    payload: { n: sequence },
    sequence,
  };
}

describe('TranscriptView', () => {
  it('shows an empty state when the session has no events', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse([])));
    const { container } = render(<TranscriptView sessionId={SESSION} />);
    expect(await screen.findByText(S.transcript.emptyTitle)).toBeInTheDocument();
    expect(container).toBeTruthy();
  });

  it('reports a live connection once the stream opens', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse([])));
    render(<TranscriptView sessionId={SESSION} />);
    await screen.findByText(S.transcript.emptyTitle);
    await waitFor(() => expect(FakeEventSource.instances).toHaveLength(1));
    await act(async () => {
      FakeEventSource.instances[0].open();
    });
    expect(await screen.findByText(S.transcript.status.live)).toBeInTheDocument();
  });

  it('renders backlog rows and appends live events', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(jsonResponse([event(0), event(1)])),
    );
    const { container } = render(<TranscriptView sessionId={SESSION} />);

    expect(await screen.findByText('0')).toBeInTheDocument();
    expect(screen.getByText('1')).toBeInTheDocument();

    FakeEventSource.instances[0].open();
    FakeEventSource.instances[0].emit(event(2));

    expect(await screen.findByText('2')).toBeInTheDocument();
    expect(container.querySelectorAll('tbody tr')).toHaveLength(3);
  });

  it('surfaces a failed backlog read', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 503,
        statusText: 'Service Unavailable',
        text: async () => 'proxy is down',
        json: async () => ({}),
      }),
    );
    render(<TranscriptView sessionId={SESSION} />);
    expect(await screen.findByText('proxy is down')).toBeInTheDocument();
  });
});
