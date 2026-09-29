import { useCallback } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useParams, useSearchParams } from 'react-router-dom';
import { Button, EmptyState, Icon, Tabs, useNotify } from '@canonical/react-components';
import { createSession, listSessions } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import { ChatPanel } from './ChatPanel';
import { CodeReviewPanel } from './CodeReviewPanel';
import { Composer } from './Composer';
import { Unavailable } from '../../components/Unavailable';
import { S } from '../../strings/catalogue';

const SESSION_ID_PATTERN = /^ses_[A-Za-z0-9]+$/;

function shortId(sessionId: string): string {
  return sessionId.replace(/^ses_/, '').slice(0, 8);
}

function labelFor(session: { id: string; time?: { created?: number; updated?: number } }): string {
  const time = session.time?.updated ?? session.time?.created;
  if (typeof time === 'number') {
    return `${shortId(session.id)} · ${formatDateTime(new Date(time).toISOString())}`;
  }
  return shortId(session.id);
}

export function CodePage() {
  const { projectId } = useParams<{ projectId: string }>();
  const [searchParams, setSearchParams] = useSearchParams();
  const queryClient = useQueryClient();
  const notify = useNotify();

  const sessionsQuery = useQuery({
    queryKey: queryKeys.opencode.sessions(),
    queryFn: listSessions,
    retry: false,
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

  const startSession = useCallback(async () => {
    const session = await createSession();
    queryClient.invalidateQueries({ queryKey: queryKeys.opencode.sessions() });
    openSession(session.id);
  }, [queryClient, openSession]);

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
      label: labelFor(session),
      onClick: () => openSession(session.id),
      active: session.id === active,
    })),
    ...(active && !known
      ? [{ id: active, label: shortId(active), onClick: () => {}, active: true }]
      : []),
  ];

  return (
    <div>
      <div className="u-flex u-justify-space-between u-align--center">
        <h2 className="p-heading--3">{S.sections.code}</h2>
        <Button
          appearance="positive"
          disabled={startPending.isPending}
          onClick={() => startPending.mutate()}
        >
          <Icon name="plus" />
          {S.code.newSession}
        </Button>
      </div>

      {sessionsQuery.error ? (
        <p data-testid="sessions-error">{S.code.sessionsUnavailable}</p>
      ) : null}

      {tabs.length === 0 ? (
        <EmptyState title={S.code.emptyTitle} image={<Icon name="comment" />}>
          <p>{S.code.emptyBody}</p>
        </EmptyState>
      ) : (
        <>
          <Tabs links={tabs} />
          {active ? (
            <div className="app-split">
              <div>
                <ChatPanel sessionId={active} />
                <Composer sessionId={active} />
              </div>
              <div>
                <CodeReviewPanel sessionId={active} />
              </div>
            </div>
          ) : (
            <EmptyState title={S.code.pickTitle} image={<Icon name="comment" />}>
              <p>{S.code.pickBody}</p>
            </EmptyState>
          )}
        </>
      )}
    </div>
  );
}
