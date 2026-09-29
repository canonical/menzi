import { describe, expect, it } from 'vitest';
import {
  describeResult,
  hasDetail,
  isReasoningPart,
  isStepPart,
  isTextPart,
  isToolPart,
  normaliseToolState,
  toolInputText,
  toolNameOf,
} from './normalise';
import type { ToolPart } from '../types';

const base: ToolPart = { type: 'tool', id: 't1', tool: 'read' };

describe('normaliseToolState', () => {
  it('normalises the real completed state captured from opencode', () => {
    const state = normaliseToolState({
      ...base,
      tool: 'bash',
      state: {
        status: 'completed',
        input: { command: 'echo hi' },
        output: 'hi\n',
        title: 'echo hi',
        metadata: { exit: 0, truncated: false },
        time: { start: 1, end: 2 },
      },
    });
    expect(state.status).toBe('completed');
    if (state.status !== 'completed') throw new Error('unreachable');
    expect(state.output).toBe('hi\n');
    expect(state.title).toBe('echo hi');
    expect(state.time).toEqual({ start: 1, end: 2 });
    expect(describeResult(state)).toBe('hi\n');
  });

  it('normalises a running state', () => {
    const state = normaliseToolState({
      ...base,
      state: { status: 'running', input: { filePath: 'a.rs' } },
    });
    expect(state).toEqual({ status: 'running', input: { filePath: 'a.rs' }, title: undefined });
  });

  it('normalises an error state from a string', () => {
    const state = normaliseToolState({
      ...base,
      state: { status: 'error', input: {}, error: 'file not found' },
    });
    expect(state.status).toBe('error');
    expect(describeResult(state)).toBe('file not found');
  });

  it('normalises an error state from a structured error', () => {
    const state = normaliseToolState({
      ...base,
      state: { status: 'error', input: {}, error: { message: 'boom' } as never },
    });
    expect(describeResult(state)).toBe('boom');
  });

  it('falls back to pending when state is missing or unknown', () => {
    expect(normaliseToolState(base)).toEqual({ status: 'pending', input: {} });
    expect(normaliseToolState({ ...base, state: { status: 'nope' } as never })).toEqual({
      status: 'pending',
      input: {},
    });
  });

  it('never leaks a non-object input', () => {
    const state = normaliseToolState({
      ...base,
      state: { status: 'running', input: 'raw {' as never },
    });
    expect(state.input).toEqual({});
  });

  it('drops an empty metadata bag', () => {
    const state = normaliseToolState({
      ...base,
      state: { status: 'completed', input: {}, output: 'x', metadata: {} },
    });
    if (state.status !== 'completed') throw new Error('unreachable');
    expect(state.metadata).toBeUndefined();
  });
});

describe('toolNameOf', () => {
  it('returns the tool field opencode sends', () => {
    expect(toolNameOf({ type: 'tool', id: 'a', tool: 'bash' })).toBe('bash');
  });

  it('falls back when the tool field is empty', () => {
    expect(toolNameOf({ type: 'tool', id: 'a', tool: '' })).toBe('tool');
  });
});

describe('hasDetail', () => {
  it('is false for a pending tool with no input', () => {
    expect(hasDetail({ status: 'pending', input: {} })).toBe(false);
  });

  it('is true for completed with output only', () => {
    expect(hasDetail({ status: 'completed', input: {}, output: 'ok' })).toBe(true);
  });

  it('is true for completed with input only', () => {
    expect(hasDetail({ status: 'completed', input: { a: 1 }, output: '' })).toBe(true);
  });

  it('is true for an error carrying a message', () => {
    expect(hasDetail({ status: 'error', input: {}, error: 'boom' })).toBe(true);
  });

  it('is false for a completed tool with neither input nor output', () => {
    expect(hasDetail({ status: 'completed', input: {}, output: '' })).toBe(false);
  });
});

describe('toolInputText', () => {
  it('is empty when there is no input', () => {
    expect(toolInputText({ status: 'running', input: {} })).toBe('');
  });

  it('pretty prints object input', () => {
    expect(toolInputText({ status: 'running', input: { a: 1 } })).toContain('"a": 1');
  });
});

describe('part guards', () => {
  it('separates tool, text, reasoning and step parts', () => {
    expect(isToolPart({ type: 'tool', id: 'a', tool: 'b' })).toBe(true);
    expect(isTextPart({ type: 'text', text: 'x' })).toBe(true);
    expect(isReasoningPart({ type: 'reasoning', text: 'x' })).toBe(true);
    expect(isStepPart({ type: 'step-start', id: 's' })).toBe(true);
    expect(isStepPart({ type: 'step-finish', id: 's' })).toBe(true);
    expect(isToolPart({ type: 'text', text: 'x' })).toBe(false);
    expect(isStepPart({ type: 'text', text: 'x' })).toBe(false);
  });
});
