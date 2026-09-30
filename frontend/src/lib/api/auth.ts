import { http } from './client';
import { apiPaths } from '../routes';

export interface AuthUser {
  id: string;
  kind: 'user' | 'service';
  email?: string | null;
  name?: string | null;
  avatar_url?: string | null;
  org_id?: string | null;
  role?: string | null;
  has_password?: boolean;
}

export interface Session {
  user: AuthUser;
  csrf_token?: string | null;
}

export interface LoginProvider {
  id: string;
  label: string;
}

export interface Providers {
  registration: 'closed' | 'invite' | 'open';
  password: boolean;
  oidc: LoginProvider[];
}

export interface Device {
  id: string;
  provider: string;
  created_at: string;
  last_seen_at: string;
  current: boolean;
  user_agent?: string | null;
  ip?: string | null;
}

export function getProviders(): Promise<Providers> {
  return http.get<Providers>(apiPaths.auth.providers());
}

export function getSession(): Promise<Session> {
  return http.get<Session>(apiPaths.auth.session());
}

export function login(email: string, password: string): Promise<void> {
  return http.post<void>(apiPaths.auth.login(), { email, password });
}

export function register(email: string, name: string, password: string): Promise<void> {
  return http.post<void>(apiPaths.auth.register(), { email, name, password });
}

export function logout(): Promise<void> {
  return http.post<void>(apiPaths.auth.logout());
}

export function requestPasswordReset(email: string): Promise<void> {
  return http.post<void>(apiPaths.auth.forgot(), { email });
}

export function changePassword(
  currentPassword: string,
  newPassword: string,
): Promise<void> {
  return http.post<void>(apiPaths.auth.changePassword(), {
    current_password: currentPassword,
    new_password: newPassword,
  });
}

export async function listDevices(): Promise<Device[]> {
  const response = await http.get<{ devices: Device[] }>(apiPaths.auth.sessions());
  return response.devices;
}

export function revokeDevice(deviceId: string): Promise<void> {
  return http.delete<void>(apiPaths.auth.session_(deviceId));
}

export function oidcStartUrl(providerId: string, redirectTo?: string): string {
  const base = apiPaths.auth.oidcStart(providerId);
  return redirectTo ? `${base}?redirect_to=${encodeURIComponent(redirectTo)}` : base;
}
