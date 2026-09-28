import { afterEach, describe, expect, it, vi } from 'vitest'
import { ApiError, api } from './api'

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
    text: async () => JSON.stringify(body),
  }
}

afterEach(() => {
  localStorage.clear()
})

describe('api request', () => {
  it('attaches bearer token from localStorage', async () => {
    localStorage.setItem('menzi_token', 'token-123')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await api.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.Authorization).toBe('Bearer token-123')
  })

  it('does not send authorization header without a token', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await api.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.Authorization).toBeUndefined()
  })

  it('throws ApiError when response is not ok', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(403, 'forbidden')))
    await expect(api.get('/api/projects')).rejects.toThrow(ApiError)
    await expect(api.get('/api/projects')).rejects.toMatchObject({ status: 403 })
  })

  it('posts JSON bodies with content type', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, { id: 'p1' }))
    vi.stubGlobal('fetch', fetchMock)
    const result = await api.post('/api/projects', { name: 'Demo' })
    expect(result).toEqual({ id: 'p1' })
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/projects')
    expect(options.method).toBe('POST')
    expect(options.headers['Content-Type']).toBe('application/json')
    expect(JSON.parse(options.body)).toEqual({ name: 'Demo' })
  })

  it('sends delete without a body', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(204, null))
    vi.stubGlobal('fetch', fetchMock)
    await api.delete('/api/projects/p1')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.method).toBe('DELETE')
    expect(options.body).toBeUndefined()
  })
})