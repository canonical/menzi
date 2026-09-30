import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { ToolCallStep } from './ToolCallStep';
import { groupableRuns } from '../../../lib/chat/groupRuns';
import { ToolCallGroup } from './ToolCallGroup';
import { ThinkingBlock, CompactionMarker } from './ThinkingBlock';
import { axeOptions } from '../../../testing/axe';
import type { ReasoningPart, ToolPart } from '../../../lib/types';

const running: ToolPart = {
  type: 'tool',
  id: 't1',
  tool: 'read',
  state: { status: 'running', input: { filePath: 'src/main.rs' } },
};

const completed: ToolPart = {
  type: 'tool',
  id: 't2',
  tool: 'read',
  state: {
    status: 'completed',
    input: { filePath: 'src/main.rs' },
    output: 'fn main() {}',
    title: 'src/main.rs',
  },
};

const bashCompleted: ToolPart = {
  type: 'tool',
  id: 't5',
  callID: 'call_function_1',
  tool: 'bash',
  state: {
    status: 'completed',
    input: { command: 'echo menzi-tool-probe' },
    output: 'menzi-tool-probe\n',
    title: 'echo menzi-tool-probe',
    metadata: { exit: 0, truncated: false },
    time: { start: 1, end: 2 },
  },
};

const failed: ToolPart = {
  type: 'tool',
  id: 't3',
  tool: 'bash',
  state: { status: 'error', input: { command: 'cargo test' }, error: 'command not found' },
};

const idle: ToolPart = { type: 'tool', id: 't4', tool: 'glob' };

function makeTool(id: string, tool: string, status: string): ToolPart {
  return {
    type: 'tool',
    id,
    tool,
    state:
      status === 'completed'
        ? { status: 'completed', input: {}, output: 'ok' }
        : { status: 'running', input: {} },
  };
}

function textOf(selector: string): string {
  const found = document.querySelector(selector);
  if (!found) throw new Error(`nothing matched ${selector}`);
  return found.textContent ?? '';
}

function detailOf(): HTMLElement {
  const detail = document.querySelector('.app-tc__detail');
  if (!detail) throw new Error('the detail panel is not open');
  return detail as HTMLElement;
}

describe('ToolCallStep', () => {
  it('renders a prose label for a running tool', () => {
    render(<ToolCallStep part={running} />);
    expect(textOf('.app-tc__label')).toBe('Reading: src/main.rs');
  });

  it('describes a real bash call from the opencode payload', () => {
    render(<ToolCallStep part={bashCompleted} />);
    expect(textOf('.app-tc__label')).toBe('Running: echo menzi-tool-probe');
  });

  it('puts the outcome on the same line as the call', () => {
    render(<ToolCallStep part={bashCompleted} />);
    expect(textOf('.app-tc__meta')).toBe('exit 0');
  });

  it('keeps the whole label reachable when it does not fit', () => {
    render(<ToolCallStep part={running} />);
    expect(screen.getByTitle('Reading: src/main.rs')).toBeInTheDocument();
  });

  it('is not interactive when there is nothing to reveal', () => {
    render(<ToolCallStep part={idle} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    expect(textOf('.app-tc__label')).toBe('Matching files');
  });

  it('exposes aria-expanded and reveals input and output when interactive', async () => {
    const user = userEvent.setup();
    render(<ToolCallStep part={completed} />);
    const button = screen.getByRole('button');
    expect(button).toHaveAttribute('aria-expanded', 'false');

    await user.click(button);

    expect(button).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByText('Input')).toBeInTheDocument();
    expect(screen.getByText('Result')).toBeInTheDocument();
    expect(within(detailOf()).getByText(/src\/main\.rs/)).toBeInTheDocument();
    expect(screen.getByText('fn main() {}')).toBeInTheDocument();
  });

  it('shows the tool name and the captured output for a bash call', async () => {
    const user = userEvent.setup();
    const { container } = render(<ToolCallStep part={bashCompleted} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByText('bash')).toBeInTheDocument();
    const output = Array.from(container.querySelectorAll('.app-tc__data'));
    expect(output[output.length - 1]?.textContent).toBe('menzi-tool-probe\n');
  });

  it('shows a failure on the line itself, before anything is opened', async () => {
    const user = userEvent.setup();
    const { container } = render(<ToolCallStep part={failed} />);
    expect(textOf('.app-tc__label')).toBe('Running: cargo test');
    expect(textOf('.app-tc__meta')).toBe('command not found');
    expect(container.querySelector('.app-tc__meta--error')).not.toBeNull();

    await user.click(screen.getByRole('button'));
    expect(within(detailOf()).getByText('command not found')).toBeInTheDocument();
  });

  it('truncates a long result and offers to expand it', async () => {
    const user = userEvent.setup();
    const long: ToolPart = {
      type: 'tool',
      id: 't6',
      tool: 'bash',
      state: { status: 'completed', input: {}, output: 'x'.repeat(1200) },
    };
    render(<ToolCallStep part={long} />);
    await user.click(screen.getByRole('button'));
    expect(screen.getByText('Show more')).toBeInTheDocument();
    await user.click(screen.getByText('Show more'));
    expect(screen.getByText('Show less')).toBeInTheDocument();
  });

  it('has no accessibility violations', async () => {
    const { container } = render(
      <div>
        <ToolCallStep part={running} />
        <ToolCallStep part={completed} />
        <ToolCallStep part={failed} />
        <ThinkingBlock
          part={{ type: 'reasoning', text: 'considering options', time: { start: 0, end: 1_000 } }}
        />
      </div>,
    );
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });
});

