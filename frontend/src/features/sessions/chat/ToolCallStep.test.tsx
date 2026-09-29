import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { ToolCallStep } from './ToolCallStep';
import { groupableRuns } from '../../../lib/chat/groupRuns';
import { ToolCallGroup } from './ToolCallGroup';
import { ThinkingBlock, CompactionMarker } from './ThinkingBlock';
import { axeOptions } from '../../../testing/axe';
import type { ToolPart } from '../../../lib/types';

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

describe('ToolCallStep', () => {
  it('renders a prose label for a running tool', () => {
    render(<ToolCallStep part={running} />);
    expect(screen.getByText('Reading main.rs')).toBeInTheDocument();
  });

  it('describes a real bash call from the opencode payload', () => {
    render(<ToolCallStep part={bashCompleted} />);
    expect(screen.getByText('Running "echo menzi-tool-probe"')).toBeInTheDocument();
  });

  it('is not interactive when there is nothing to reveal', () => {
    render(<ToolCallStep part={idle} />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
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
    expect(screen.getByText(/src\/main\.rs/)).toBeInTheDocument();
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

  it('keeps a failed step in place and shows the error', async () => {
    const user = userEvent.setup();
    render(<ToolCallStep part={failed} />);
    expect(screen.getByText('Running "cargo test"')).toBeInTheDocument();
    await user.click(screen.getByRole('button'));
    expect(screen.getByText('command not found')).toBeInTheDocument();
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
        <ThinkingBlock text="considering options" />
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
    render(<ThinkingBlock text="internal reasoning" />);
    const details = screen.getByText('Thinking').closest('details');
    expect(details?.hasAttribute('open')).toBe(false);
  });

  it('renders nothing for empty reasoning', () => {
    const { container } = render(<ThinkingBlock text="   " />);
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
