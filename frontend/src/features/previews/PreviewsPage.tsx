import { useState } from 'react';
import { useParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Chip,
  ConfirmationModal,
  ContextualMenu,
  Icon,
  MainTable,
  useNotify,
} from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import {
  createPreview,
  listPreviews,
  resetPreview,
  restartPreview,
  teardownPreview,
} from '../../lib/api/previews';
import { queryKeys } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import { getErrorMessage } from '../../lib/api/errors';
import type { Preview } from '../../lib/types';
import { S } from '../../strings/catalogue';

type PreviewRow = NonNullable<MainTableProps['rows']>[number];
type ChipAppearance = 'caution' | 'information' | 'negative' | 'positive';

function appearanceFor(status: string): ChipAppearance {
  switch (status) {
    case 'ready':
      return 'positive';
    case 'starting':
      return 'caution';
    case 'failed':
      return 'negative';
    default:
      return 'information';
  }
}

function labelFor(status: string): string {
  switch (status) {
    case 'starting':
      return S.previews.statuses.starting;
    case 'ready':
      return S.previews.statuses.ready;
    case 'failed':
      return S.previews.statuses.failed;
    default:
      return status;
  }
}

export function PreviewsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const queryClient = useQueryClient();
  const notify = useNotify();
  const [teardownTarget, setTeardownTarget] = useState<Preview | null>(null);
  const route = projectId ?? '';

  const { data: previews, isLoading, error, refetch } = useQuery({
    queryKey: queryKeys.previews.forProject(route),
    queryFn: () => listPreviews(route),
    enabled: !!projectId,
  });

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: queryKeys.previews.forProject(route) });

  const createMutation = useMutation({
    mutationFn: () => createPreview({ projectId: route }),
    onSuccess: () => {
      invalidate();
      notify.success(S.previews.toasts.created);
    },
    onError: (err) => notify.failure(S.previews.toasts.title, err, getErrorMessage(err)),
  });

  const resetMutation = useMutation({
    mutationFn: (id: string) => resetPreview(id),
    onSuccess: () => {
      invalidate();
      notify.success(S.previews.toasts.resetStarted);
    },
    onError: (err) => notify.failure(S.previews.toasts.title, err, getErrorMessage(err)),
  });

  const restartMutation = useMutation({
    mutationFn: (id: string) => restartPreview(id),
    onSuccess: () => {
      invalidate();
      notify.success(S.previews.toasts.restartStarted);
    },
    onError: (err) => notify.failure(S.previews.toasts.title, err, getErrorMessage(err)),
  });

  const teardownMutation = useMutation({
    mutationFn: (id: string) => teardownPreview(id),
    onSuccess: () => {
      setTeardownTarget(null);
      invalidate();
      notify.success(S.previews.toasts.tornDown);
    },
    onError: (err) => notify.failure(S.previews.toasts.title, err, getErrorMessage(err)),
  });

  const rows: PreviewRow[] = (previews ?? []).map((preview) => ({
    columns: [
      {
        content: (
          <Chip
            value={labelFor(preview.status)}
            appearance={appearanceFor(preview.status)}
            isReadOnly
            isDense
          />
        ),
      },
      {
        content:
          preview.mode === 'pinned' ? S.previews.modes.pinned : S.previews.modes.live,
      },
      { content: preview.branch || S.previews.branchDefault },
      { content: formatDateTime(preview.created_at) },
      {
        content: (
          <ContextualMenu
            hasToggleIcon
            position="right"
            toggleLabel={S.previews.actions.menu}
            toggleAppearance="base"
            links={[
              {
                children: S.previews.actions.reset,
                onClick: () => resetMutation.mutate(preview.id),
              },
              {
                children: S.previews.actions.restart,
                onClick: () => restartMutation.mutate(preview.id),
              },
              {
                children: S.previews.actions.teardown,
                onClick: () => setTeardownTarget(preview),
              },
            ]}
          />
        ),
      },
    ],
  }));

  return (
    <div className="app-content">
      <div className="app-page-header">
        <h1 className="p-heading--2">{S.previews.title}</h1>
        <div className="app-page-header__actions">
          <Button
            appearance="positive"
            onClick={() => createMutation.mutate()}
            disabled={createMutation.isPending || !projectId}
          >
            <Icon name="plus" />
            {S.previews.create}
          </Button>
        </div>
      </div>
      <DataState
        loading={isLoading}
        error={error ? getErrorMessage(error) : null}
        empty={!previews?.length}
        emptyTitle={S.previews.emptyTitle}
        emptyBody={S.previews.emptyBody}
        onRetry={() => refetch()}
      >
        <MainTable headers={previewHeaders} rows={rows} responsive />
      </DataState>
      {teardownTarget && (
        <ConfirmationModal
          title={S.previews.confirmations.teardownTitle}
          confirmButtonLabel={S.previews.confirmations.confirm}
          cancelButtonLabel={S.previews.confirmations.cancel}
          confirmButtonAppearance="negative"
          confirmButtonLoading={teardownMutation.isPending}
          onConfirm={() => teardownMutation.mutate(teardownTarget.id)}
          close={() => setTeardownTarget(null)}
        >
          <p>{S.previews.confirmations.teardownBody}</p>
        </ConfirmationModal>
      )}
    </div>
  );
}

const previewHeaders = [
  { content: S.previews.columns.status },
  { content: S.previews.columns.mode },
  { content: S.previews.columns.branch },
  { content: S.previews.columns.created },
  { content: S.previews.columns.actions },
];