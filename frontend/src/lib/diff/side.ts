import type { DiffLine, ParsedPatch } from './patch';

export interface DiffCell {
  kind: 'add' | 'context' | 'empty' | 'meta' | 'remove';
  text: string;
  oldLine: number | null;
  newLine: number | null;
}

export interface DiffRow {
  left: DiffCell;
  right: DiffCell;
}

export interface DiffSide {
  header: string;
  rows: DiffRow[];
}

const EMPTY: DiffCell = { kind: 'empty', text: '', oldLine: null, newLine: null };

function cell(line: DiffLine | null): DiffCell {
  if (!line) return EMPTY;
  return {
    kind: line.kind,
    text: line.text,
    oldLine: line.oldLine,
    newLine: line.newLine,
  };
}

/**
 * Lays a hunk out as two columns.
 *
 * A run of removals followed by a run of additions is one change, so the two
 * runs are zipped index by index: the first removal sits opposite the first
 * addition. When the runs are different lengths the longer one keeps its own
 * rows with an empty cell opposite, which is what a reader expects to see.
 */
function rowsFor(lines: readonly DiffLine[]): DiffRow[] {
  const rows: DiffRow[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index];

    if (line.kind === 'context' || line.kind === 'meta') {
      const same = { ...cell(line) };
      rows.push({ left: same, right: { ...same } });
      index += 1;
      continue;
    }

    const removed: DiffLine[] = [];
    const added: DiffLine[] = [];

    while (index < lines.length && lines[index].kind === 'remove') {
      removed.push(lines[index]);
      index += 1;
    }
    while (index < lines.length && lines[index].kind === 'add') {
      added.push(lines[index]);
      index += 1;
    }

    const height = Math.max(removed.length, added.length);
    for (let row = 0; row < height; row += 1) {
      rows.push({ left: cell(removed[row] ?? null), right: cell(added[row] ?? null) });
    }
  }

  return rows;
}

export function sideBySide(parsed: ParsedPatch): DiffSide[] {
  return parsed.hunks.map((hunk) => ({ header: hunk.header, rows: rowsFor(hunk.lines) }));
}

export function totals(parsed: ParsedPatch): { additions: number; deletions: number } {
  return { additions: parsed.additions, deletions: parsed.deletions };
}