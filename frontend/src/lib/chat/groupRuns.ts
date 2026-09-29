import { toolNameOf } from './normalise';
import type { ToolPart } from '../types';

const MIN_GROUP_SIZE = 3;

export type ToolRun = { kind: 'group'; parts: ToolPart[] } | { kind: 'single'; part: ToolPart };

export function groupableRuns(parts: ToolPart[]): ToolRun[] {
  const runs: ToolRun[] = [];
  let current: ToolPart[] = [];

  const flush = () => {
    if (current.length === 0) return;
    if (current.length >= MIN_GROUP_SIZE) {
      runs.push({ kind: 'group', parts: [...current] });
    } else {
      for (const part of current) runs.push({ kind: 'single', part });
    }
    current = [];
  };

  for (const part of parts) {
    if (current.length > 0 && toolNameOf(current[0]) !== toolNameOf(part)) flush();
    current.push(part);
  }
  flush();
  return runs;
}