describe('ToolCallGroup', () => {
  it('groups only runs of three or more of the same tool', () => {
    const runs = groupableRuns([
      makeTool('a', 'read', 'completed'),
      makeTool('b', 'read', 'completed'),
      makeTool('c', 'read', 'completed'),
      makeTool('d', 'bash', 'completed'),
      makeTool('e', 'read', 'completed'),
    ]);
    expect(runs).toHaveLength(3);
    expect(runs[0].kind).toBe('group');
    expect(runs[1].kind).toBe('single');
    expect(runs[2].kind).toBe('single');
  });

  it('summarises the group', () => {
    render(
      <ToolCallGroup
        parts={[
          makeTool('a', 'read', 'completed'),
          makeTool('b', 'read', 'completed'),
          makeTool('c', 'read', 'completed'),
        ]}
      />,
    );
    expect(screen.getByText('Ran 3 file reads')).toBeInTheDocument();
  });

  it('opens automatically while a member is running', () => {
    render(
      <ToolCallGroup
        parts={[
          makeTool('a', 'read', 'running'),
          makeTool('b', 'read', 'completed'),
          makeTool('c', 'read', 'completed'),
        ]}
      />,
    );
    const details = screen.getByText('Ran 3 file reads').closest('details');
    expect(details?.hasAttribute('open')).toBe(true);
  });
});

describe('ThinkingBlock', () => {
  it('folds reasoning away by default', () => {
    render(<ThinkingBlock part={{ type: 'reasoning', text: 'internal reasoning' }} />);
    const details = screen.getByText('Thinking').closest('details');
    expect(details?.hasAttribute('open')).toBe(false);
  });

  it('reports how long the thought took instead of quoting it', () => {
    render(
      <ThinkingBlock
        part={{
          type: 'reasoning',
          text: 'I should check the routes first.',
          time: { start: 1_000, end: 6_400 },
        }}
      />,
    );
    expect(textOf('.app-thinking__label')).toBe('Thought for 5s');
    expect(textOf('.app-thinking__body')).toBe('I should check the routes first.');
  });

  it('keeps a decimal for a thought under a second', () => {
    render(
      <ThinkingBlock
        part={{ type: 'reasoning', text: 'quick', time: { start: 0, end: 189 } }}
      />,
    );
    expect(screen.getByText('Thought for 0.2s')).toBeInTheDocument();
  });

  it('still says it is thinking while the thought is unfinished', () => {
    render(
      <ThinkingBlock
        part={{ type: 'reasoning', text: 'half a thought', time: { start: 1_000 } }}
      />,
    );
    expect(screen.getByText('Thinking')).toBeInTheDocument();
  });

  it('falls back to thinking when the server sent no timing', () => {
    render(<ThinkingBlock part={{ type: 'reasoning', text: 'no timing here' }} />);
    expect(screen.getByText('Thinking')).toBeInTheDocument();
  });

  it('ignores a timing the server sent in the wrong shape', () => {
    const part = {
      type: 'reasoning',
      text: 'odd timing',
      time: { start: 0, end: '5000' },
    } as unknown as ReasoningPart;
    render(<ThinkingBlock part={part} />);
    expect(screen.getByText('Thinking')).toBeInTheDocument();
  });

  it('keeps the whole thought in the tooltip', () => {
    const text = 'I should check the routes first\nand then the guard.';
    render(<ThinkingBlock part={{ type: 'reasoning', text, time: { start: 0, end: 2_000 } }} />);
    expect(document.querySelector('.app-thinking__summary')?.getAttribute('title')).toBe(text);
  });

  it('renders nothing for empty reasoning', () => {
    const { container } = render(<ThinkingBlock part={{ type: 'reasoning', text: '   ' }} />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe('CompactionMarker', () => {
  it('distinguishes completed and failed compaction', () => {
    const { unmount } = render(<CompactionMarker part={{ type: 'compaction' }} />);
    expect(screen.getByText('Context compacted')).toBeInTheDocument();
    unmount();
    render(<CompactionMarker part={{ type: 'compaction', error: 'no' }} />);
    expect(screen.getByText('Context compaction failed')).toBeInTheDocument();
  });
});
