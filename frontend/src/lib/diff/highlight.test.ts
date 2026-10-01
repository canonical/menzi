import { describe, expect, it } from 'vitest';
import {
  escapeForTest,
  highlight,
  languageFor,
  primeHighlighter,
  splitHighlightedLines,
  truncateLine,
} from './highlight';

describe('languageFor', () => {
  it('reads the extension of the last segment', () => {
    expect(languageFor('src/main.rs')).toBe('rs');
    expect(languageFor('web-ui/src/App.vue')).toBe('vue');
    expect(languageFor('a/b/c/Component.tsx')).toBe('tsx');
  });

  it('lowercases the extension', () => {
    expect(languageFor('README.MD')).toBe('md');
  });

  it('treats a dotfile as a shell script', () => {
    expect(languageFor('.bashrc')).toBe('bash');
  });

  it('has nothing to say about an extensionless file', () => {
    expect(languageFor('Makefile')).toBeNull();
    expect(languageFor('LICENSE')).toBeNull();
  });

  it('has nothing to say about a bare extension', () => {
    expect(languageFor('.')).toBeNull();
  });
});

describe('highlight', () => {
  it('escapes markup when there is no grammar', () => {
    expect(highlight('<script>', null)).toBe('&lt;script&gt;');
  });

  it('escapes markup when the grammar refuses the input', () => {
    primeHighlighter({
      highlight: () => {
        throw new Error('unknown language');
      },
    });
    expect(highlight('<b>', 'not-a-language')).toBe('&lt;b&gt;');
  });

  it('escapes a fragment larger than the tokenising budget', () => {
    const huge = 'x'.repeat(200_001);
    expect(highlight(huge, 'rs')).toBe(huge);
  });

  it('colours through the loaded grammar', async () => {
    const module = await import('highlight.js');
    primeHighlighter((module.default ?? module) as never);
    const out = highlight('let x = 1;', 'rs');
    expect(out).toContain('hljs-');
    expect(out).toContain('let');
  });
});

describe('splitHighlightedLines', () => {
  it('keeps plain text on one line', () => {
    expect(splitHighlightedLines('one')).toEqual(['one']);
  });

  it('splits on a newline', () => {
    expect(splitHighlightedLines('a\nb\nc')).toEqual(['a', 'b', 'c']);
  });

  it('closes a span that is open at the newline and reopens it after', () => {
    expect(
      splitHighlightedLines('<span class="hljs-string">let\nx</span>'),
    ).toEqual([
      '<span class="hljs-string">let</span>',
      '<span class="hljs-string">x</span>',
    ]);
  });

  it('reopens nested spans in order', () => {
    const lines = splitHighlightedLines('<span class="a"><span class="b">x\ny</span></span>');
    expect(lines[1]).toBe('<span class="a"><span class="b">y</span></span>');
  });

  it('leaves a self closing tag alone', () => {
    expect(splitHighlightedLines('<span class="a"/>x\ny')).toEqual([
      '<span class="a"/>x',
      'y',
    ]);
  });

  it('round trips a real highlighted fragment', async () => {
    const module = await import('highlight.js');
    const hljs = (module.default ?? module) as never as {
      highlight: (code: string, options: { language: string }) => { value: string };
    };
    primeHighlighter(hljs);
    const code = 'fn main() {\n    let s = "hi";\n}';
    const lines = splitHighlightedLines(highlight(code, 'rs'));
    expect(lines).toHaveLength(3);
    const text = lines
      .join('\n')
      .replace(/<\/?span[^>]*>/g, '')
      .replace(/&quot;/g, '"')
      .replace(/&lt;/g, '<')
      .replace(/&gt;/g, '>')
      .replace(/&amp;/g, '&');
    expect(text).toBe(code);
  });
});

describe('truncateLine', () => {
  it('leaves a short line alone', () => {
    expect(truncateLine('short')).toBe('short');
  });

  it('caps a very long line', () => {
    const out = truncateLine('y'.repeat(5_000));
    expect(out).toHaveLength(2_001);
    expect(out.endsWith('…')).toBe(true);
  });
});

describe('escapeForTest', () => {
  it('is the same escaping the viewer falls back to', () => {
    expect(escapeForTest('a & b < c')).toBe('a &amp; b &lt; c');
  });
});