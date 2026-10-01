import { describe, expect, it } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ToolCallStep } from './ToolCallStep';
import type { ToolPart, ToolState } from '../../../lib/types';

function completed(tool: string, state: Partial<ToolState> & { input: Record<string, unknown> }): ToolPart {
  return {
    type: 'tool',
    id: 'p1',
    tool,
    state: { status: 'completed', output: '', ...state } as ToolState,
  };
}

async function open(part: ToolPart) {
  const user = userEvent.setup();
  render(<ToolCallStep part={part} />);
  await user.click(screen.getByRole('button'));
}

const DIFF = [
  'Index: /workspace/src/a.rs',
  '===...===',
  '--- /workspace/src/a.rs',
  '+++ /workspace/src/a.rs',
  '@@ -1,2 +1,2 @@',
  ' fn main() {',
  '-    old();',
  '+    new();',
  ' }',
].join('\n');

describe('tool detail views', () => {
  it('renders an edit as a diff rather than json', async () => {
    await open(
      completed('edit', {
        input: { filePath: '/workspace/src/a.rs', oldString: 'old();', newString: 'new();' },
        output: 'Edit applied successfully.',
        metadata: { diff: DIFF },
      }),
    );

    expect(screen.getByTestId('tool-diff')).toBeInTheDocument();
    expect(screen.getByText('Changes')).toBeInTheDocument();
    expect(document.body.textContent).not.toContain('"oldString"');
  });

  it('colours the diff once the grammar loads', async () => {
    await open(
      completed('edit', {
        input: { filePath: '/workspace/src/a.rs' },
        metadata: { diff: DIFF },
      }),
    );

    await waitFor(
      () => expect(document.querySelector('.app-tv__diff .hljs-keyword')).not.toBeNull(),
      { timeout: 5_000 },
    );
  });

  it('marks removals and additions in the diff', async () => {
    await open(
      completed('edit', { input: { filePath: '/workspace/src/a.rs' }, metadata: { diff: DIFF } }),
    );

    const diff = screen.getByTestId('tool-diff');
    expect(within(diff).getByText('-')).toBeInTheDocument();
    expect(diff.textContent).toContain('old();');
    expect(diff.textContent).toContain('new();');
    expect(diff.querySelector('.app-tv__diffline--add')).not.toBeNull();
    expect(diff.querySelector('.app-tv__diffline--remove')).not.toBeNull();
  });

  it('falls back to the replacement when there is no diff', async () => {
    await open(
      completed('edit', {
        input: { filePath: 'src/a.rs', oldString: 'let a = 1;', newString: 'let a = 2;' },
      }),
    );

    expect(screen.getByText('Replaced')).toBeInTheDocument();
    expect(screen.getByText('With')).toBeInTheDocument();
    expect(document.body.textContent).toContain('let a = 2;');
  });

  it('shows what a write created', async () => {
    await open(
      completed('write', {
        input: { filePath: '/workspace/src/new.ts', content: 'export const a = 1;' },
        output: 'Wrote file successfully.',
      }),
    );

    expect(screen.getByText('Content')).toBeInTheDocument();
    expect(document.body.textContent).toContain('export const a = 1;');
  });

  it('shows the command and its output for a shell call', async () => {
    await open(
      completed('bash', {
        input: { command: 'cargo test' },
        output: 'test result: ok. 0 passed',
        metadata: { exit: 0 },
      }),
    );

    expect(screen.getByText('Command')).toBeInTheDocument();
    expect(screen.getByText('Output')).toBeInTheDocument();
    expect(document.body.textContent).not.toContain('"command"');
  });

  it('calls out a command that failed', async () => {
    await open(
      completed('bash', {
        input: { command: 'cargo test' },
        output: 'error[E0308]: mismatched types',
        metadata: { exit: 101 },
      }),
    );

    expect(screen.getByText('The command exited with status 101.')).toBeInTheDocument();
  });

  it('lists the plan a todo write produced', async () => {
    await open(
      completed('todowrite', {
        input: {
          todos: [
            { content: 'Read the file', status: 'completed' },
            { content: 'Edit the file', status: 'pending' },
          ],
        },
      }),
    );

    expect(screen.getByText('Plan')).toBeInTheDocument();
    expect(screen.getByText('[x] Read the file')).toBeInTheDocument();
    expect(screen.getByText('[ ] Edit the file')).toBeInTheDocument();
  });

  it('lists search matches', async () => {
    await open(
      completed('grep', { input: { pattern: 'fn main' }, output: 'src/a.rs:10\nsrc/b.rs:4' }),
    );

    expect(screen.getByText('Pattern')).toBeInTheDocument();
    expect(document.body.textContent).toContain('src/a.rs:10');
  });

  it('still falls back to json for a tool it does not know', async () => {
    await open(completed('wibble', { input: { thing: 1 } }));

    expect(document.body.textContent).toContain('"thing": 1');
  });
});