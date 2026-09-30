import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { MockInstance } from 'vitest'
import { http, readCookie, UNAUTHORIZED_EVENT } from './client'
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

function setCookie(name: string, value: string) {
  document.cookie = `${name}=${value}; path=/`
}

beforeEach(() => {
  dispatchEventMock = vi.spyOn(window, 'dispatchEvent').mockImplementation(() => true)
  for (const part of document.cookie.split(';')) {
    const key = part.split('=')[0]?.trim()
    if (key) document.cookie = `${key}=; path=/; Max-Age=0`
  }
})

afterEach(() => {
  vi.restoreAllMocks()
  localStorage.clear()
})

describe('api client', () => {
  it('sends cookies on every request', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await http.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.credentials).toBe('include')
  })

  it('never sends an authorization header', async () => {
    localStorage.setItem('menzi_token', 'token-123')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await http.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('Authorization')).toBeNull()
  })

  it('attaches the csrf header to a post', async () => {
    setCookie('menzi_csrf', 'csrf-value')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, { id: 'p1' }))
    vi.stubGlobal('fetch', fetchMock)
    await http.post('/api/projects', { name: 'Demo' })
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('x-menzi-csrf')).toBe('csrf-value')
  })

  it('attaches the csrf header to a delete', async () => {
    setCookie('menzi_csrf', 'csrf-value')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(204, null))
    vi.stubGlobal('fetch', fetchMock)
    await http.delete('/api/projects/p1')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('x-menzi-csrf')).toBe('csrf-value')
  })

  it('does not attach the csrf header to a get', async () => {
    setCookie('menzi_csrf', 'csrf-value')
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, { ok: true }))
    vi.stubGlobal('fetch', fetchMock)
    await http.get('/api/projects')
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('x-menzi-csrf')).toBeNull()
  })

  it('omits the csrf header when no cookie is present', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, { id: 'p1' }))
    vi.stubGlobal('fetch', fetchMock)
    await http.post('/api/projects', { name: 'Demo' })
    const [, options] = fetchMock.mock.calls[0]
    expect(options.headers.get('x-menzi-csrf')).toBeNull()
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

  it('dispatches the unauthorized event on 401 without touching storage', async () => {
    localStorage.setItem('menzi_token', 'token-123')
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(jsonResponse(401, { error: { message: 'expired' } }, 'Unauthorized')),
    )
    await expect(http.get('/api/demo')).rejects.toBeInstanceOf(ApiError)
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

describe('readCookie', () => {
  it('reads a value out of the document cookie', () => {
    setCookie('menzi_csrf', 'abc')
    expect(readCookie('menzi_csrf')).toBe('abc')
  })

  it('returns null for a cookie that is not set', () => {
    expect(readCookie('not-set')).toBeNull()
  })
})
