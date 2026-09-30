import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Button, Chip } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { getSessionDiff, listMessages } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { needsPatch, reviewDiffs, type ReviewDiff } from '../../lib/diff/session';
import { DiffViewer } from './DiffViewer';
import { S } from '../../strings/catalogue';

const STATUS_APPEARANCE: Record<string, 'caution' | 'information' | 'negative' | 'positive'> = {
  added: 'positive',
  modified: 'caution',
  deleted: 'negative',
  renamed: 'information',
};

export function CodeReviewPanel({ sessionId }: { sessionId: string }) {
  const [selected, setSelected] = useState<string | null>(null);

  // The same query the chat uses, so the panel adds no request of its own.
  const messagesQuery = useQuery({
    queryKey: queryKeys.opencode.messages(sessionId),
    queryFn: () => listMessages(sessionId),
    retry: false,
  });

  const diffs = reviewDiffs(messagesQuery.data?.messages ?? []);
  const active = diffs.find((diff) => diff.file === selected) ?? diffs[0] ?? null;
  const needsLoading = active ? needsPatch(active) : false;

  // A summary reports the counts but often not the patch, so fetch it for the
  // file on screen. Scoping the request to its message is the only way to get it.
  const patchQuery = useQuery({
    queryKey: queryKeys.opencode.diff(sessionId, active?.messageID),
    queryFn: () => getSessionDiff(sessionId, active?.messageID),
    enabled: needsLoading && !!active?.messageID,
    retry: false,
  });

  const patch =
    patchQuery.data?.find((entry) => entry.file === active?.file)?.patch ?? active?.patch ?? '';
  const loading =
    messagesQuery.isLoading || (needsLoading && patchQuery.isLoading && !patchQuery.error);

  return (
    <section className="app-review">
      <h2 className="app-panel-heading">{S.review.title}</h2>

      <DataState
        loading={loading}
        error={messagesQuery.error ? getErrorMessage(messagesQuery.error) : null}
        empty={!messagesQuery.isLoading && !messagesQuery.error && diffs.length === 0}
        emptyIcon="file-blank"
        emptyTitle={S.review.emptyTitle}
        emptyBody={S.review.emptyBody}
        onRetry={() => messagesQuery.refetch()}
      >
        <div className="app-review__panes">
          <ul className="app-review__files" aria-label={S.review.filesLabel}>
            {diffs.map((diff) => (
              <FileRow
                key={diff.file}
                diff={diff}
                active={diff.file === active?.file}
                onSelect={() => setSelected(diff.file)}
              />
            ))}
          </ul>

          <div className="app-review__viewer">
            {patchQuery.error ? (
              <p className="u-no-margin--bottom" data-testid="review-diff-error">
                {getErrorMessage(patchQuery.error)}
              </p>
            ) : active && patch ? (
              <>
                <div className="app-review__viewer-header">
                  <h3 className="p-heading--5 u-no-margin--bottom">{active.file}</h3>
                  <Chip
                    value={`+${active.additions} -${active.deletions}`}
                    appearance={active.additions > 0 ? 'positive' : 'information'}
                    isReadOnly
                    isDense
                  />
                </div>
                <DiffViewer file={active.file} patch={patch} />
              </>
            ) : (
              <p className="u-no-margin--bottom">{S.review.noPatch}</p>
            )}
          </div>
        </div>
      </DataState>
    </section>
  );
}

function FileRow({
  diff,
  active,
  onSelect,
}: {
  diff: ReviewDiff;
  active: boolean;
  onSelect: () => void;
}) {
  const slash = diff.file.lastIndexOf('/');
  const base = slash === -1 ? diff.file : diff.file.slice(slash + 1);
  const directory = slash === -1 ? '' : diff.file.slice(0, slash);

  return (
    <li>
      <Button
        className="app-review__file"
        // The row shows the file name split across two lines and truncated, so
        // name the button with the whole path.
        aria-label={diff.file}
        aria-current={active ? 'true' : undefined}
        onClick={onSelect}
      >
        <span className="app-review__file-row">
          <span className="app-review__file-base">{base}</span>
          <span className="app-review__file-counts">
            <span className="app-review__count app-review__count--added">+{diff.additions}</span>
            <span className="app-review__count app-review__count--removed">-{diff.deletions}</span>
          </span>
        </span>
        <span className="app-review__file-row">
          <span className="app-review__file-dir">{directory || S.review.repositoryRoot}</span>
          {diff.status ? (
            <Chip
              value={diff.status}
              appearance={STATUS_APPEARANCE[diff.status] ?? 'information'}
              isReadOnly
              isDense
            />
          ) : null}
        </span>
      </Button>
    </li>
  );
}
