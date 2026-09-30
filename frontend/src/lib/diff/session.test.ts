import { describe, expect, it } from 'vitest';
import { needsPatch, reviewDiffs } from './session';
import type { OpencodeMessage } from '../types';

const PATCH = '@@ -1,2 +1,2 @@\n a\n-b\n+c';

function user(id: string, diffs: unknown): OpencodeMessage {
  return {
    info: { id, sessionID: 'ses_1', role: 'user', summary: { diffs } as never },
    parts: [],
  };
}

function assistant(id: string, diffs: unknown): OpencodeMessage {
  return {
    info: { id, sessionID: 'ses_1', role: 'assistant', summary: { diffs } as never },
    parts: [],
  };
}

describe('reviewDiffs', () => {
  it('reads the file changes from a user message summary', () => {
    const diffs = reviewDiffs([
      user('msg_1', [
        { file: 'src/main.rs', additions: 2, deletions: 1, status: 'modified', patch: PATCH },
      ]),
    ]);

    expect(diffs).toEqual([
      {
        file: 'src/main.rs',
        additions: 2,
        deletions: 1,
        status: 'modified',
        patch: PATCH,
        messageID: 'msg_1',
      },
    ]);
  });

  it('ignores assistant messages', () => {
    expect(reviewDiffs([assistant('msg_1', [{ file: 'a.rs', additions: 1 }])])).toEqual([]);
  });

  it('ignores a message with no summary diffs', () => {
    const message: OpencodeMessage = {
      info: { id: 'msg_1', sessionID: 'ses_1', role: 'user' },
      parts: [],
    };
    expect(reviewDiffs([message])).toEqual([]);
  });

  it('lets a later message win for a file that was edited twice', () => {
    const diffs = reviewDiffs([
      user('msg_1', [{ file: 'a.rs', additions: 1, deletions: 0, patch: PATCH }]),
      user('msg_2', [{ file: 'a.rs', additions: 5, deletions: 2 }]),
    ]);

    expect(diffs).toHaveLength(1);
    expect(diffs[0]).toMatchObject({ additions: 5, deletions: 2, messageID: 'msg_2' });
  });

  it('keeps the order the files were first reported in', () => {
    const diffs = reviewDiffs([
      user('msg_1', [{ file: 'b.rs' }, { file: 'a.rs' }]),
      user('msg_2', [{ file: 'c.rs' }]),
    ]);
    expect(diffs.map((diff) => diff.file)).toEqual(['b.rs', 'a.rs', 'c.rs']);
  });

  it('skips entries with no file name', () => {
    expect(reviewDiffs([user('msg_1', [{ additions: 1 }, null, { file: '' }])])).toEqual([]);
  });
});

describe('needsPatch', () => {
  it('wants a patch when the summary did not carry one', () => {
    expect(needsPatch({ file: 'a.rs', additions: 2, deletions: 0, messageID: 'msg_1' })).toBe(true);
  });

  it('does not want a patch when one with a hunk is present', () => {
    expect(
      needsPatch({ file: 'a.rs', additions: 1, deletions: 1, patch: PATCH, messageID: 'msg_1' }),
    ).toBe(false);
  });

  it('wants a patch when the one it has has no hunk', () => {
    expect(
      needsPatch({
        file: 'a.rs',
        additions: 1,
        deletions: 0,
        patch: 'diff --git a/a.rs b/a.rs\n',
        messageID: 'msg_1',
      }),
    ).toBe(true);
  });

  it('does not ask for a patch when there is nothing changed', () => {
    expect(needsPatch({ file: 'a.rs', additions: 0, deletions: 0, messageID: 'msg_1' })).toBe(false);
  });
});
