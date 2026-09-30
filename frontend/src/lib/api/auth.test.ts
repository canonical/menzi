import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import * as auth from './auth'

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  }
}

let fetchMock: ReturnType<typeof vi.fn>

beforeEach(() => {
  fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  vi.restoreAllMocks()
})

function lastCall(): [string, RequestInit] {
  return fetchMock.mock.calls[fetchMock.mock.calls.length - 1] as [string, RequestInit]
}

describe('auth client', () => {
  it('reads the enabled providers', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse(200, { registration: 'open', password: true, oidc: [{ id: 'google', label: 'Google' }] }),
    )
    const providers = await auth.getProviders()
    expect(providers.registration).toBe('open')
    expect(providers.oidc[0].id).toBe('google')
    expect(lastCall()[0]).toBe('/api/v1/auth/providers')
  })

  it('reads the current session', async () => {
    fetchMock.mockResolvedValue(
      jsonResponse(200, { user: { id: 'u1', kind: 'user', email: 'a@b' }, csrf_token: 'c' }),
    )
    const session = await auth.getSession()
    expect(session.user.id).toBe('u1')
    expect(lastCall()[0]).toBe('/api/v1/auth/session')
  })

  it('posts the email and password to sign in', async () => {
    await auth.login('a@b', 'secret')
    const [url, options] = lastCall()
    expect(url).toBe('/api/v1/auth/login')
    expect(options.method).toBe('POST')
    expect(JSON.parse(options.body as string)).toEqual({ email: 'a@b', password: 'secret' })
  })

  it('posts the email, name and password to register', async () => {
    await auth.register('a@b', 'A Person', 'secret')
    const [url, options] = lastCall()
    expect(url).toBe('/api/v1/auth/register')
    expect(JSON.parse(options.body as string)).toEqual({
      email: 'a@b',
      name: 'A Person',
      password: 'secret',
    })
  })

  it('signs out through the logout route', async () => {
    await auth.logout()
    expect(lastCall()[0]).toBe('/api/v1/auth/logout')
  })

  it('asks for a reset link', async () => {
    await auth.requestPasswordReset('a@b')
    const [url, options] = lastCall()
    expect(url).toBe('/api/v1/auth/password/forgot')
    expect(JSON.parse(options.body as string)).toEqual({ email: 'a@b' })
  })

  it('sends both passwords when changing one', async () => {
    await auth.changePassword('old-one', 'new-one')
    const [url, options] = lastCall()
    expect(url).toBe('/api/v1/auth/password/change')
    expect(JSON.parse(options.body as string)).toEqual({
      current_password: 'old-one',
      new_password: 'new-one',
    })
  })

  it('unwraps the device list', async () => {
    fetchMock.mockResolvedValue(jsonResponse(200, { devices: [{ id: 'd1' }] }))
    const devices = await auth.listDevices()
    expect(devices).toHaveLength(1)
    expect(devices[0].id).toBe('d1')
  })

  it('revokes one device', async () => {
    await auth.revokeDevice('d1')
    const [url, options] = lastCall()
    expect(url).toBe('/api/v1/auth/sessions/d1')
    expect(options.method).toBe('DELETE')
  })

  it('builds the provider start url', () => {
    expect(auth.oidcStartUrl('google')).toBe('/api/v1/auth/oidc/google/start')
  })

  it('encodes the return path on the provider start url', () => {
    expect(auth.oidcStartUrl('google', '/projects/a/code')).toBe(
      '/api/v1/auth/oidc/google/start?redirect_to=%2Fprojects%2Fa%2Fcode',
    )
  })
})
