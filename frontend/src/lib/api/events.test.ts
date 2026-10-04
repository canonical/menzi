import { describe, expect, it } from 'vitest';
import { toSessionEvent } from './events';

describe('toSessionEvent', () => {
  it('maps tool lifecycle events to tool parts', () => {
    const called = toSessionEvent({
      type: 'session.tool.called',
      data: {
        sessionID: 'ses_1',
        assistantMessageID: 'msg_1',
        id: 'call_1',
        name: 'shell',
        input: { command: 'pwd' },
      },
    });

    const completed = toSessionEvent({
      type: 'session.tool.success',
      data: {
        sessionID: 'ses_1',
        assistantMessageID: 'msg_1',
        id: 'call_1',
        content: [{ type: 'text', text: '/workspace' }],
      },
    });

    expect(called?.part).toMatchObject({ type: 'tool', tool: 'shell' });
    expect(completed?.part).toMatchObject({ type: 'tool', state: { status: 'completed' } });
  });

  it('maps reasoning started and ended with timing', () => {
    const started = toSessionEvent({
      type: 'session.reasoning.started',
      created: 100,
      data: { sessionID: 'ses_1', assistantMessageID: 'msg_1', ordinal: 0 },
    });
    const ended = toSessionEvent({
      type: 'session.reasoning.ended',
      created: 300,
      data: { sessionID: 'ses_1', assistantMessageID: 'msg_1', ordinal: 0, text: 'done' },
    });

    expect(started?.part).toMatchObject({ type: 'reasoning', time: { start: 100 } });
    expect(ended?.part).toMatchObject({ type: 'reasoning', time: { end: 300 } });
  });
});
