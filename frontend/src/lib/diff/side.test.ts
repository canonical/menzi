import { describe, expect, it } from 'vitest';
import { parsePatch } from './patch';
import { sideBySide } from './side';

function sides(patch: string) {
  return sideBySide(parsePatch(patch));
}

describe('sideBySide', () => {
  it('mirrors a context line onto both sides', () => {
    const [side] = sides('@@ -1,3 +1,3 @@\n keep\n-bad\n+good\n');

    expect(side.rows).toHaveLength(2);
    expect(side.rows[0].left).toMatchObject({ kind: 'context', text: 'keep', oldLine: 1, newLine: 1 });
    expect(side.rows[0].right.text).toBe('keep');
  });

  it('puts a removal on the left and its addition on the right', () => {
    const [side] = sides('@@ -1,2 +1,2 @@\n-bad\n+good\n');
    const row = side.rows[0];

    expect(row.left).toMatchObject({ kind: 'remove', text: 'bad', oldLine: 1, newLine: null });
    expect(row.right).toMatchObject({ kind: 'add', text: 'good', newLine: 1, oldLine: null });
  });

  it('pairs a run of changes index by index', () => {
    const [side] = sides('@@ -1,3 +1,3 @@\n-a\n-b\n-c\n+A\n+B\n+C\n');

    expect(side.rows).toHaveLength(3);
    expect(side.rows.map((row) => [row.left.text, row.right.text])).toEqual([
      ['a', 'A'],
      ['b', 'B'],
      ['c', 'C'],
    ]);
  });

  it('leaves the right side empty when there are more removals', () => {
    const [side] = sides('@@ -1,3 +1,2 @@\n-a\n-b\n-c\n+A\n');

    expect(side.rows).toHaveLength(3);
    expect(side.rows[2].left.text).toBe('c');
    expect(side.rows[2].right.kind).toBe('empty');
  });

  it('leaves the left side empty when there are more additions', () => {
    const [side] = sides('@@ -1,2 +1,3 @@\n-a\n+A\n+B\n+C\n');

    expect(side.rows).toHaveLength(3);
    expect(side.rows[0].left.text).toBe('a');
    expect(side.rows[1].left.kind).toBe('empty');
    expect(side.rows[1].right.text).toBe('B');
  });

  it('keeps the line numbers on the side they belong to', () => {
    const [side] = sides('@@ -10,3 +20,3 @@\n ctx\n-old\n+new\n');

    expect(side.rows[0].left.oldLine).toBe(10);
    expect(side.rows[0].left.newLine).toBe(20);
    expect(side.rows[1].left.oldLine).toBe(11);
    expect(side.rows[1].right.newLine).toBe(21);
  });

  it('keeps each hunk as its own block with its header', () => {
    const parsed = sides('@@ -1,1 +1,1 @@\n-a\n+A\n@@ -9,1 +9,1 @@\n-b\n+B\n');

    expect(parsed).toHaveLength(2);
    expect(parsed[0].header).toContain('-1,1');
    expect(parsed[1].header).toContain('-9,1');
  });

  it('shows metadata on both sides', () => {
    const [side] = sides('@@ -1,2 +1,2 @@\n ctx\n\\ No newline at end of file\n');

    expect(side.rows[1].left.kind).toBe('meta');
    expect(side.rows[1].right.kind).toBe('meta');
  });

  it('handles a pure addition block', () => {
    const [side] = sides('@@ -1,1 +1,3 @@\n ctx\n+one\n+two\n');

    expect(side.rows).toHaveLength(3);
    expect(side.rows[1].left.kind).toBe('empty');
    expect(side.rows[1].right.text).toBe('one');
  });

  it('handles a pure removal block', () => {
    const [side] = sides('@@ -1,3 +1,1 @@\n ctx\n-one\n-two\n');

    expect(side.rows).toHaveLength(3);
    expect(side.rows[1].left.text).toBe('one');
    expect(side.rows[1].right.kind).toBe('empty');
  });
});