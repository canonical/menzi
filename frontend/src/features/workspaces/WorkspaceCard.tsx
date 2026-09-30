import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Chip,
  ConfirmationModal,
  ContextualMenu,
  MainTable,
  Spinner,
  useNotify,
} from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import { getErrorMessage } from '../../lib/api/errors';
import {
  destroyWorkspace,
  listWorkspacesForProject,
  startWorkspace,
  suspendWorkspace,
} from '../../lib/api/workspaces';
import { queryKeys } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import type { Workspace } from '../../lib/types';
import { S } from '../../strings/catalogue';

type Row = NonNullable<MainTableProps['rows']>[number];
type Appearance = 'caution' | 'information' | 'negative' | 'positive';

interface WorkspaceCardProps {
  projectId: string;
  userId: string | undefined;
  onChanged?: () => void;
}

function appearanceFor(status: string): Appearance {
  switch (status) {
    case 'ready':
    case 'running':
      return 'positive';
    case 'idle':
    case 'requested':
    case 'provisioning':
      return 'caution';
    case 'deleted':
      return 'negative';
    default:
      return 'information';
  }
}

function labelFor(status: string): string {
  const statuses = S.workspace.card.statuses as Record<string, string>;
  return statuses[status] ?? status;
}

function mine(workspaces: Workspace[], userId: string | undefined): Workspace | null {
  if (!userId) return null;
  return workspaces.find((workspace) => workspace.user_id === userId) ?? null;
}

export function WorkspaceCard({ projectId, userId, onChanged }: WorkspaceCardProps) {
  const queryClient = useQueryClient();
  const notify = useNotify();
  const [confirming, setConfirming] = useState<Workspace | null>(null);

  const projectKey = queryKeys.workspaces.forProject(projectId);
  const workspacesQuery = useQuery({
    queryKey: projectKey,
    queryFn: () => listWorkspacesForProject(projectId),
    enabled: !!projectId,
    retry: false,
  });

  const workspace = mine(workspacesQuery.data ?? [], userId);
  const detailKey = queryKeys.workspaces.detail(userId ?? '', projectId);

  const invalidate = () => {
    queryClient.invalidateQueries({ queryKey: projectKey });
    queryClient.invalidateQueries({ queryKey: detailKey });
    onChanged?.();
  };

  const pauseMutation = useMutation({
    mutationFn: () => suspendWorkspace(userId as string, projectId),
    onSuccess: () => {
      invalidate();
      notify.success(S.workspace.card.toasts.paused);
    },
    onError: (error) => notify.failure(S.workspace.card.toasts.failed, error, getErrorMessage(error)),
  });

  const resumeMutation = useMutation({
    mutationFn: () => startWorkspace(userId as string, projectId),
    onSuccess: () => {
      invalidate();
      notify.success(S.workspace.card.toasts.resumed);
    },
    onError: (error) => notify.failure(S.workspace.card.toasts.failed, error, getErrorMessage(error)),
  });

  const deleteMutation = useMutation({
    mutationFn: () => destroyWorkspace(userId as string, projectId),
    onSuccess: () => {
      setConfirming(null);
      invalidate();
      notify.success(S.workspace.card.toasts.deleted);
    },
    onError: (error) => notify.failure(S.workspace.card.toasts.failed, error, getErrorMessage(error)),
  });

  const busy = pauseMutation.isPending || resumeMutation.isPending;

  if (workspacesQuery.isLoading) {
    return (
      <div className="u-align--center">
        <Spinner text={S.dataState.loading} />
      </div>
    );
  }

  if (workspacesQuery.error) {
    return (
      <div className="p-card" data-testid="workspace-card">
        <div className="p-card__content">
          <h3 className="p-heading--5">{S.workspace.card.title}</h3>
          <p className="u-text--muted">{getErrorMessage(workspacesQuery.error)}</p>
        </div>
      </div>
    );
  }

  if (!workspace) {
    return (
      <div className="p-card" data-testid="workspace-card">
        <div className="p-card__content">
          <h3 className="p-heading--5">{S.workspace.card.title}</h3>
          <p>{S.workspace.card.none}</p>
          <p className="u-text--muted">{S.workspace.card.noneBody}</p>
        </div>
      </div>
    );
  }

  const rows: Row[] = [
    {
      columns: [
        {
          content: (
            <Chip
              value={labelFor(workspace.status)}
              appearance={appearanceFor(workspace.status)}
              isReadOnly
              isDense
            />
          ),
        },
        { content: workspace.instance_name ?? '—' },
        { content: formatDateTime(workspace.created_at) },
        {
          content: (
            <ContextualMenu
              hasToggleIcon
              position="right"
              toggleLabel={S.workspace.card.columns.actions}
              toggleAppearance="base"
              disabled={busy}
              links={[
                ...(workspace.status === 'idle'
                  ? [
                      {
                        children: S.workspace.card.actions.resume,
                        onClick: () => resumeMutation.mutate(),
                      },
                    ]
                  : [
                      {
                        children: S.workspace.card.actions.pause,
                        onClick: () => pauseMutation.mutate(),
                      },
                    ]),
                {
                  children: S.workspace.card.actions.delete,
                  onClick: () => setConfirming(workspace),
                },
              ]}
            />
          ),
        },
      ],
    },
  ];

  return (
    <div className="p-card" data-testid="workspace-card">
      <div className="p-card__content">
        <h3 className="p-heading--5">{S.workspace.card.title}</h3>
        <MainTable
          headers={[
            { content: S.workspace.card.columns.status },
            { content: S.workspace.card.columns.instance },
            { content: S.workspace.card.columns.created },
            { content: S.workspace.card.columns.actions },
          ]}
          rows={rows}
          responsive
        />
      </div>
      {confirming ? (
        <ConfirmationModal
          title={S.workspace.card.deleteTitle}
          confirmButtonLabel={S.workspace.card.deleteConfirm}
          cancelButtonLabel={S.workspace.card.deleteCancel}
          confirmButtonAppearance="negative"
          confirmButtonLoading={deleteMutation.isPending}
          onConfirm={() => deleteMutation.mutate()}
          close={() => setConfirming(null)}
        >
          <p>{S.workspace.card.deleteBody}</p>
        </ConfirmationModal>
      ) : null}
    </div>
  );
}


