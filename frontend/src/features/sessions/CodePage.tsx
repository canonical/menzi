import { useCallback, useEffect, useRef, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams, useSearchParams } from 'react-router-dom';
import { Button, EmptyState, Icon, Tabs, useNotify } from '@canonical/react-components';
import { getErrorMessage } from '../../lib/api/errors';
import { openWorkspaceSession, listWorkspaceSessions, workspaceTreeChanges } from '../../lib/api/workspaces';
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
import { useSessionForms } from '../../lib/chat/useSessionForms';

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
  const sessions = (sessionsQuery.data ?? []).filter((session) => !session.parentID);
  const known = sessions.some((session) => session.id === requested);
  const active = requested && (known || SESSION_ID_PATTERN.test(requested)) ? requested : '';
  const activeDirectory = sessions.find((session) => session.id === active)?.directory ?? undefined;

  const openSession = useCallback(
    (sessionId: string) => {
      setSearchParams({ session: sessionId });
    },
    [setSearchParams],
  );

  const panel = searchParams.get('panel');
  const reviewOpen = panel === REVIEW_PANEL || panel === 'review-full';
  const layout = !reviewOpen ? 'chat' : panel === 'review-full' ? 'review' : 'split';
  const [reviewMounted, setReviewMounted] = useState(reviewOpen);
  const reviewButtonRef = useRef<HTMLButtonElement>(null);
  const changesQuery = useQuery({
    queryKey: queryKeys.workspaces.diff(userId ?? '', projectId ?? '', activeDirectory ?? ''),
    queryFn: () => workspaceTreeChanges(userId as string, projectId as string, activeDirectory),
    enabled: !!userId && !!projectId && !!active && workspaceState.canStartSession,
    retry: false,
    refetchInterval: 15000,
  });
  const changeCount = Array.isArray(changesQuery.data?.changes)
    ? changesQuery.data.changes.length
    : null;

  const setLayout = useCallback((nextLayout: 'chat' | 'split' | 'review') => {
    const next = new URLSearchParams(searchParams);
    if (nextLayout === 'chat') {
      next.delete('panel');
      reviewButtonRef.current?.focus();
    } else {
      setReviewMounted(true);
      next.set('panel', nextLayout === 'review' ? 'review-full' : REVIEW_PANEL);
    }
    setSearchParams(next);
  }, [searchParams, setSearchParams]);

  const { pending } = useSessionForms(active);
  const pendingId = pending[0]?.id;
  const revealedQuestion = useRef<string | null>(null);
  useEffect(() => {
    if (!pendingId || revealedQuestion.current === pendingId) return;
    revealedQuestion.current = pendingId;
    if (layout === 'review') setLayout('split');
  }, [pendingId, layout, setLayout]);

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
      <div className="app-code-header">
        <h1 className="u-off-screen">{S.sections.code}</h1>
        <div className="app-code-header__tabs">
          {tabs.length > 0 ? <Tabs links={tabs} /> : null}
        </div>
        <div className="app-code-header__actions">
          <Button
            appearance="base"
            className="app-code-header__button"
            aria-label={S.code.newSession}
            title={S.code.newSession}
            disabled={!canStart}
            onClick={() => startPending.mutate()}
          >
            <Icon name="plus" />
          </Button>
          <Button
            ref={reviewButtonRef}
            appearance="base"
            className="app-code-header__button"
            aria-pressed={reviewOpen}
            aria-controls="code-review-panel"
            aria-label={reviewOpen ? S.code.hideReview : S.code.showReview}
            title={reviewOpen ? S.code.hideReview : `${S.code.changes}${changeCount !== null ? ` · ${changeCount}` : ''}`}
            disabled={!active}
            onClick={() => setLayout(reviewOpen ? 'chat' : 'split')}
          >
            <svg className="app-code__review-toggle-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
              <rect x="2.5" y="3.5" width="15" height="13" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
              <path d="M11.5 4v12" stroke="currentColor" strokeWidth="1.5" />
              <path d="M12 4h4a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1h-4z" fill="currentColor" opacity={reviewOpen ? 0.5 : 0.2} />
            </svg>
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
          {active ? (
            <div
              className="app-split app-split--animated"
              data-testid="code-split"
              data-review={reviewOpen ? 'open' : 'closed'}
              data-layout={layout}
            >
              <div
                className="app-code__chat"
                id="code-conversation-panel"
                inert={layout === 'review'}
                aria-hidden={layout === 'review'}
              >
                <ChatPanel sessionId={active} />
                {pending.length > 0 ? <p className="app-question__composer-hint">{S.questions.composerHint}</p> : null}
                <Composer sessionId={active} />
              </div>
              <div className="app-code__divider" aria-hidden={!reviewOpen} inert={!reviewOpen}>
                <Button
                  appearance="base"
                  className="app-code__panel-control"
                  aria-label={layout === 'review' ? S.code.splitReview : S.code.expandReview}
                  title={layout === 'review' ? S.code.splitReview : S.code.expandReview}
                  aria-controls="code-conversation-panel code-review-panel"
                  onClick={() => setLayout(layout === 'review' ? 'split' : 'review')}
                >
                  <Icon name="chevron" className={layout === 'review' ? 'app-code__arrow--right' : 'app-code__arrow--left'} />
                </Button>
              </div>
              <div className="app-code__review" id="code-review-panel" inert={!reviewOpen} aria-hidden={!reviewOpen}>
                {reviewMounted || reviewOpen ? (
                  <CodeReviewPanel
                    key={`${userId}:${projectId}:${activeDirectory ?? ''}`}
                    directory={activeDirectory}
                    userId={userId}
                    projectId={projectId}
                  />
                ) : null}
              </div>
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
