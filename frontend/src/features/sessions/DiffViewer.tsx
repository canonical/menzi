import { useEffect, useMemo, useRef, useState } from 'react';
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
export function DiffViewer({
  file,
  patch,
  mode = 'split',
  wrapLines = true,
}: {
  file: string;
  patch: string;
  mode?: 'auto' | 'unified' | 'split';
  wrapLines?: boolean;
}) {
  const parsed = useMemo(() => parsePatch(patch), [patch]);
  const language = useMemo(() => languageFor(file), [file]);
  const [ready, setReady] = useState(false);
  const viewerRef = useRef<HTMLDivElement>(null);
  const [autoMode, setAutoMode] = useState<'unified' | 'split'>('unified');
  const resolvedMode = mode === 'auto' ? autoMode : mode;

  useEffect(() => {
    if (mode !== 'auto') return;
    setAutoMode('unified');
    const viewer = viewerRef.current;
    if (!viewer || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver((entries) => {
      const entry = entries.find((value) => value.target === viewer);
      if (entry) setAutoMode(entry.contentRect.width >= 850 ? 'split' : 'unified');
    });
    observer.observe(viewer);
    return () => observer.disconnect();
  }, [mode, parsed.hunks.length]);

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

  const unified = useMemo(() => parsed.hunks.map((hunk, index) => {
    const rows = rendered[index].rows;
    const left = new Map<number | null, DiffCell & { markup?: string }>(
      rows.map((row) => [row.left.oldLine, row.left]),
    );
    const right = new Map<number | null, DiffCell & { markup?: string }>(
      rows.map((row) => [row.right.newLine, row.right]),
    );
    return {
      header: hunk.header,
      lines: hunk.lines.map((line) => ({
        ...line,
        markup: line.kind === 'remove'
          ? left.get(line.oldLine)?.markup
          : line.kind === 'meta'
            ? escapeText(line.text)
            : right.get(line.newLine)?.markup,
      })),
    };
  }), [parsed, rendered]);

  if (parsed.hunks.length === 0) {
    return <p className="u-no-margin--bottom">{S.review.noPatch}</p>;
  }

  return (
    <div
      ref={viewerRef}
      className={`app-diff app-diff--${resolvedMode}${wrapLines ? ' app-diff--wrap' : ''}`}
      data-testid="diff-viewer"
      aria-label={file}
    >
      <div className="app-diff__body">
        <div className="app-diff__columns">
          <span>{S.review.before}</span>
          <span>{S.review.after}</span>
        </div>
        {resolvedMode === 'split' ? rendered.map((side, sideIndex) => (
          <div key={`${side.header}-${sideIndex}`} className="app-diff__hunk">
            <div className="app-diff__hunk-header">{side.header}</div>
            {side.rows.map((row, rowIndex) => (
              <div key={rowIndex} className="app-diff__row">
                <SideCell value={row.left} side="left" gutter={row.left.oldLine} ready={ready} />
                <SideCell value={row.right} side="right" gutter={row.right.newLine} ready={ready} />
              </div>
            ))}
          </div>
        )) : unified.map((hunk, hunkIndex) => (
          <div key={`${hunk.header}-${hunkIndex}`} className="app-diff__hunk">
            <div className="app-diff__hunk-header">{hunk.header}</div>
            {hunk.lines.map((line, lineIndex) => (
              <div key={lineIndex} className="app-diff__row">
                <SideCell
                  value={line}
                  side={line.kind === 'remove' ? 'left' : 'right'}
                  gutter={line.oldLine}
                  newGutter={line.newLine}
                  ready={ready}
                />
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

function SideCell({
  value,
  side,
  gutter,
  newGutter,
  ready,
}: {
  value: DiffCell & { markup?: string };
  side: 'left' | 'right';
  gutter: number | null;
  newGutter?: number | null;
  ready: boolean;
}) {
  const mark = value.kind === 'add' ? '+' : value.kind === 'remove' ? '-' : '';
  return (
    <div className={cellClass(value, side)} data-side={side} data-kind={value.kind}>
      <span className="app-diff__gutter">{value.kind === 'empty' ? '' : (gutter ?? '')}</span>
      {newGutter !== undefined ? <span className="app-diff__gutter">{newGutter ?? ''}</span> : null}
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