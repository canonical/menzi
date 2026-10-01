import { afterEach, describe, expect, it, vi } from 'vitest';
import { listAgents, listModels } from './opencode';

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
});