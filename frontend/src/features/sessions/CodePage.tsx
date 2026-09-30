import { useCallback } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams, useSearchParams } from 'react-router-dom';
import { Button, EmptyState, Icon, Tabs, useNotify } from '@canonical/react-components';
import { getErrorMessage } from '../../lib/api/errors';
import { openWorkspaceSession, listWorkspaceSessions } from '../../lib/api/workspaces';
import { queryKeys } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import { useAuthStore } from '../../stores/auth';
import { ChatPanel } from './ChatPanel';
import { CodeReviewPanel } from './CodeReviewPanel';
import { Composer } from './Composer';
import { Unavailable } from '../../components/Unavailable';
import { WorkspaceStatus } from '../workspaces/WorkspaceStatus';
import { blockReason } from '../workspaces/reasons';
import { useWorkspace } from '../workspaces/useWorkspace';
import { S } from '../../strings/catalogue';

const SESSION_ID_PATTERN = /^ses_[A-Za-z0-9]+$/;

function shortId(sessionId: string): string {
  return sessionId.replace(/^ses_/, '').slice(0, 8);
}

const REVIEW_PANEL = 'review';

function labelFor(session: { id: string; title?: string | null }): string {
  const title = session.title?.trim();
  if (title) return title;
  return shortId(session.id);
}

function timestampFor(session: { time?: { created?: number; updated?: number } }): string {
  const time = session.time?.updated ?? session.time?.created;
  if (typeof time !== 'number') return '';
  return ` · ${formatDateTime(new Date(time).toISOString())}`;
}

export function CodePage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const notify = useNotify();
  const user = useAuthStore((state) => state.user);
  const userId = user?.id;

  const workspaceState = useWorkspace(projectId, userId);
  const blocked = workspaceState.canStartSession ? null : blockReason(workspaceState.reason);

  const sessionsKey = queryKeys.opencode.workspaceSessions(userId ?? '', projectId ?? '');
  const sessionsQuery = useQuery({
    queryKey: sessionsKey,
    queryFn: () => listWorkspaceSessions(userId as string, projectId as string),
    enabled: !!projectId && !!userId && workspaceState.canStartSession,
    retry: false,
    refetchInterval: workspaceState.sessionsPollMs,
  });

  const requested = searchParams.get('session') ?? '';
  const sessions = sessionsQuery.data ?? [];
  const known = sessions.some((session) => session.id === requested);
  const active = requested && (known || SESSION_ID_PATTERN.test(requested)) ? requested : '';

  const openSession = useCallback(
    (sessionId: string) => {
      setSearchParams({ session: sessionId });
    },
    [setSearchParams],
  );

  const reviewOpen = searchParams.get('panel') === REVIEW_PANEL;

  const toggleReview = useCallback(() => {
    const next = new URLSearchParams(searchParams);
    if (reviewOpen) {
      next.delete('panel');
    } else {
      next.set('panel', REVIEW_PANEL);
    }
    setSearchParams(next);
  }, [reviewOpen, searchParams, setSearchParams]);

  const startSession = useCallback(async () => {
    const session = await openWorkspaceSession(
      userId as string,
      projectId as string,
    );
    queryClient.invalidateQueries({ queryKey: sessionsKey });
    openSession(session.id);
  }, [projectId, userId, queryClient, openSession, sessionsKey]);

  const startPending = useMutation({
    mutationFn: startSession,
    onError: (error) => notify.failure(S.code.startFailed, error, getErrorMessage(error)),
  });

  if (!projectId) {
    return <Unavailable icon="code" />;
  }

  const tabs = [
    ...sessions.map((session) => ({
      id: session.id,
      label: `${labelFor(session)}${timestampFor(session)}`,
      onClick: () => openSession(session.id),
      active: session.id === active,
    })),
    ...(active && !known
      ? [{ id: active, label: shortId(active), onClick: () => {}, active: true }]
      : []),
  ];

  const canStart = workspaceState.canStartSession && !startPending.isPending;

  return (
    <div className="app-code">
      <div className="app-page-header">
        <h2 className="p-heading--3">{S.sections.code}</h2>
        <div className="app-page-header__actions">
          <Button
            appearance="positive"
            disabled={!canStart}
            onClick={() => startPending.mutate()}
          >
            <Icon name="plus" />
            {S.code.newSession}
          </Button>
          <Button
            appearance="base"
            className="u-no-margin--bottom"
            aria-pressed={reviewOpen}
            aria-controls="code-review-panel"
            aria-label={reviewOpen ? S.code.hideReview : S.code.showReview}
            onClick={toggleReview}
          >
            <Icon name="file-blank" />
          </Button>
        </div>
      </div>

      <WorkspaceStatus state={workspaceState} onRetry={workspaceState.retry} />

      {blocked ? (
        <p className="u-text--muted" data-testid="session-blocked-reason">
          {blocked}
        </p>
      ) : null}

      {sessionsQuery.error ? (
        <p data-testid="sessions-error">{S.code.sessionsUnavailable}</p>
      ) : null}

      {tabs.length === 0 ? (
        <EmptyState title={S.code.emptyTitle} image={<Icon name="quote" />}>
          <p>{S.workspace.sessions.emptyBody}</p>
        </EmptyState>
      ) : (
        <>
          <Tabs links={tabs} />
          {active ? (
            <div
              className={`app-split${reviewOpen ? ' app-split--review' : ''}`}
              data-testid="code-split"
              data-review={reviewOpen ? 'open' : 'closed'}
            >
              <div className="app-code__chat">
                <ChatPanel sessionId={active} />
                <Composer sessionId={active} />
              </div>
              {reviewOpen ? (
                <div className="app-code__review" id="code-review-panel">
                  <CodeReviewPanel sessionId={active} />
                </div>
              ) : null}
            </div>
          ) : (
            <EmptyState title={S.code.pickTitle} image={<Icon name="quote" />}>
              <p>{S.code.pickBody}</p>
            </EmptyState>
          )}
        </>
      )}
    </div>
  );
}
