import { http } from './client';
import { apiPaths } from '../routes';
import type { UserSshKey } from '../types';

export interface CreateUserSshKeyInput {
  label: string;
  public_key: string;
  private_key: string;
  passphrase?: string;
  is_default?: boolean;
}

export interface GeneratedUserSshKey {
  key: UserSshKey;
  private_key: string;
  public_key: string;
}

export function listUserSshKeys(): Promise<UserSshKey[]> {
  return http.get<UserSshKey[]>(apiPaths.me.sshKeys());
}

export function createUserSshKey(input: CreateUserSshKeyInput): Promise<UserSshKey> {
  return http.post<UserSshKey>(apiPaths.me.sshKeys(), input);
}

export function generateUserSshKey(input: {
  label: string;
  is_default?: boolean;
}): Promise<GeneratedUserSshKey> {
  return http.post<GeneratedUserSshKey>(apiPaths.me.sshKeyGenerate(), input);
}

export function updateUserSshKey(
  id: string,
  input: { label?: string; is_default?: boolean },
): Promise<UserSshKey> {
  return http.patch<UserSshKey>(apiPaths.me.sshKey_(id), input);
}

export function deleteUserSshKey(id: string): Promise<void> {
  return http.delete<void>(apiPaths.me.sshKey_(id));
}
