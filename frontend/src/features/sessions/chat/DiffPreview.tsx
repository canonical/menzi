import { parsePatch } from '../../../lib/diff/patch';
import { highlight, languageFor, loadHighlighter, splitHighlightedLines } from '../../../lib/diff/highlight';
import { useEffect, useState } from 'react';

const MAX_ROWS = 200;

export function DiffPreview({ file, patch }: { file: string; patch: string }) {
  const [ready, setReady] = useState(false);
  const parsed = parsePatch(patch);
  const language = file ? languageFor(file) : null;
  const truncated = parsed.hunks.some((hunk) => hunk.lines.length > MAX_ROWS);

  useEffect(() => {
    let live = true;
    void loadHighlighter().then(() => {
      if (live) setReady(true);
    });
    return () => {
      live = false;
    };
  }, []);

  const rows: { kind: string; text: string; markup: string }[] = [];
  for (const hunk of parsed.hunks) {
    for (const line of hunk.lines.slice(0, MAX_ROWS)) {
      rows.push({ kind: line.kind, text: line.text, markup: line.text });
    }
  }

  const source = rows.map((row) => row.text).join('\n');
  const rendered = ready
    ? splitHighlightedLines(highlight(source, language))
    : [];

  return (
    <div className="app-tv__diff" data-testid="tool-diff">
      {rows.map((row, index) => (
        <div className={`app-tv__diffline app-tv__diffline--${row.kind}`} key={index}>
          <span className="app-tv__diffmark" aria-hidden="true">
            {row.kind === 'add' ? '+' : row.kind === 'remove' ? '-' : ' '}
          </span>
          <span className="app-tv__difftext">
            {ready && rendered[index] !== undefined ? (
              <span dangerouslySetInnerHTML={{ __html: rendered[index] || ' ' }} />
            ) : (
              row.text || ' '
            )}
          </span>
        </div>
      ))}
      {truncated ? (
        <p className="app-tv__note">{`Only the first ${MAX_ROWS} lines of each hunk are shown.`}</p>
      ) : null}
      {rows.length === 0 ? <p className="app-tv__note">No textual changes.</p> : null}
    </div>
  );
}