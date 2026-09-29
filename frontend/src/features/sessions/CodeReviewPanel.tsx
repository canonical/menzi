import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Chip, MainTable } from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { getSessionDiff, getVcsStatus, type DiffLine } from '../../lib/api/opencode';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { S } from '../../strings/catalogue';

type Row = NonNullable<MainTableProps['rows']>[number];

const STATUS_APPEARANCE: Record<string, 'caution' | 'information' | 'negative' | 'positive'> =
  {
    added: 'positive',
    modified: 'caution',
    deleted: 'negative',
    renamed: 'information',
  };

function lineClass(line: DiffLine): string {
  if (line.kind === 'add') return 'app-diff__line app-diff__line--added';
  if (line.kind === 'remove') return 'app-diff__line app-diff__line--removed';
  return 'app-diff__line';
}

function linePrefix(line: DiffLine): string {
  if (line.kind === 'add') return '+';
  if (line.kind === 'remove') return '-';
  return ' ';
}

export function CodeReviewPanel({ sessionId }: { sessionId: string }) {
  const [selected, setSelected] = useState<string | null>(null);

  const vcsQuery = useQuery({
    queryKey: queryKeys.opencode.vcs(),
    queryFn: getVcsStatus,
    retry: false,
  });

  const diffQuery = useQuery({
    queryKey: queryKeys.opencode.diff(sessionId),
    queryFn: () => getSessionDiff(sessionId),
    retry: false,
  });

  const diffs = diffQuery.data ?? [];
  const active = diffs.find((diff) => diff.path === selected) ?? diffs[0] ?? null;

  const rows: Row[] = (vcsQuery.data ?? []).map((file) => ({
    columns: [
      {
        content: (
          <a
            href="#"
            onClick={(event) => {
              event.preventDefault();
              setSelected(file.file);
            }}
          >
            {file.file}
          </a>
        ),
      },
      {
        content: (
          <Chip
            value={file.status ?? S.review.unknown}
            appearance={STATUS_APPEARANCE[file.status ?? ''] ?? 'information'}
            isReadOnly
            isDense
          />
        ),
      },
      { content: `+${file.additions ?? 0}` },
      { content: `-${file.deletions ?? 0}` },
    ],
  }));

  const empty = !vcsQuery.isLoading && !vcsQuery.error && !(vcsQuery.data ?? []).length;

  return (
    <section className="app-section">
      <h2 className="app-panel-heading">{S.review.title}</h2>

      <DataState
        loading={vcsQuery.isLoading}
        error={vcsQuery.error ? getErrorMessage(vcsQuery.error) : null}
        empty={empty}
        emptyIcon="document"
        emptyTitle={S.review.emptyTitle}
        emptyBody={S.review.emptyBody}
        onRetry={() => vcsQuery.refetch()}
      >
        <MainTable
          headers={[
            { content: S.review.columns.file },
            { content: S.review.columns.status },
            { content: S.review.columns.additions },
            { content: S.review.columns.deletions },
          ]}
          rows={rows}
          responsive
        />
      </DataState>

      {active ? (
        <div>
          <div className="app-page-header">
            <h3 className="p-heading--5 u-no-margin--bottom">{active.path}</h3>
            <Chip
              value={`+${active.additions} -${active.deletions}`}
              appearance={active.additions > 0 ? 'positive' : 'information'}
              isReadOnly
              isDense
            />
          </div>
          {active.hunks.length === 0 ? (
            <p>{S.review.noHunks}</p>
          ) : (
            active.hunks.map((hunk, hunkIndex) => (
              <div key={`${active.path}-${hunkIndex}`} className="u-margin--bottom">
                <pre className="app-diff">
                  <span className="app-diff__header">{hunk.header}</span>
                  {hunk.lines.map((line, lineIndex) => (
                    <span key={lineIndex} className={lineClass(line)}>
                      {linePrefix(line)}
                      {line.text}
                      {'\n'}
                    </span>
                  ))}
                </pre>
              </div>
            ))
          )}
        </div>
      ) : null}

      {diffQuery.error ? (
        <p data-testid="review-diff-error">{getErrorMessage(diffQuery.error)}</p>
      ) : null}
    </section>
  );
}
