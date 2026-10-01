import { describe, expect, it } from 'vitest';
import { isWorking } from './isWorking';
import type { OpencodeMessage, ToolPart } from '../types';

function user(text: string): OpencodeMessage {
  return {
    info: { id: 'msg_u', sessionID: 'ses_1', role: 'user' },
    parts: [{ type: 'text', text }],
  };
}

function assistant(over: Partial<OpencodeMessage['info']> = {}, parts: ToolPart[] = []): OpencodeMessage {
  return {
    info: { id: 'msg_a', sessionID: 'ses_1', role: 'assistant', ...over },
    parts,
  };
}

function tool(status: 'pending' | 'running' | 'completed' | 'error'): ToolPart {
  return {
    type: 'tool',
    id: 'prt_1',
    tool: 'bash',
    state:
      status === 'pending'
        ? { status: 'pending', input: {} }
        : status === 'running'
          ? { status: 'running', input: {} }
          : status === 'error'
            ? { status: 'error', input: {}, error: 'no' }
            : { status: 'completed', input: {}, output: 'ok' },
  };
}

describe('isWorking', () => {
  it('is false with no messages', () => {
    expect(isWorking([])).toBe(false);
  });

  it('is false when the only message is the prompt', () => {
    expect(isWorking([user('go')])).toBe(false);
  });

  it('is true while an assistant message has no finish', () => {
    expect(isWorking([user('go'), assistant()])).toBe(true);
  });

  it('is false once the turn reports a finish', () => {
    expect(isWorking([user('go'), assistant({ finish: 'stop' })])).toBe(false);
  });

  it('treats an empty finish as unfinished', () => {
    expect(isWorking([user('go'), assistant({ finish: '' })])).toBe(true);
  });

  it('is true while a tool is still running', () => {
    expect(isWorking([assistant({}, [tool('running')])])).toBe(true);
  });

  it('is true while a tool is pending', () => {
    expect(isWorking([assistant({}, [tool('pending')])])).toBe(true);
  });

  it('is false when every tool completed', () => {
    expect(isWorking([assistant({ finish: 'stop' }, [tool('completed')])])).toBe(false);
  });

  it('is false when a tool failed and the turn ended', () => {
    expect(isWorking([assistant({ finish: 'stop' }, [tool('error')])])).toBe(false);
  });

  it('looks at the last assistant message, not an earlier one', () => {
    const history = [assistant({ finish: 'stop' }), user('again'), assistant()];
    expect(isWorking(history)).toBe(true);
  });

  it('is false when the last assistant message finished even if an earlier one did not', () => {
    const history = [assistant(), assistant({ finish: 'stop' })];
    expect(isWorking(history)).toBe(false);
  });

  it('ignores non assistant trailing messages', () => {
    expect(isWorking([assistant({ finish: 'stop' }), user('next')])).toBe(false);
  });

  it('ignores text and reasoning parts', () => {
    const message: OpencodeMessage = {
      info: { id: 'msg_a', sessionID: 'ses_1', role: 'assistant' },
      parts: [
        { type: 'text', text: 'thinking out loud' },
        { type: 'reasoning', text: 'why' },
      ],
    };
    expect(isWorking([message])).toBe(true);
  });
});