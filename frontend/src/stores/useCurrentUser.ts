import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getCurrentUser } from '../lib/api/whoami';
import { useAuthStore } from './auth';

export const CURRENT_USER_KEY = ['me'] as const;

/**
 * The gateway decides who the caller is, and the browser is told. Nothing in the
 * browser is allowed to claim an identity, so the workspace it asks about is
 * always the workspace the gateway resolved.
 */
export function useCurrentUser() {
  const setUser = useAuthStore((state) => state.setUser);

  const query = useQuery({
    queryKey: CURRENT_USER_KEY,
    queryFn: getCurrentUser,
    retry: false,
    staleTime: 5 * 60_000,
  });

  useEffect(() => {
    const current = query.data;
    if (!current || current.kind !== 'user') return;
    const store = useAuthStore.getState();
    if (store.user?.id !== current.id) setUser(current.id);
  }, [query.data, setUser]);

  return query;
}
