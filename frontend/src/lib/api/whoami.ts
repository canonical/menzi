import { http } from './client';

export interface CurrentUser {
  id: string;
  kind: 'user' | 'service';
  email?: string | null;
  name?: string | null;
  avatar_url?: string | null;
  org_id?: string | null;
  role?: string | null;
  has_password?: boolean;
}

export async function getCurrentUser(): Promise<CurrentUser> {
  return http.get<CurrentUser>('/api/v1/me');
}
