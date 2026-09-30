import { beforeEach, describe, expect, it } from 'vitest';
import { useAuthStore } from './auth';
import type { AuthUser } from '../lib/api/auth';

const user: AuthUser = {
  id: 'u1',
  kind: 'user',
  email: 'ada@example.com',
  name: 'Ada',
};

beforeEach(() => {
  useAuthStore.setState({ user: null, recorded: false });
});

describe('auth store', () => {
  it('starts with nothing known about the caller', () => {
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().recorded).toBe(false);
  });

  it('records a resolved user', () => {
    useAuthStore.getState().setSession(user);
    expect(useAuthStore.getState().user).toEqual(user);
    expect(useAuthStore.getState().recorded).toBe(true);
  });

  it('records the absence of a user', () => {
    useAuthStore.getState().setSession(null);
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().recorded).toBe(true);
  });

  it('clears back to anonymous', () => {
    useAuthStore.getState().setSession(user);
    useAuthStore.getState().clear();
    expect(useAuthStore.getState().user).toBeNull();
    expect(useAuthStore.getState().recorded).toBe(true);
  });

  it('holds no credential of its own', () => {
    const state = useAuthStore.getState() as unknown as Record<string, unknown>;
    expect(state.token).toBeUndefined();
    expect(localStorage.getItem('menzi_token')).toBeNull();
  });
});
