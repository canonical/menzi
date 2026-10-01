import { describe, expect, it } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { DiffViewer } from './DiffViewer';
import { highlight, languageFor, primeHighlighter } from '../../lib/diff/highlight';

async function withGrammar() {
  const module = await import('highlight.js');
  primeHighlighter((module.default ?? module) as never);
}

const SAMPLE = 'let x = 1; /* c */ "s"';

const UNSUPPORTED = ['vue', 'svelte', 'zig', 'tf', 'hcl', 'promql', 'jl'];

const MARKDOWN_LIKE = [
  'xml', 'asciidoc', 'bnf', 'csp', 'diff', 'django', 'dust', 'erb', 'freedesktop',
  'gherkin', 'haml', 'handlebars', 'http', 'latex', 'ldif', 'leaf', 'mojolicious',
  'nestedtext', 'php-template', 'plaintext', 'roboconf', 'shell', 'subunit',
  'taggerscript', 'tap', 'twig', 'vbscript-html',
];

describe('language coverage', () => {
  it('tokenises every grammar the bundle ships', async () => {
    await withGrammar();
    const module = await import('highlight.js');
    const hljs = (module.default ?? module) as never as { listLanguages(): string[] };

    const skip = new Set([...UNSUPPORTED, ...MARKDOWN_LIKE]);
    const failures: string[] = [];

    for (const language of hljs.listLanguages()) {
      if (skip.has(language) || language.includes('-repl')) continue;
      if (!highlight(SAMPLE, language).includes('hljs-')) failures.push(language);
    }

    expect(failures).toEqual([]);
  });

  it('leaves a markup or prose grammar unstyled rather than miscolouring it', async () => {
    await withGrammar();
    const module = await import('highlight.js');
    const hljs = (module.default ?? module) as never as { listLanguages(): string[] };

    for (const language of MARKDOWN_LIKE) {
      expect(hljs.listLanguages()).toContain(language);
    }
    expect(highlight('<a href="x">y</a>', 'xml')).toContain('hljs-');
    expect(highlight(SAMPLE, 'xml')).not.toContain('hljs-');
  });

  it('ships a grammar for the languages agents write most', async () => {
    await withGrammar();
    const wanted = [
      'rust', 'typescript', 'tsx', 'javascript', 'jsx', 'python', 'go', 'java', 'c',
      'cpp', 'php', 'ruby', 'bash', 'sh', 'zsh', 'json', 'yaml', 'toml', 'ini', 'xml',
      'html', 'css', 'scss', 'less', 'sql', 'graphql', 'lua', 'perl', 'r', 'scala',
      'kotlin', 'swift', 'dart', 'elixir', 'erlang', 'haskell', 'clojure', 'ocaml',
      'fsharp', 'julia', 'nim', 'vim', 'dockerfile', 'makefile', 'markdown', 'diff',
    ];

    const missing = wanted.filter((language) => {
      const html = highlight(SAMPLE, language);
      return html.includes('&lt;') || html.includes('&amp;') && !html.includes('hljs-');
    });

    expect(missing).toEqual([]);
  });

  it('falls back to escaped text for an extension with no grammar', async () => {
    await withGrammar();
    expect(highlight('<template/>', 'vue')).toBe('&lt;template/&gt;');
    expect(highlight('x = 1', 'tf')).toBe('x = 1');
  });

  it('still resolves those extensions, so the file is named in the UI', () => {
    for (const extension of UNSUPPORTED) {
      expect(languageFor(`src/a.${extension}`)).toBe(extension);
    }
  });

  it('colours a multi line comment as one run across rows', async () => {
    await withGrammar();
    const patch = [
      'Index: /w/a.rs',
      '===',
      '@@ -1,4 +1,4 @@',
      ' fn f() {',
      '+    /* start',
      '+     * middle',
      '+     * end */',
      ' }',
    ].join('\n');

    render(<DiffViewer file="a.rs" patch={patch} />);

    await waitFor(
      () => expect(document.querySelector('.app-diff__content .hljs-comment')).not.toBeNull(),
      { timeout: 5_000 },
    );
  });
});