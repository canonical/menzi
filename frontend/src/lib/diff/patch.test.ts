import { describe, expect, it } from 'vitest';
import { parsePatch } from './patch';

const SIMPLE = `diff --git a/src/main.rs b/src/main.rs
index 1234567..89abcde 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
-    old();
+    setup();
+    run();
 }
`;

describe('parsePatch', () => {
  it('reads the line numbers from the hunk header', () => {
    const { hunks } = parsePatch(SIMPLE);
    expect(hunks).toHaveLength(1);
    expect(hunks[0].oldStart).toBe(1);
    expect(hunks[0].newStart).toBe(1);
  });

  it('numbers both sides and offsets the right hand side by additions', () => {
    const { hunks } = parsePatch(SIMPLE);
    expect(hunks[0].lines).toEqual([
      { kind: 'context', text: 'fn main() {', oldLine: 1, newLine: 1 },
      { kind: 'remove', text: '    old();', oldLine: 2, newLine: null },
      { kind: 'add', text: '    setup();', oldLine: null, newLine: 2 },
      { kind: 'add', text: '    run();', oldLine: null, newLine: 3 },
      { kind: 'context', text: '}', oldLine: 3, newLine: 4 },
    ]);
  });

  it('counts the additions and deletions', () => {
    expect(parsePatch(SIMPLE)).toMatchObject({ additions: 2, deletions: 1 });
  });

  it('ignores everything before the first hunk', () => {
    const parsed = parsePatch(SIMPLE);
    expect(parsed.hunks[0].lines.some((line) => line.text.includes('index '))).toBe(false);
  });

  it('keeps the section name git records after the header', () => {
    const { hunks } = parsePatch('@@ -10,2 +10,2 @@ impl Server {\n-a\n+b\n');
    expect(hunks[0].section).toBe('impl Server {');
  });

  it('handles several hunks with independent numbering', () => {
    const parsed = parsePatch(
      ['@@ -1,2 +1,2 @@', ' a', '-b', '+c', '@@ -20,2 +21,3 @@', ' d', '+e', ' f'].join('\n'),
    );
    expect(parsed.hunks).toHaveLength(2);
    expect(parsed.hunks[1].lines[1]).toEqual({
      kind: 'add',
      text: 'e',
      oldLine: null,
      newLine: 22,
    });
    expect(parsed).toMatchObject({ additions: 2, deletions: 1 });
  });

  it('treats a blank line inside a hunk as an empty context line', () => {
    const { hunks } = parsePatch('@@ -1,3 +1,3 @@\n a\n\n-b');
    expect(hunks[0].lines[1]).toEqual({ kind: 'context', text: '', oldLine: 2, newLine: 2 });
  });

  it('keeps a no-newline marker as metadata rather than a change', () => {
    const parsed = parsePatch('@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file');
    expect(parsed).toMatchObject({ additions: 1, deletions: 1 });
    expect(parsed.hunks[0].lines.at(-1)).toMatchObject({ kind: 'meta' });
  });

  it('tolerates carriage returns and a missing trailing newline', () => {
    const parsed = parsePatch('@@ -1,2 +1,2 @@\r\n a\r\n-b\r\n+c');
    expect(parsed.hunks[0].lines.map((line) => line.kind)).toEqual(['context', 'remove', 'add']);
  });

  it('returns nothing for an empty patch', () => {
    expect(parsePatch('')).toEqual({ hunks: [], additions: 0, deletions: 0 });
  });
});
