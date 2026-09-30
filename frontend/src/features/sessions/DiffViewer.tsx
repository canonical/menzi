import { useMemo } from 'react';
import { parsePatch, type DiffLine } from '../../lib/diff/patch';
import { S } from '../../strings/catalogue';

const KIND_CLASS: Record<DiffLine['kind'], string> = {
  add: 'app-diff__line--added',
  remove: 'app-diff__line--removed',
  context: 'app-diff__line--context',
  meta: 'app-diff__line--meta',
};

const KIND_MARK: Record<DiffLine['kind'], string> = {
  add: '+',
  remove: '-',
  context: ' ',
  meta: '',
};

function lineClass(line: DiffLine): string {
  return `app-diff__line ${KIND_CLASS[line.kind]}`;
}

/**
 * A read-only unified diff: a line number gutter on both sides, then the
 * content. Removed and added lines share the same visual weight as a git diff
 * so the change reads at a glance.
 */
export function DiffViewer({ file, patch }: { file: string; patch: string }) {
  const parsed = useMemo(() => parsePatch(patch), [patch]);

  if (parsed.hunks.length === 0) {
    return <p className="u-no-margin--bottom">{S.review.noPatch}</p>;
  }

  return (
    <div className="app-diff" data-testid="diff-viewer" aria-label={file}>
      {parsed.hunks.map((hunk, hunkIndex) => (
        <div key={`${hunk.header}-${hunkIndex}`} className="app-diff__hunk">
          <div className="app-diff__hunk-header">{hunk.header}</div>
          {hunk.lines.map((line, lineIndex) => (
            <div key={lineIndex} className={lineClass(line)}>
              <span className="app-diff__gutter app-diff__gutter--old">
                {line.oldLine ?? ''}
              </span>
              <span className="app-diff__gutter app-diff__gutter--new">
                {line.newLine ?? ''}
              </span>
              <span className="app-diff__mark" aria-hidden="true">
                {KIND_MARK[line.kind]}
              </span>
              <span className="app-diff__content">{line.text}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
