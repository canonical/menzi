import { describe, expect, it } from 'vitest';
import { phaseFor, sessionGate } from './useWorkspace';
import { WorkspaceNotFoundError } from '../../lib/api/workspaces';
import type { Workspace } from '../../lib/types';

function workspace(status: Workspace['status'], lastError?: string): Workspace {
  return {
    id: 'w1',
    user_id: 'u1',
    project_id: 'p1',
    name: 'wsp-a-b',
    status,
    last_error: lastError ?? null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

function apiError(status: number): Error {
  return Object.assign(new Error('request failed'), { status });
}

describe('workspace phase', () => {
  it('is loading until the first answer arrives', () => {
    expect(phaseFor(undefined, undefined, true)).toBe('loading');
  });

  it('is absent when the service says there is none', () => {
    expect(phaseFor(undefined, new WorkspaceNotFoundError(), false)).toBe('absent');
    expect(phaseFor(undefined, apiError(404), false)).toBe('absent');
  });

  it('is setting up while a workspace is being built', () => {
    expect(phaseFor(workspace('provisioning'), undefined, false)).toBe('setting-up');
    expect(phaseFor(workspace('requested'), undefined, false)).toBe('setting-up');
  });

  it('is failed when a requested workspace carries a reason', () => {
    expect(phaseFor(workspace('requested', 'copy failed'), undefined, false)).toBe('failed');
  });

  it('is ready for a workspace that accepts work', () => {
    expect(phaseFor(workspace('ready'), undefined, false)).toBe('ready');
    expect(phaseFor(workspace('running'), undefined, false)).toBe('ready');
  });

  it('is idle for a paused workspace', () => {
    expect(phaseFor(workspace('idle'), undefined, false)).toBe('idle');
  });

  it('is absent for a workspace that was torn down', () => {
    expect(phaseFor(workspace('deleted'), undefined, false)).toBe('absent');
    expect(phaseFor(workspace('archived'), undefined, false)).toBe('absent');
  });

  it('is not-permitted when the caller is not a member', () => {
    expect(phaseFor(undefined, apiError(403), false)).toBe('not-permitted');
  });

  it('is unreachable for anything else that went wrong', () => {
    expect(phaseFor(undefined, apiError(500), false)).toBe('unreachable');
    expect(phaseFor(undefined, new Error('network down'), false)).toBe('unreachable');
  });
});

describe('session gate', () => {
  it('allows a session only once a workspace accepts work', () => {
    expect(sessionGate('ready')).toEqual({ enabled: true, reason: null });
  });

  it('allows a session when paused, but says why it is slower', () => {
    expect(sessionGate('idle')).toEqual({ enabled: true, reason: 'paused' });
  });

  it('blocks a session while the workspace is being built', () => {
    expect(sessionGate('setting-up')).toEqual({ enabled: false, reason: 'setting-up' });
  });

  it('blocks a session when setup failed', () => {
    expect(sessionGate('failed')).toEqual({ enabled: false, reason: 'failed' });
  });

  it('blocks a session when there is no workspace at all', () => {
    expect(sessionGate('absent')).toEqual({ enabled: false, reason: 'absent' });
  });

  it('blocks a session when the caller has no access', () => {
    expect(sessionGate('not-permitted')).toEqual({ enabled: false, reason: 'not-permitted' });
  });

  it('blocks a session while the answer is still unknown', () => {
    expect(sessionGate('loading')).toEqual({ enabled: false, reason: 'loading' });
  });
});
