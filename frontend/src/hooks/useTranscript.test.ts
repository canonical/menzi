import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useTranscript } from './useTranscript';

const SESSION = '11111111-2222-3333-4444-555555555555';

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  static lastUrl(): string {
    const instance = FakeEventSource.instances[FakeEventSource.instances.length - 1];
    return instance?.url ?? '';
  }

  url: string;
  onopen: (() => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: (() => void) | null = null;
  closed = false;

  constructor(url: string) {
    this.url = url;
    FakeEventSource.instances.push(this);
  }

  close() {
    this.closed = true;
  }

  open() {
    this.onopen?.();
  }

  emit(payload: unknown) {
    this.onmessage?.({ data: JSON.stringify(payload) } as MessageEvent);
  }

  fail() {
    this.onerror?.();
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

function event(sequence: number) {
  return {
    message_type: 'event',
    session_id: SESSION,
    payload: { n: sequence },
    sequence,
  };
}

beforeEach(() => {
  FakeEventSource.instances = [];
  vi.stubGlobal('EventSource', FakeEventSource as unknown as typeof EventSource);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  localStorage.clear();
});

describe('useTranscript', () => {
  it('stays idle while disabled', async () => {
    vi.stubGlobal('fetch', vi.fn());
    const { result } = renderHook(() => useTranscript(SESSION, { enabled: false }));
    expect(result.current.status).toBe('idle');
    expect(result.current.events).toEqual([]);
    expect(FakeEventSource.instances).toHaveLength(0);
  });

  it('loads the backlog then follows from the last sequence', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(jsonResponse([event(0), event(1)])),
    );
    const { result } = renderHook(() => useTranscript(SESSION, { enabled: true }));

    await waitFor(() => expect(result.current.events).toHaveLength(2));
    await waitFor(() => expect(result.current.status).toBe('loading'));

    act(() => {
      FakeEventSource.instances[0].open();
    });
    expect(result.current.status).toBe('live');
    expect(FakeEventSource.lastUrl()).toContain('after=1');
  });

  it('appends live events and ignores duplicates', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse([event(0)])));
    const { result } = renderHook(() => useTranscript(SESSION, { enabled: true }));
    await waitFor(() => expect(FakeEventSource.instances).toHaveLength(1));
    act(() => {
      FakeEventSource.instances[0].open();
    });

    act(() => {
      FakeEventSource.instances[0].emit(event(1));
      FakeEventSource.instances[0].emit(event(1));
      FakeEventSource.instances[0].emit(event(0));
    });

    expect(result.current.events.map((item) => item.sequence)).toEqual([0, 1]);
    expect(result.current.lastSequence).toBe(1);
  });

  it('reconnects from the cursor after a dropped stream', async () => {
    vi.useFakeTimers();
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse([event(0)])));
    const { result } = renderHook(() => useTranscript(SESSION, { enabled: true }));
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(FakeEventSource.instances).toHaveLength(1);
    act(() => {
      FakeEventSource.instances[0].open();
      FakeEventSource.instances[0].emit(event(5));
    });

    act(() => {
      FakeEventSource.instances[0].fail();
    });
    expect(result.current.status).toBe('reconnecting');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });

    expect(FakeEventSource.instances).toHaveLength(2);
    expect(FakeEventSource.lastUrl()).toContain('after=5');
  });

  it('reports a failure when the backlog cannot be read', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
        statusText: 'Internal Server Error',
        text: async () => 'boom',
        json: async () => ({}),
      }),
    );
    const { result } = renderHook(() => useTranscript(SESSION, { enabled: true }));
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.error).toBe('boom');
    expect(FakeEventSource.instances).toHaveLength(0);
  });
});
