const MAX_HIGHLIGHT_BYTES = 200_000;
const MAX_LINE_CHARS = 2_000;

type Highlighter = { highlight: (code: string, options: { language: string }) => { value: string } };

let pending: Promise<Highlighter | null> | null = null;
let loaded: Highlighter | null = null;
let ready: Highlighter | null = null;

async function highlighter(): Promise<Highlighter | null> {
  if (loaded) return loaded;
  pending ??= import('highlight.js').then((module) => {
    const resolved = (module.default ?? module) as Highlighter;
    loaded = resolved;
    ready = resolved;
    return resolved;
  });
  try {
    return await pending;
  } catch {
    return null;
  } finally {
    pending = null;
  }
}

/**
 * The file extension decides the language. A dotfile with no extension is common
 * enough in a diff that treating it as a shell script is the better guess.
 */
export function languageFor(file: string): string | null {
  const name = file.split('/').pop() ?? '';
  const dot = name.lastIndexOf('.');
  if (dot <= 0) {
    return name.startsWith('.') && name.length > 1 ? 'bash' : null;
  }
  return name.slice(dot + 1).toLowerCase() || null;
}

/**
 * The markup for one code fragment, or the fragment unchanged when there is no
 * language for it, when it is too large to be worth tokenising, or when the
 * grammar refuses it.
 */
export function highlight(code: string, language: string | null): string {
  if (!language || code.length > MAX_HIGHLIGHT_BYTES) return escapeHtml(code);
  const hljs = ready;
  if (!hljs) return escapeHtml(code);
  try {
    return hljs.highlight(code, { language }).value;
  } catch {
    return escapeHtml(code);
  }
}

export function primeHighlighter(result: Highlighter | null): void {
  ready = result;
}

export async function loadHighlighter(): Promise<void> {
  primeHighlighter(await highlighter());
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Splits a highlighted fragment back into one string per source line, so each
 * diff row can carry its own markup. Newlines only appear in text, so walking
 * the markup and closing the spans that are open at each one is enough.
 */
export function splitHighlightedLines(html: string): string[] {
  const lines: string[] = [];
  let current = '';
  const open: string[] = [];
  let index = 0;

  const closeAll = (): string => '</span>'.repeat(open.length);
  const reopenAll = (): string => open.map((tag) => `<${tag}>`).join('');

  while (index < html.length) {
    const char = html[index];

    if (char === '<') {
      const end = html.indexOf('>', index);
      if (end === -1) {
        current += html.slice(index);
        break;
      }
      const tag = html.slice(index, end + 1);
      if (tag.startsWith('</')) {
        open.pop();
        current += tag;
      } else if (!tag.endsWith('/>')) {
        open.push(tag.slice(1, -1));
        current += tag;
      } else {
        current += tag;
      }
      index = end + 1;
      continue;
    }

    if (char === '\n') {
      lines.push(`${current}${closeAll()}`);
      current = reopenAll();
      index += 1;
      continue;
    }

    current += char;
    index += 1;
  }

  lines.push(current);
  return lines;
}

export function truncateLine(value: string): string {
  return value.length > MAX_LINE_CHARS ? `${value.slice(0, MAX_LINE_CHARS)}…` : value;
}

export function escapeForTest(value: string): string {
  return escapeHtml(value);
}