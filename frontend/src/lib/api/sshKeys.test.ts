import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createUserSshKey,
  deleteUserSshKey,
  generateUserSshKey,
  listUserSshKeys,
  updateUserSshKey,
} from './sshKeys';

const KEY = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  label: 'Laptop',
  public_key: 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFake menzi@test',
  fingerprint_sha256: 'SHA256:abc',
  is_default: true,
  last_used_at: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
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

describe('ssh keys api', () => {
  it('lists keys', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, [KEY]));
    vi.stubGlobal('fetch', fetchMock);
    await expect(listUserSshKeys()).resolves.toEqual([KEY]);
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/me/ssh-keys');
  });

  it('creates a key', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(201, KEY));
    vi.stubGlobal('fetch', fetchMock);
    await createUserSshKey({
      label: KEY.label,
      public_key: KEY.public_key,
      private_key: 'private',
    });
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/me/ssh-keys');
    expect((options as RequestInit).method).toBe('POST');
  });

  it('generates a key', async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValue(jsonResponse(201, { key: KEY, private_key: 'private', public_key: KEY.public_key }));
    vi.stubGlobal('fetch', fetchMock);
    await generateUserSshKey({ label: 'Generated key' });
    const [url] = fetchMock.mock.calls[0];
    expect(url).toBe('/api/v1/me/ssh-keys/generate');
  });

  it('updates a key', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(200, KEY));
    vi.stubGlobal('fetch', fetchMock);
    await updateUserSshKey(KEY.id, { is_default: true });
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/me/ssh-keys/${KEY.id}`);
    expect((options as RequestInit).method).toBe('PATCH');
  });

  it('deletes a key', async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse(204, null));
    vi.stubGlobal('fetch', fetchMock);
    await deleteUserSshKey(KEY.id);
    const [url, options] = fetchMock.mock.calls[0];
    expect(url).toBe(`/api/v1/me/ssh-keys/${KEY.id}`);
    expect((options as RequestInit).method).toBe('DELETE');
  });
});
