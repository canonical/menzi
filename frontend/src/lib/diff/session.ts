import type { OpencodeMessage } from '../types';

/** A file change reported on a user message, or by the diff route. */
export interface ReviewDiff {
  file: string;
  additions: number;
  deletions: number;
  status?: string;
  /** Present when the producer already had the full patch. */
  patch?: string;
  /** The message this change came from, needed to load a missing patch. */
  messageID?: string;
}

const HUNK = /^@@ /m;

function number(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

function toDiff(raw: unknown, messageID?: string): ReviewDiff | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const entry = raw as Record<string, unknown>;
  const file = entry.file ?? entry.path;
  if (typeof file !== 'string' || !file) return null;
  return {
    file,
    additions: number(entry.additions),
    deletions: number(entry.deletions),
    status: typeof entry.status === 'string' ? entry.status : undefined,
    patch: typeof entry.patch === 'string' ? entry.patch : undefined,
    ...(messageID ? { messageID } : {}),
  };
}

/**
 * The file changes of a session, taken from the summary opencode attaches to
 * each user message. Later messages win, so a file edited twice shows its
 * current state. This is the only source that can carry a patch, which is why
 * the file list is built here rather than from the working tree.
 */
export function reviewDiffs(messages: readonly OpencodeMessage[]): ReviewDiff[] {
  const byFile = new Map<string, ReviewDiff>();

  for (const message of messages) {
    if (message.info.role !== 'user') continue;
    const diffs = message.info.summary?.diffs;
    if (!Array.isArray(diffs)) continue;
    for (const raw of diffs) {
      const diff = toDiff(raw, message.info.id);
      if (diff) byFile.set(diff.file, diff);
    }
  }

  return [...byFile.values()];
}

/**
 * A summary often reports the counts without the patch, so the patch has to be
 * fetched separately for that message.
 */
export function needsPatch(diff: ReviewDiff): boolean {
  if (diff.additions === 0 && diff.deletions === 0) return false;
  return !diff.patch || !HUNK.test(diff.patch);
}
