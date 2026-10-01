import { useEffect, useMemo, useState } from 'react';
import { parsePatch } from '../../lib/diff/patch';
import {
  highlight,
  languageFor,
  loadHighlighter,
  splitHighlightedLines,
  truncateLine,
} from '../../lib/diff/highlight';
import { sideBySide, type DiffCell } from '../../lib/diff/side';
import { S } from '../../strings/catalogue';

const KIND_CLASS: Record<DiffCell['kind'], string> = {
  add: 'app-diff__cell--added',
  context: 'app-diff__cell--context',
  empty: 'app-diff__cell--empty',
  meta: 'app-diff__cell--meta',
  remove: 'app-diff__cell--removed',
};

function cellClass(value: DiffCell, side: 'left' | 'right'): string {
  return `app-diff__cell ${KIND_CLASS[value.kind]} app-diff__cell--${side}`;
}

/**
 * A read-only side-by-side diff: the removed side on the left, the added side on
 * the right, each with its own line numbers, so a reader can follow one change
 * horizontally instead of reading past a block of unrelated lines.
 *
 * Highlighting runs over each side as a whole and is then split back onto the
 * rows, because a grammar has to see consecutive lines to colour a block comment
 * or a multi-line string correctly.
 */
export function DiffViewer({ file, patch }: { file: string; patch: string }) {
  const parsed = useMemo(() => parsePatch(patch), [patch]);
  const language = useMemo(() => languageFor(file), [file]);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let live = true;
    void loadHighlighter().then(() => {
      if (live) setReady(true);
    });
    return () => {
      live = false;
    };
  }, []);

  const sides = useMemo(() => sideBySide(parsed), [parsed]);

  const rendered = useMemo(() => {
    if (!ready) return sides.map((side) => ({ header: side.header, rows: side.rows }));
    return sides.map((side) => {
      const left = side.rows.map((row) => row.left.text).join('\n');
      const right = side.rows.map((row) => row.right.text).join('\n');
      const leftMarkup = splitHighlightedLines(highlight(left, language));
      const rightMarkup = splitHighlightedLines(highlight(right, language));

      return {
        header: side.header,
        rows: side.rows.map((row, index) => ({
          left: { ...row.left, markup: leftMarkup[index] ?? escapeText(row.left.text) },
          right: { ...row.right, markup: rightMarkup[index] ?? escapeText(row.right.text) },
        })),
      };
    });
  }, [sides, ready, language]);

  if (parsed.hunks.length === 0) {
    return <p className="u-no-margin--bottom">{S.review.noPatch}</p>;
  }

  return (
    <div className="app-diff" data-testid="diff-viewer" aria-label={file}>
      {rendered.map((side, sideIndex) => (
        <div key={`${side.header}-${sideIndex}`} className="app-diff__hunk">
          <div className="app-diff__hunk-header">{side.header}</div>
          {side.rows.map((row, rowIndex) => (
            <div key={rowIndex} className="app-diff__row">
              <SideCell value={row.left} side="left" gutter={row.left.oldLine} ready={ready} />
              <SideCell value={row.right} side="right" gutter={row.right.newLine} ready={ready} />
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}

function SideCell({
  value,
  side,
  gutter,
  ready,
}: {
  value: DiffCell & { markup?: string };
  side: 'left' | 'right';
  gutter: number | null;
  ready: boolean;
}) {
  const mark = value.kind === 'add' ? '+' : value.kind === 'remove' ? '-' : '';
  return (
    <div className={cellClass(value, side)} data-side={side} data-kind={value.kind}>
      <span className="app-diff__gutter">{value.kind === 'empty' ? '' : (gutter ?? '')}</span>
      <span className="app-diff__mark" aria-hidden="true">
        {mark}
      </span>
      <span className="app-diff__content">
        {ready && value.markup !== undefined ? (
          <span dangerouslySetInnerHTML={{ __html: truncateLine(value.markup) }} />
        ) : (
          value.text
        )}
      </span>
    </div>
  );
}

function escapeText(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}