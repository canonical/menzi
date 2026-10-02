import { useEffect, useId, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button, Select, Spinner } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { workspaceFilePatch, workspaceTreeChanges, type TreeDiff } from '../../lib/api/workspaces';
import { getErrorMessage, isApiError } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { DiffViewer } from './DiffViewer';
import { S } from '../../strings/catalogue';

import { ReviewFileTree } from './ReviewFileTree';

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
  const [mode, setMode] = useState<'auto' | 'unified' | 'split'>('auto');
  const modeId = useId();

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
  const version = listQuery.data?.version ?? '';

  const patchQuery = useQuery({
    queryKey: queryKeys.workspaces.diffPatch(
      userId ?? '',
      projectId ?? '',
      directory ?? '',
      active?.file ?? '',
      version,
    ),
    queryFn: () =>
      workspaceFilePatch(
        userId as string,
        projectId as string,
        directory,
        active?.file ?? '',
        version || undefined,
      ),
    enabled: enabled && !!active,
    retry: false,
    refetchInterval: 15000,
  });

  useEffect(() => {
    if (!isApiError(patchQuery.error) || patchQuery.error.status !== 409) return;
    void listQuery.refetch();
  }, [patchQuery.error, listQuery]);

  const change = patchQuery.data?.changes.find((entry) => entry.file === active?.file) ?? active;
  const patch = change?.patch ?? '';
  const activeIndex = changes.findIndex((entry) => entry.file === active?.file);
  const totals = changes.reduce(
    (sum, entry) => ({ additions: sum.additions + entry.additions, deletions: sum.deletions + entry.deletions }),
    { additions: 0, deletions: 0 },
  );

  return (
    <section className="app-review" aria-label={S.review.title}>
      <div className="app-review__toolbar">
        <span>{S.review.filesCount.replace('{count}', String(changes.length))}</span>
        <span className="app-review__file-counts">
          <span className="app-review__count--added">+{totals.additions}</span>
          <span className="app-review__count--removed">-{totals.deletions}</span>
        </span>
      </div>

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
          <ReviewFileTree
            files={changes.map((entry) => entry.file)}
            selected={active?.file ?? null}
            onSelect={setSelected}
          />

          <div className="app-review__viewer">
            <div className="app-review__viewer-header">
              <div className="app-review__file-heading">
                <h3 className="p-heading--5 u-no-margin--bottom">{active?.file}</h3>
                <span className="app-review__file-counts">
                  <span className="app-review__count--added">+{change?.additions ?? 0}</span>
                  <span className="app-review__count--removed">-{change?.deletions ?? 0}</span>
                </span>
              </div>
              <div className="app-review__controls">
                <Button
                  appearance="base"
                  className="u-no-margin--bottom"
                  disabled={activeIndex <= 0}
                  onClick={() => setSelected(changes[activeIndex - 1].file)}
                >
                  {S.review.previousFile}
                </Button>
                <span aria-live="polite">
                  {S.review.filePosition.replace('{current}', String(activeIndex + 1)).replace('{total}', String(changes.length))}
                </span>
                <Button
                  appearance="base"
                  className="u-no-margin--bottom"
                  disabled={activeIndex >= changes.length - 1}
                  onClick={() => setSelected(changes[activeIndex + 1].file)}
                >
                  {S.review.nextFile}
                </Button>
                <Select
                  id={modeId}
                  label={S.review.viewMode}
                  labelClassName="u-off-screen"
                  wrapperClassName="app-review__mode"
                  value={mode}
                  onChange={(event) => setMode(event.target.value as typeof mode)}
                  options={[
                    { value: 'auto', label: S.review.autoView },
                    { value: 'unified', label: S.review.unifiedView },
                    { value: 'split', label: S.review.splitView },
                  ]}
                />
              </div>
            </div>
            <div className="app-review__content" key={active?.file}>
              {patchQuery.isLoading && !patch ? (
                <div role="status"><Spinner text={S.review.patchLoading} /></div>
              ) : patchQuery.error ? (
                <div role="alert">
                  <p>{getErrorMessage(patchQuery.error)}</p>
                  <Button onClick={() => patchQuery.refetch()}>{S.dataState.retry}</Button>
                </div>
              ) : change?.binary ? (
                <p>{S.review.binaryFile}</p>
              ) : change && patch ? (
                <>
                  {change.truncated ? <p className="p-form-validation__message">{S.review.patchTruncated}</p> : null}
                  <DiffViewer file={change.file} patch={patch} mode={mode} wrapLines />
                </>
              ) : (
                <p>{S.review.noPatch}</p>
              )}
            </div>
          </div>
        </div>
      </DataState>
    </section>
  );
}

export type { TreeDiff };
