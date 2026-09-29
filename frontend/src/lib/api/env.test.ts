import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  execInEnv,
  getEnvLogs,
  getEnvStatus,
  launchEnv,
  relaunchEnv,
} from './env';
import { ApiError } from './errors';

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

function stubFetchOnce(body: unknown, status = 200) {
  const fetchMock = vi.fn().mockResolvedValue(jsonResponse(status, body));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

const STATE = {
  name: 'dev',
  status: 'degraded',
  components: [
    { name: 'db', status: 'running', health: 'healthy' },
    { name: 'api', status: 'unknown', health: 'degraded' },
  ],
  exposures: [{ name: 'api-web', url: 'http://api-web.dev.local', as_type: 'http' }],
};

describe('env api', () => {
  it('reads status with a GET carrying a JSON body', async () => {
    const fetchMock = stubFetchOnce({ success: true, message: 'status retrieved', data: STATE });
    const state = await getEnvStatus(SESSION, 'dev');
    expect(state).toEqual(STATE);
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/env/status');
    expect(options.method).toBe('GET');
    expect(JSON.parse(options.body)).toEqual({
      session_id: SESSION,
      environment_name: 'dev',
    });
  });

  it('surfaces the orchestrator message when a spec is missing', async () => {
    stubFetchOnce(
      { success: false, message: "environment spec 'nope' not found", data: null },
      404,
    );
    await expect(getEnvStatus(SESSION, 'nope')).rejects.toMatchObject({
      status: 404,
      message: "environment spec 'nope' not found",
    });
  });

  it('throws when the orchestrator reports failure without an http error', async () => {
    stubFetchOnce({ success: false, message: 'launch failed', data: null });
    await expect(launchEnv(SESSION, 'dev')).rejects.toBeInstanceOf(ApiError);
  });

  it('returns the launch result', async () => {
    const result = {
      success: true,
      components: [{ name: 'db', success: true, message: 'started' }],
      exposures: [],
    };
    stubFetchOnce({ success: true, message: 'launch completed', data: result });
    await expect(launchEnv(SESSION, 'dev')).resolves.toEqual(result);
  });

  it('returns the relaunch diff', async () => {
    const diff = {
      changed_inputs: ['api'],
      components_to_rebuild: ['api'],
      components_to_restart: ['api'],
      components_to_skip: ['db'],
    };
    stubFetchOnce({ success: true, message: 'relaunch completed', data: diff });
    await expect(relaunchEnv(SESSION, 'dev')).resolves.toEqual(diff);
  });

  it('returns component logs', async () => {
    const logs = { component: 'api', lines: ['listening on 8080'], truncated: false };
    const fetchMock = stubFetchOnce({ success: true, message: 'logs', data: logs });
    await expect(getEnvLogs(SESSION, 'dev', 'api', 50)).resolves.toEqual(logs);
    const [, options] = fetchMock.mock.calls[0];
    expect(JSON.parse(options.body)).toEqual({
      session_id: SESSION,
      environment_name: 'dev',
      component: 'api',
      tail: 50,
    });
  });

  it('returns an exec result', async () => {
    const exec = { exit_code: 0, stdout: 'ok', stderr: '' };
    stubFetchOnce({ success: true, message: 'exec completed', data: exec });
    await expect(execInEnv(SESSION, 'dev', 'api', ['ls', '-la'])).resolves.toEqual(exec);
  });

  it('uses a plain text error body when the backend rejects the payload', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: false,
        status: 422,
        statusText: 'Unprocessable Entity',
        text: async () => 'session_id: UUID parsing failed',
        json: async () => {
          throw new Error('not json');
        },
      }),
    );
    await expect(getEnvStatus('not-a-uuid', 'dev')).rejects.toMatchObject({
      status: 422,
      message: 'session_id: UUID parsing failed',
    });
  });
});
