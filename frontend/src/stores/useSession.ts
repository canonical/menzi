import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { getSession, logout as endSession } from '../lib/api/auth';
import { queryKeys } from '../lib/routes';
import { useAuthStore, type SessionStatus } from './auth';

export function useSession() {
  const queryClient = useQueryClient();
  const setSession = useAuthStore((state) => state.setSession);
  const clear = useAuthStore((state) => state.clear);

  const query = useQuery({
    queryKey: queryKeys.session(),
    queryFn: getSession,
    retry: false,
    staleTime: 60_000,
  });

  const pending = query.isPending;
  const resolved = pending ? null : (query.data?.user ?? null);

  useEffect(() => {
    if (pending) return;
    const store = useAuthStore.getState();
    if (store.recorded && store.user?.id === resolved?.id) return;
    setSession(resolved);
  }, [pending, resolved, setSession]);

  const status: SessionStatus = pending
    ? 'loading'
    : resolved
      ? 'authenticated'
      : 'anonymous';

  const signOut = async () => {
    await endSession();
    queryClient.clear();
    clear();
  };

  return {
    status,
    user: resolved,
    error: query.error,
    refresh: () => queryClient.invalidateQueries({ queryKey: queryKeys.session() }),
    signOut,
  };
}
