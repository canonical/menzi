import { afterEach, describe, expect, it, vi } from 'vitest';
import { listTranscript, transcriptStreamUrl } from './transcript';

const SESSION = '11111111-2222-3333-4444-555555555555';

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

const EVENTS = [
  {
    message_type: 'event',
    session_id: SESSION,
    payload: { n: 0 },
    sequence: 0,
  },
];

describe('transcript api', () => {
  it('reads the whole transcript without a cursor', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, EVENTS));
    vi.stubGlobal('fetch', fetchMock);
    await expect(listTranscript(SESSION)).resolves.toEqual(EVENTS);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/tunnel/${SESSION}/transcript`);
  });

  it('passes the cursor as a query parameter', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, []));
    vi.stubGlobal('fetch', fetchMock);
    await listTranscript(SESSION, 7);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/tunnel/${SESSION}/transcript?after=7`);
  });

  it('builds a follow url with the cursor', () => {
    expect(transcriptStreamUrl(SESSION, 3)).toBe(
      `/api/tunnel/${SESSION}/transcript?after=3&follow=true`,
    );
  });
});
