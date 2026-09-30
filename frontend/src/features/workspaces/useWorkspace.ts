import { useEffect, useRef } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { getWorkspace, ensureWorkspace, WorkspaceNotFoundError } from '../../lib/api/workspaces';
import { queryKeys } from '../../lib/routes';
import type { Workspace } from '../../lib/types';

const POLL_INTERVAL_MS = 3000;
const SLOW_AFTER_MS = 90_000;
const SESSION_POLL_INTERVAL_MS = 10_000;
const RUNNING_STATUSES = ['ready', 'running', 'idle'];

export type WorkspacePhase =
  | 'loading'
  | 'absent'
  | 'setting-up'
  | 'failed'
  | 'ready'
  | 'idle'
  | 'unreachable'
  | 'not-permitted';

export interface WorkspaceState {
  phase: WorkspacePhase;
  workspace: Workspace | null;
  /** True while the status should be shown; false once things are ready. */
  shouldAnnounce: boolean;
  elapsedMs: number;
  isSlow: boolean;
  error: string | null;
  lastError: string | null;
  canStartSession: boolean;
  reason: string | null;
}

export function phaseFor(
  workspace: Workspace | null | undefined,
  error: unknown,
  loading: boolean,
): WorkspacePhase {
  if (error instanceof WorkspaceNotFoundError) return 'absent';
  if (isNotFound(error)) return 'absent';
  if (error && statusOf(error) === 403) return 'not-permitted';
  if (error) return 'unreachable';
  if (loading || workspace === undefined) return 'loading';
  if (!workspace) return 'absent';
  if (workspace.status === 'requested') {
    return workspace.last_error ? 'failed' : 'setting-up';
  }
  if (workspace.status === 'provisioning') return 'setting-up';
  if (workspace.status === 'archived' || workspace.status === 'deleted') return 'absent';
  if (workspace.status === 'idle') return 'idle';
  return 'ready';
}

export function sessionGate(phase: WorkspacePhase): { enabled: boolean; reason: string | null } {
  switch (phase) {
    case 'ready':
      return { enabled: true, reason: null };
    case 'idle':
      return { enabled: true, reason: 'paused' };
    case 'setting-up':
      return { enabled: false, reason: 'setting-up' };
    case 'failed':
      return { enabled: false, reason: 'failed' };
    case 'not-permitted':
      return { enabled: false, reason: 'not-permitted' };
    case 'unreachable':
      return { enabled: false, reason: 'unreachable' };
    case 'loading':
      return { enabled: false, reason: 'loading' };
    default:
      return { enabled: false, reason: 'absent' };
  }
}

export function useWorkspace(projectId: string | undefined, userId: string | undefined) {
  const queryClient = useQueryClient();
  const enabled = !!projectId && !!userId;
  const key = queryKeys.workspaces.detail(userId ?? '', projectId ?? '');

  const query = useQuery({
    queryKey: key,
    queryFn: () => getWorkspace(userId as string, projectId as string),
    enabled,
    retry: false,
    refetchInterval: (current) => {
      const status = current.state.data?.status;
      return status === 'requested' || status === 'provisioning' ? POLL_INTERVAL_MS : false;
    },
  });

  const phase = phaseFor(
    enabled ? query.data : undefined,
    query.error,
    query.isLoading && enabled,
  );

  const ensure = useMutation({
    mutationFn: () => ensureWorkspace({ projectId: projectId as string }),
    onSuccess: (workspace) => {
      queryClient.setQueryData(key, workspace);
    },
  });

  const prefetched = useRef(false);
  useEffect(() => {
    if (!enabled) return;
    if (phase !== 'absent') return;
    if (prefetched.current) return;
    prefetched.current = true;
    ensure.mutate();
  }, [enabled, phase, ensure]);

  const startedAt = useRef<number | null>(null);
  useEffect(() => {
    if (phase === 'setting-up' && startedAt.current === null) {
      startedAt.current = Date.now();
    }
    if (phase !== 'setting-up' && phase !== 'failed') {
      startedAt.current = null;
    }
  }, [phase]);

  const gate = sessionGate(phase);
  const announce =
    phase === 'setting-up' ||
    phase === 'failed' ||
    phase === 'not-permitted' ||
    phase === 'unreachable' ||
    phase === 'idle';

  return {
    workspace: query.data ?? null,
    phase,
    shouldAnnounce: announce,
    elapsedMs: startedAt.current ? Date.now() - startedAt.current : 0,
    isSlow: startedAt.current ? Date.now() - startedAt.current > SLOW_AFTER_MS : false,
    error: query.error ? messageOf(query.error) : null,
    lastError: query.data?.last_error ?? null,
    canStartSession: gate.enabled,
    reason: gate.reason,
    sessionsPollMs: RUNNING_STATUSES.includes(query.data?.status ?? '')
      ? SESSION_POLL_INTERVAL_MS
      : (false as const),
    isPrefetching: ensure.isPending,
    retry: () => ensure.mutate(),
    refetch: query.refetch,
    queryKey: key,
  };
}

function statusOf(error: unknown): number | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const status = (error as { status?: unknown }).status;
  return typeof status === 'number' ? status : undefined;
}

function isNotFound(error: unknown): boolean {
  return statusOf(error) === 404;
}

function messageOf(error: unknown): string {
  if (error instanceof Error) return error.message;
  return 'Unexpected error';
}
