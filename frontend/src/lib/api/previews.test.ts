import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createPreview,
  getPreview,
  listPreviews,
  resetPreview,
  restartPreview,
  teardownPreview,
} from './previews'

function previewBody(id: string, overrides: Record<string, unknown> = {}) {
  return {
    id,
    project_id: 'proj-1',
    status: 'ready',
    mode: 'pinned',
    url: 'http://prv-1.preview.dev.local',
    created_at: '2026-01-01T00:00:00Z',
    ...overrides,
  }
}

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    json: async () => body,
  }
}

afterEach(() => {
  localStorage.clear()
})

describe('previews api', () => {
  it('lists previews for a project', async () => {
    const previews = [previewBody('prv-1'), previewBody('prv-2')]
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, previews))
    vi.stubGlobal('fetch', fetchMock)
    const result = await listPreviews('proj-1')
    expect(result).toHaveLength(2)
    const [url] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/projects/proj-1/previews')
  })

  it('creates a preview with commit sha, branch and mode', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, previewBody('prv-3')))
    vi.stubGlobal('fetch', fetchMock)
    const result = await createPreview({
      projectId: 'proj-1',
      commitSha: 'abc123',
      branch: 'feature/x',
      mode: 'pinned',
    })
    expect(result.id).toBe('prv-3')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/previews')
    expect(JSON.parse(options.body)).toEqual({
      project_id: 'proj-1',
      commit_sha: 'abc123',
      branch: 'feature/x',
      mode: 'pinned',
    })
  })

  it('creates a preview with the default mode', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, previewBody('prv-4')))
    vi.stubGlobal('fetch', fetchMock)
    await createPreview({ projectId: 'proj-1' })
    const [, options] = fetchMock.mock.calls[0]
    const body = JSON.parse(options.body)
    expect(body.commit_sha).toBeUndefined()
    expect(body.branch).toBeUndefined()
    expect(body.mode).toBe('pinned')
  })

  it('fetches a single preview', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, previewBody('prv-1')))
    vi.stubGlobal('fetch', fetchMock)
    const result = await getPreview('prv-1')
    expect(result.id).toBe('prv-1')
    const [url] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/previews/prv-1')
  })

  it('resets a preview', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    await resetPreview('prv-1')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/previews/prv-1/reset')
    expect(options.method).toBe('POST')
  })

  it('restarts a preview', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    await restartPreview('prv-1')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/previews/prv-1/restart')
    expect(options.method).toBe('POST')
  })

  it('tears down a preview', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    await teardownPreview('prv-1')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/previews/prv-1')
    expect(options.method).toBe('DELETE')
  })
})