import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button, Chip } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { workspaceFilePatch, workspaceTreeChanges, type TreeDiff } from '../../lib/api/workspaces';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { DiffViewer } from './DiffViewer';
import { S } from '../../strings/catalogue';

const STATUS_APPEARANCE: Record<string, 'caution' | 'information' | 'negative' | 'positive'> = {
  added: 'positive',
  copied: 'information',
  deleted: 'negative',
  modified: 'caution',
  renamed: 'information',
  untracked: 'positive',
};

export function CodeReviewPanel({
  directory,
  userId,
  projectId,
}: {
  directory?: string;
  userId?: string;
  projectId?: string;
}) {
  const [selected, setSelected] = useState<string | null>(null);

  const enabled = !!userId && !!projectId;
  const listQuery = useQuery({
    queryKey: queryKeys.workspaces.diff(userId ?? '', projectId ?? '', directory ?? ''),
    queryFn: () => workspaceTreeChanges(userId as string, projectId as string, directory),
    enabled,
    retry: false,
    refetchInterval: 15000,
  });

  const changes = Array.isArray(listQuery.data?.changes) ? listQuery.data!.changes : [];
  const active = changes.find((change) => change.file === selected) ?? changes[0] ?? null;

  const patchQuery = useQuery({
    queryKey: queryKeys.workspaces.diffPatch(
      userId ?? '',
      projectId ?? '',
      directory ?? '',
      active?.file ?? '',
    ),
    queryFn: () =>
      workspaceFilePatch(
        userId as string,
        projectId as string,
        directory,
        active?.file ?? '',
      ),
    enabled: enabled && !!active,
    retry: false,
  });

  const change = patchQuery.data?.changes.find((entry) => entry.file === active?.file) ?? active;
  const patch = change?.patch ?? '';
  const head = listQuery.data?.head ?? patchQuery.data?.head ?? '';

  return (
    <section className="app-review">
      <h2 className="app-panel-heading">{S.review.title}</h2>

      <DataState
        loading={listQuery.isLoading}
        error={listQuery.error ? getErrorMessage(listQuery.error) : null}
        empty={!listQuery.isLoading && !listQuery.error && changes.length === 0}
        emptyIcon="file-blank"
        emptyTitle={S.review.emptyTitle}
        emptyBody={S.review.emptyBody}
        onRetry={() => listQuery.refetch()}
      >
        <div className="app-review__panes">
          <ul className="app-review__files" aria-label={S.review.filesLabel}>
            {changes.map((entry) => (
              <FileRow
                key={entry.file}
                file={entry.file}
                additions={entry.additions}
                deletions={entry.deletions}
                status={entry.status}
                active={entry.file === active?.file}
                onSelect={() => setSelected(entry.file)}
              />
            ))}
          </ul>

          <div className="app-review__viewer">
            {change && patch ? (
              <>
                <div className="app-review__viewer-header">
                  <h3 className="p-heading--5 u-no-margin--bottom">{change.file}</h3>
                  <Chip
                    value={`+${change.additions} -${change.deletions}`}
                    appearance={change.additions > 0 ? 'positive' : 'information'}
                    isReadOnly
                    isDense
                  />
                </div>
                {change.truncated ? (
                  <p className="p-form-validation__message">{S.review.patchTruncated}</p>
                ) : null}
                <DiffViewer file={change.file} patch={patch} />
              </>
            ) : change?.binary ? (
              <p className="u-no-margin--bottom">{S.review.binaryFile}</p>
            ) : (
              <p className="u-no-margin--bottom">{S.review.noPatch}</p>
            )}
          </div>
        </div>
        <p className="app-review__base" data-testid="review-base">
          {head ? S.review.againstHead.replace('{head}', head.slice(0, 12)) : ''}
        </p>
      </DataState>
    </section>
  );
}

function FileRow({
  file,
  additions,
  deletions,
  status,
  active,
  onSelect,
}: {
  file: string;
  additions: number;
  deletions: number;
  status: string;
  active: boolean;
  onSelect: () => void;
}) {
  const slash = file.lastIndexOf('/');
  const base = slash === -1 ? file : file.slice(slash + 1);
  const directory = slash === -1 ? '' : file.slice(0, slash);

  return (
    <li>
      <Button
        className="app-review__file"
        aria-label={file}
        aria-current={active ? 'true' : undefined}
        onClick={onSelect}
      >
        <span className="app-review__file-row">
          <span className="app-review__file-base">{base}</span>
          <span className="app-review__file-counts">
            <span className="app-review__count app-review__count--added">+{additions}</span>
            <span className="app-review__count app-review__count--removed">-{deletions}</span>
          </span>
        </span>
        <span className="app-review__file-row">
          <span className="app-review__file-dir">{directory || S.review.repositoryRoot}</span>
          <Chip
            value={status}
            appearance={STATUS_APPEARANCE[status] ?? 'information'}
            isReadOnly
            isDense
          />
        </span>
      </Button>
    </li>
  );
}

export type { TreeDiff };