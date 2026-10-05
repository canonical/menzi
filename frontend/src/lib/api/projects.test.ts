import { afterEach, describe, expect, it, vi } from 'vitest';
import { createProject, getProject, listProjects } from './projects';

const PROJECT = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  name: 'Storefront',
  slug: 'storefront',
  description: 'Customer facing app',
  repository_url: 'git@github.com:acme/storefront.git',
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

describe('projects api', () => {
  it('lists projects without filters', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, [PROJECT]));
    vi.stubGlobal('fetch', fetchMock);
    await expect(listProjects()).resolves.toEqual([PROJECT]);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/projects');
  });

  it('passes the search filter as a query parameter', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, []));
    vi.stubGlobal('fetch', fetchMock);
    await listProjects({ search: 'store' });
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/projects?search=store');
  });

  it('omits empty filters', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, []));
    vi.stubGlobal('fetch', fetchMock);
    await listProjects({ search: '' });
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/projects');
  });

  it('reads a single project', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, PROJECT));
    vi.stubGlobal('fetch', fetchMock);
    await expect(getProject(PROJECT.id)).resolves.toEqual(PROJECT);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/projects/${PROJECT.id}`);
  });

  it('creates a project with snake_case fields', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, PROJECT));
    vi.stubGlobal('fetch', fetchMock);
    await createProject({ name: 'Storefront', slug: 'storefront' });
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/projects');
    expect(options.method).toBe('POST');
    expect(JSON.parse(options.body)).toEqual({
      name: 'Storefront',
      slug: 'storefront',
      description: null,
      repository_url: null,
    });
  });

  it('surfaces a duplicate slug conflict', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        jsonResponse(409, { error: 'a project with this slug already exists' }),
      ),
    );
    await expect(
      createProject({ name: 'Storefront', slug: 'storefront' }),
    ).rejects.toMatchObject({ status: 409 });
  });
});
