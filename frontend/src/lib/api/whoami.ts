import { http } from './client';

export interface CurrentUser {
  id: string;
  kind: 'user' | 'service';
}

export async function getCurrentUser(): Promise<CurrentUser> {
  return http.get<CurrentUser>('/api/v1/me');
}
