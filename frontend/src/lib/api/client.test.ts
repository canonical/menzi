import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { MockInstance } from 'vitest'
import { http, UNAUTHORIZED_EVENT } from './client'
import { ApiError } from './errors'

function jsonResponse(status: number, body: unknown, statusText = 'OK') {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText,
    text: async () => JSON.stringify(body),
    json: async () => body,
  }
}

let dispatchEventMock: MockInstance<typeof window.dispatchEvent>

beforeEach(() => {
  dispatchEventMock = vi.spyOn(window, 'dispatchEvent').mockImplementation(() => true)
})

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('api client', () => {
  it('attaches bearer token from localStorage', async () => {
    localStorage.setItem('menzi_token', 'token-123')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await http.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('Authorization')).toBe('Bearer token-123')
  })

  it('does not send authorization header without a token', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await http.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('Authorization')).toBeNull()
  })

  it('throws ApiError with parsed message when response is not ok', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        jsonResponse(403, { error: { message: 'forbidden route', code: 'auth/forbidden' } }, 'Forbidden'),
      ),
    )
    await expect(http.get('/api/demo')).rejects.toBeInstanceOf(ApiError)
    await expect(http.get('/api/demo')).rejects.toMatchObject({ status: 403, message: 'forbidden route', code: 'auth/forbidden' })
  })

  it('reads the flat error string used by the previews and projects services', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(jsonResponse(404, { error: 'project not found' }, 'Not Found')),
    )
    await expect(http.get('/api/demo')).rejects.toMatchObject({
      status: 404,
      message: 'project not found',
    })
  })

  it('clears the token and dispatches the unauthorized event on 401', async () => {
    localStorage.setItem('menzi_token', 'token-123')
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(jsonResponse(401, { error: { message: 'expired' } }, 'Unauthorized')),
    )
    await expect(http.get('/api/demo')).rejects.toBeInstanceOf(ApiError)
    expect(localStorage.getItem('menzi_token')).toBeNull()
    expect(dispatchEventMock).toHaveBeenCalledWith(expect.objectContaining({ type: UNAUTHORIZED_EVENT }))
  })

  it('posts JSON bodies with content type', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, { id: 'p1' }))
    vi.stubGlobal('fetch', fetchMock)
    const result = await http.post('/api/projects', { name: 'Demo' })
    expect(result).toEqual({ id: 'p1' })
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/projects')
    expect(options.method).toBe('POST')
    expect(options.headers.get('Content-Type')).toBe('application/json')
    expect(JSON.parse(options.body)).toEqual({ name: 'Demo' })
  })

  it('sends delete without a body', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(204, null))
    vi.stubGlobal('fetch', fetchMock)
    await http.delete('/api/projects/p1')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.method).toBe('DELETE')
    expect(options.body).toBeUndefined()
  })
})