import { S } from '../../strings/catalogue';

export function blockReason(reason: string | null): string | null {
  if (!reason) return null;
  const reasons = S.workspace.reasons as Record<string, string>;
  return reasons[reason] ?? null;
}
