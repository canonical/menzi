/**
 * A unified diff parsed into renderable rows.
 *
 * opencode returns diffs as a `patch` string (the same format `git diff`
 * produces), so the viewer parses that rather than a pre-split structure.
 */
export type DiffLineKind = 'add' | 'context' | 'meta' | 'remove';

export interface DiffLine {
  kind: DiffLineKind;
  text: string;
  /** Line number on the left hand side, or null for added lines. */
  oldLine: number | null;
  /** Line number on the right hand side, or null for removed lines. */
  newLine: number | null;
}

export interface DiffHunk {
  header: string;
  /** The function or section name git recorded after the hunk header. */
  section: string;
  oldStart: number;
  newStart: number;
  lines: DiffLine[];
}

export interface ParsedPatch {
  hunks: DiffHunk[];
  additions: number;
  deletions: number;
}

const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?: (.*))?$/;

/** Line endings and a trailing newline never affect the parsed content. */
function toLines(patch: string): string[] {
  return patch.replace(/\r\n/g, '\n').replace(/\n$/, '').split('\n');
}

/**
 * Parses a unified diff. Anything that is not a hunk, a hunk body line or a
 * `\ No newline` marker is recorded as metadata so the viewer can skip it.
 */
export function parsePatch(patch: string): ParsedPatch {
  const hunks: DiffHunk[] = [];
  let additions = 0;
  let deletions = 0;
  let hunk: DiffHunk | null = null;
  let oldLine = 0;
  let newLine = 0;

  for (const raw of toLines(patch)) {
    const header = HUNK_HEADER.exec(raw);
    if (header) {
      oldLine = Number(header[1]);
      newLine = Number(header[3]);
      hunk = {
        header: raw,
        section: header[5] ?? '',
        oldStart: oldLine,
        newStart: newLine,
        lines: [],
      };
      hunks.push(hunk);
      continue;
    }

    // A `@@` header always starts a hunk, so anything else before one is
    // metadata: `diff --git`, `index`, `new file mode`, `--- a/x`, and so on.
    if (!hunk) continue;

    if (raw.startsWith('+')) {
      hunk.lines.push({ kind: 'add', text: raw.slice(1), oldLine: null, newLine });
      newLine += 1;
      additions += 1;
      continue;
    }

    if (raw.startsWith('-')) {
      hunk.lines.push({ kind: 'remove', text: raw.slice(1), oldLine, newLine: null });
      oldLine += 1;
      deletions += 1;
      continue;
    }

    if (raw.startsWith(' ')) {
      hunk.lines.push({ kind: 'context', text: raw.slice(1), oldLine, newLine });
      oldLine += 1;
      newLine += 1;
      continue;
    }

    // An empty line inside a hunk is an empty context line: some producers drop
    // the leading space of a blank line.
    if (raw === '') {
      hunk.lines.push({ kind: 'context', text: '', oldLine, newLine });
      oldLine += 1;
      newLine += 1;
      continue;
    }

    hunk.lines.push({ kind: 'meta', text: raw, oldLine: null, newLine: null });
  }

  return { hunks, additions, deletions };
}
