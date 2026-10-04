import { afterEach, describe, expect, it, vi } from 'vitest';
import { listAgents, listMessages, listModels } from './opencode';

function json(body: unknown) {
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('capability reads are scoped to a session', () => {
  it('asks the model route for that session', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json({ data: [] }));
    });
    vi.stubGlobal('fetch', fetchMock);

    await listModels('ses_abc');

    expect(String(fetchMock.mock.calls[0][0])).toBe('/api/model?session=ses_abc');
  });

  it('asks the agent route for that session', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json([]));
    });
    vi.stubGlobal('fetch', fetchMock);

    await listAgents('ses_abc');

    expect(String(fetchMock.mock.calls[0][0])).toBe('/agent?session=ses_abc');
  });

  it('escapes a session id', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json({ data: [] }));
    });
    vi.stubGlobal('fetch', fetchMock);

    await listModels('ses_a b&c');

    expect(String(fetchMock.mock.calls[0][0])).toBe('/api/model?session=ses_a%20b%26c');
  });

  it('leaves the route bare when there is no session', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json({ data: [] }));
    });
    vi.stubGlobal('fetch', fetchMock);

    await listModels();

    expect(String(fetchMock.mock.calls[0][0])).toBe('/api/model');
  });

  it('drops a deprecated model', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        void input;
        return Promise.resolve(
          json({ data: [{ id: 'kept', status: 'active' }, { id: 'gone', status: 'deprecated' }] }),
        );
      }),
    );

    const models = await listModels('ses_abc');

    expect(models.map((model) => model.id)).toEqual(['kept']);
  });

  it('maps v2 raw messages with finish and tool parts', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        void input;
        return Promise.resolve(
          json({
            data: [
              { id: 'm2', type: 'assistant', finish: 'tool-calls', time: { created: 20 }, content: [{ type: 'tool', id: 'call_1', name: 'shell', state: { status: 'completed', input: { command: 'pwd' }, content: [{ type: 'text', text: '/workspace' }] } }] },
              { id: 'm1', type: 'user', text: 'go', time: { created: 10 } },
            ],
          }),
        );
      }),
    );

    const response = await listMessages('ses_abc');

    expect(response.messages.map((message) => message.info.role)).toEqual(['user', 'assistant']);
    expect(response.messages[1].info.finish).toBe('tool-calls');
    expect(response.messages[1].parts[0]).toMatchObject({ type: 'tool', tool: 'shell' });
  });

  it('accepts already-normalised info/parts envelopes', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        void input;
        return Promise.resolve(
          json({
            data: [
              {
                info: { id: 'u1', sessionID: 'ses_abc', role: 'user', time: { created: 1 } },
                parts: [{ type: 'text', text: 'hello' }],
              },
              {
                info: { id: 'a1', sessionID: 'ses_abc', role: 'assistant', finish: 'stop', time: { created: 2 } },
                parts: [{ type: 'text', text: 'hi' }],
              },
            ],
          }),
        );
      }),
    );

    const response = await listMessages('ses_abc');

    expect(response.messages.map((message) => message.info.role)).toEqual(['user', 'assistant']);
    expect(response.messages[1].info.finish).toBe('stop');
  });
});
