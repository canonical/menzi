import type { OpencodeMessage } from '../types';

export function isWorking(messages: readonly OpencodeMessage[]): boolean {
  const last = lastAssistant(messages);
  if (!last) return false;
  if (hasFinish(last.info.finish)) return false;

  const parts = last.parts ?? [];
  if (!parts.some((part) => part.type === 'tool')) return true;

  return parts.some((part) => part.type === 'tool' && !settled(part));
}

function lastAssistant(
  messages: readonly OpencodeMessage[],
): OpencodeMessage | undefined {
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index].info.role === 'assistant') return messages[index];
  }
  return undefined;
}

function hasFinish(finish: string | undefined): boolean {
  return typeof finish === 'string' && finish.length > 0;
}

function settled(part: OpencodeMessage['parts'][number]): boolean {
  if (part.type !== 'tool') return true;
  const status = part.state?.status;
  return status === 'completed' || status === 'error';
}