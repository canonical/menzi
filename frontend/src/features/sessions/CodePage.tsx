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
import { TerminalPanel } from './TerminalPanel';
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
  const terminalOpen = searchParams.get('terminal') === '1';
  const reviewOpen = panel === REVIEW_PANEL || panel === 'review-full';
  const sideOpen = reviewOpen || terminalOpen;
  const layout = !sideOpen ? 'chat' : panel === 'review-full' ? 'review' : 'split';
  const sideMode: 'review' | 'terminal' | 'both' = reviewOpen && terminalOpen
    ? 'both'
    : reviewOpen
      ? 'review'
      : 'terminal';
  const [splitRatio, setSplitRatio] = useState(55);
  const [dragging, setDragging] = useState(false);
  const sideSplitRef = useRef<HTMLDivElement>(null);
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
      next.delete('terminal');
      reviewButtonRef.current?.focus();
    } else {
      if (!sideOpen) next.set('panel', REVIEW_PANEL);
      if (next.get('panel') === null) next.set('panel', REVIEW_PANEL);
      if (nextLayout === 'review') next.set('panel', 'review-full');
      if (nextLayout === 'split' && next.get('panel') === 'review-full') next.set('panel', REVIEW_PANEL);
    }
    setSearchParams(next);
  }, [searchParams, setSearchParams, sideOpen]);

  const toggleReview = useCallback(() => {
    const next = new URLSearchParams(searchParams);
    if (reviewOpen) {
      next.delete('panel');
      if (!terminalOpen) next.delete('terminal');
    } else {
      next.set('panel', REVIEW_PANEL);
    }
    setSearchParams(next);
  }, [reviewOpen, searchParams, setSearchParams, terminalOpen]);

  const toggleTerminal = useCallback(() => {
    const next = new URLSearchParams(searchParams);
    if (terminalOpen) {
      next.delete('terminal');
      if (!reviewOpen) next.delete('panel');
    } else {
      next.set('terminal', '1');
    }
    setSearchParams(next);
  }, [reviewOpen, searchParams, setSearchParams, terminalOpen]);

  useEffect(() => {
    if (!dragging) return;
    const onMove = (event: MouseEvent) => {
      const container = sideSplitRef.current;
      if (!container) return;
      const box = container.getBoundingClientRect();
      if (box.height <= 0) return;
      const next = Math.min(80, Math.max(20, ((event.clientY - box.top) / box.height) * 100));
      setSplitRatio(next);
    };
    const onUp = () => setDragging(false);
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [dragging]);

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
              onClick={toggleReview}
            >
              <svg className="app-code__header-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                <path d="M4.5 3.5h11a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1z" stroke="currentColor" strokeWidth="1.5" />
                <path d="M6.5 7.25h7M6.5 10h7M6.5 12.75h4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                <path d="M14.4 12.8l2 2M12 13.9a2.1 2.1 0 1 0 0-4.2 2.1 2.1 0 0 0 0 4.2z" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </Button>
            <Button
              appearance="base"
              className="app-code-header__button"
              aria-pressed={terminalOpen}
              aria-controls="code-terminal-panel"
              aria-label={terminalOpen ? S.code.hideTerminal : S.code.showTerminal}
              title={terminalOpen ? S.code.hideTerminal : S.code.showTerminal}
              disabled={!active}
              onClick={toggleTerminal}
            >
              <svg className="app-code__header-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
                <rect x="2.75" y="3.75" width="14.5" height="12.5" rx="1.5" stroke="currentColor" strokeWidth="1.5" />
                <path d="M5.75 8.25L8.5 10l-2.75 1.75M9.75 12h4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
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
              data-review={sideOpen ? 'open' : 'closed'}
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
              <div className="app-code__divider" aria-hidden={!sideOpen} inert={!sideOpen}>
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
              <div className="app-code__review" id="code-review-panel" inert={!sideOpen} aria-hidden={!sideOpen}>
                <div
                  className="app-side-panel"
                >
                  <div
                    className="app-side-panel__split"
                    ref={sideSplitRef}
                    data-mode={sideMode}
                    style={sideMode === 'both'
                      ? { gridTemplateRows: `minmax(8rem, ${splitRatio}fr) 0.5rem minmax(8rem, ${100 - splitRatio}fr)` }
                      : undefined}
                  >
                    {sideMode === 'review' || sideMode === 'both' ? (
                      <div>
                        <CodeReviewPanel
                          key={`${userId}:${projectId}:${activeDirectory ?? ''}`}
                          directory={activeDirectory}
                          userId={userId}
                          projectId={projectId}
                        />
                      </div>
                    ) : null}
                    {sideMode === 'both' ? (
                      <div
                        className="app-side-panel__divider"
                        role="separator"
                        aria-orientation="horizontal"
                        onMouseDown={() => {
                          setDragging(true);
                        }}
                      />
                    ) : null}
                    {sideMode === 'terminal' || sideMode === 'both' ? (
                      <div id="code-terminal-panel">
                        <TerminalPanel userId={userId} projectId={projectId} open={terminalOpen} />
                      </div>
                    ) : null}
                  </div>
                </div>
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
