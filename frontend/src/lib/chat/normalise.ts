import type {
  CompactionPart,
  MessagePart,
  ReasoningPart,
  TextPart,
  ToolPart,
  ToolState,
} from '../types';

const PENDING: ToolState = { status: 'pending', input: {} };

export function isToolPart(part: MessagePart): part is ToolPart {
  return part.type === 'tool';
}

export function isTextPart(part: MessagePart): part is TextPart {
  return part.type === 'text';
}

export function isReasoningPart(part: MessagePart): part is ReasoningPart {
  return part.type === 'reasoning';
}

export function isCompactionPart(part: MessagePart): part is CompactionPart {
  return part.type === 'compaction';
}

export function isStepPart(part: MessagePart): boolean {
  return part.type === 'step-start' || part.type === 'step-finish';
}

export function toolNameOf(part: ToolPart): string {
  return part.tool && part.tool.length > 0 ? part.tool : 'tool';
}

export function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function asText(value: unknown): string {
  if (typeof value === 'string') return value;
  return '';
}

function errorText(value: unknown): string {
  if (typeof value === 'string') return value;
  const record = asRecord(value);
  if (typeof record.message === 'string') return record.message;
  const data = asRecord(record.data);
  return typeof data.message === 'string' ? data.message : '';
}

export function normaliseToolState(part: ToolPart): ToolState {
  const record = asRecord(part.state);
  const input = asRecord(record.input);
  const title = asText(record.title) || undefined;
  const metadata = asRecord(record.metadata);
  const time = asRecord(record.time);
  const clock =
    typeof time.start === 'number' || typeof time.end === 'number'
      ? {
          start: typeof time.start === 'number' ? time.start : undefined,
          end: typeof time.end === 'number' ? time.end : undefined,
        }
      : undefined;
  const meta = Object.keys(metadata).length > 0 ? metadata : undefined;

  switch (record.status) {
    case 'running':
      return { status: 'running', input, title };
    case 'completed':
      return { status: 'completed', input, output: asText(record.output), title, metadata: meta, time: clock };
    case 'error':
      return { status: 'error', input, error: errorText(record.error), title, metadata: meta, time: clock };
    case 'pending':
      return { status: 'pending', input, ...(title ? { title } : {}) };
    default:
      return PENDING;
  }
}

export function isActive(state: ToolState): boolean {
  return state.status === 'running' || state.status === 'pending';
}

export function hasDetail(state: ToolState): boolean {
  if (Object.keys(state.input).length > 0) return true;
  if (state.status === 'completed') return state.output.length > 0;
  if (state.status === 'error') return state.error.length > 0;
  return false;
}

export function describeResult(state: ToolState): string {
  if (state.status === 'error') return state.error;
  if (state.status === 'completed') return state.output;
  return '';
}

export function toolInputText(state: ToolState): string {
  if (Object.keys(state.input).length === 0) return '';
  return JSON.stringify(state.input, null, 2);
}
