import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  getEnvironment,
  launchEnvironment,
  relaunchEnvironment,
  teardownEnvironment,
} from './api'

function environmentBody(overrides: Record<string, unknown> = {}) {
  return {
    id: 'env-1',
    workspace_id: 'ws-1',
    status: 'ready',
    spec: { name: 'dev', components: [] },
    ...overrides,
  }
}

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

describe('environments api', () => {
  it('fetches the environment for a workspace', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, environmentBody()))
    vi.stubGlobal('fetch', fetchMock)
    const result = await getEnvironment('ws-1')
    expect(result.id).toBe('env-1')
    const [url] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/workspaces/ws-1/environment')
  })

  it('launches an environment with the given name', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, environmentBody({ status: 'starting' })))
    vi.stubGlobal('fetch', fetchMock)
    const result = await launchEnvironment('ws-1', 'dev')
    expect(result.status).toBe('starting')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/workspaces/ws-1/environment/launch')
    expect(options.method).toBe('POST')
    expect(JSON.parse(options.body)).toEqual({ name: 'dev' })
  })

  it('relaunches an environment with reset flag', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, environmentBody()))
    vi.stubGlobal('fetch', fetchMock)
    await relaunchEnvironment('ws-1', true)
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/workspaces/ws-1/environment/relaunch')
    expect(JSON.parse(options.body)).toEqual({ reset_data: true })
  })

  it('tears down an environment', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, {}))
    vi.stubGlobal('fetch', fetchMock)
    await teardownEnvironment('ws-1')
    const [url, options] = fetchMock.mock.calls[0]
    expect(url).toBe('/api/v1/workspaces/ws-1/environment/teardown')
    expect(options.method).toBe('POST')
  })
})