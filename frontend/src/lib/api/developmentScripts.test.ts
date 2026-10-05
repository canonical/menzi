import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createDevelopmentScript,
  deleteDevelopmentScript,
  listDevelopmentScripts,
  updateDevelopmentScript,
} from './developmentScripts';

const SCRIPT = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  project_id: 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
  name: 'Frontend',
  slug: 'frontend',
  relative_path: '.menzi/scripts/dev/frontend.sh',
  body: 'npm run dev\n',
  source: 'imported',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-02T00:00:00Z',
};

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

describe('development scripts api', () => {
  it('lists scripts', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, [SCRIPT]));
    vi.stubGlobal('fetch', fetchMock);
    await expect(listDevelopmentScripts(SCRIPT.project_id)).resolves.toEqual([SCRIPT]);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/projects/${SCRIPT.project_id}/development-scripts`);
  });

  it('creates a script', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, SCRIPT));
    vi.stubGlobal('fetch', fetchMock);
    await createDevelopmentScript(SCRIPT.project_id, {
      name: 'Frontend',
      slug: 'frontend',
      body: 'npm run dev\n',
    });
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/projects/${SCRIPT.project_id}/development-scripts`);
    expect((options as RequestInit).method).toBe('POST');
  });

  it('updates a script', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, SCRIPT));
    vi.stubGlobal('fetch', fetchMock);
    await updateDevelopmentScript(SCRIPT.project_id, SCRIPT.id, { body: 'npm run dev\n' });
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/projects/${SCRIPT.project_id}/development-scripts/${SCRIPT.id}`);
    expect((options as RequestInit).method).toBe('PUT');
  });

  it('deletes a script', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(204, null));
    vi.stubGlobal('fetch', fetchMock);
    await deleteDevelopmentScript(SCRIPT.project_id, SCRIPT.id);
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/projects/${SCRIPT.project_id}/development-scripts/${SCRIPT.id}`);
    expect((options as RequestInit).method).toBe('DELETE');
  });
});
