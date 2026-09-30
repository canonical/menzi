import { describe, expect, it } from 'vitest';
import { describeToolCall, groupNoun, groupSummary, shortPath, toolLine } from './describeToolCall';
import type { ToolState } from '../types';

function running(input: Record<string, unknown> = {}, title?: string): ToolState {
  return title === undefined
    ? { status: 'running', input }
    : { status: 'running', input, title };
}

function done(
  input: Record<string, unknown> = {},
  extra: { output?: string; title?: string; metadata?: Record<string, unknown> } = {},
): ToolState {
  return {
    status: 'completed',
    input,
    output: extra.output ?? '',
    title: extra.title,
    metadata: extra.metadata,
  };
}

describe('shortPath', () => {
  it('drops the leading directory of an absolute path', () => {
    expect(shortPath('/workspace/README.md')).toBe('README.md');
    expect(shortPath('/workspace/docs/runbooks/local-dev.md')).toBe('docs/runbooks/local-dev.md');
  });

  it('leaves a relative path alone', () => {
    expect(shortPath('src/lib/chat.ts')).toBe('src/lib/chat.ts');
  });

  it('leaves a path with nothing to drop alone', () => {
    expect(shortPath('/hosts')).toBe('/hosts');
  });
});

describe('describeToolCall', () => {
  it('describes a file read with the path relative to the workspace', () => {
    expect(describeToolCall('read', running({ filePath: '/workspace/src/lib/chat.ts' }))).toBe(
      'Reading: src/lib/chat.ts',
    );
  });

  it('describes writes and edits', () => {
    expect(describeToolCall('write', running({ filePath: '/tmp/out.txt' }))).toBe('Writing: out.txt');
    expect(describeToolCall('edit', running({ filePath: '/w/a/b/c.rs' }))).toBe('Editing: a/b/c.rs');
    expect(describeToolCall('patch', running({ filePath: '/w/a/b/c.rs' }))).toBe('Editing: a/b/c.rs');
  });

  it('describes a command and clips a long one', () => {
    expect(describeToolCall('bash', running({ command: 'ls -la' }))).toBe('Running: ls -la');
    const long = 'cargo test --workspace --all-features --no-fail-fast -- --nocapture';
    const label = describeToolCall('bash', running({ command: long }));
    expect(label.startsWith('Running: ')).toBe(true);
    expect(label.endsWith('...')).toBe(true);
    expect(label.length).toBeLessThanOrEqual('Running: '.length + 52 + 3);
  });

  it('squashes a multi-line command onto one line', () => {
    expect(describeToolCall('bash', running({ command: 'ls\n  -la\n' }))).toBe('Running: ls -la');
  });

  it('describes a search and a glob', () => {
    expect(describeToolCall('grep', running({ pattern: 'TODO' }))).toBe('Searching: TODO');
    expect(describeToolCall('glob', running({ pattern: '**/*.ts' }))).toBe('Matching: **/*.ts');
  });

  it('describes a delegated task by its description', () => {
    expect(
      describeToolCall('task', running({ description: 'Explore the backend crates', subagent_type: 'explore' })),
    ).toBe('Delegating: Explore the backend crates');
  });

  it('describes fetching and searching the web', () => {
    expect(describeToolCall('webfetch', running({ url: 'https://example.com/a' }))).toBe(
      'Fetching: https://example.com/a',
    );
    expect(describeToolCall('websearch', running({ query: 'rust axum' }))).toBe(
      'Searching the web: rust axum',
    );
  });

  it('describes a question by its first ask', () => {
    expect(
      describeToolCall('question', running({ questions: [{ question: 'Which database?' }] })),
    ).toBe('Asking: Which database?');
  });

  it('describes loading a skill', () => {
    expect(describeToolCall('skill', running({ name: 'report' }))).toBe('Loading skill: report');
  });

  it('names the tool the model tried to use when it was invalid', () => {
    expect(describeToolCall('invalid', running({ tool: 'browser' }))).toBe('Skipped: browser');
  });

  it('falls back to the state title when the input is empty', () => {
    expect(describeToolCall('read', running({}, 'docs/plan.md'))).toBe('Reading: docs/plan.md');
    expect(describeToolCall('bash', running({}, 'make test'))).toBe('Running: make test');
  });

  it('says something sensible when there is nothing to describe', () => {
    expect(describeToolCall('read', running())).toBe('Reading a file');
    expect(describeToolCall('bash', running())).toBe('Running a command');
    expect(describeToolCall('todowrite', running())).toBe('Updating the plan');
  });

  it('falls back to words for an unknown tool', () => {
    expect(describeToolCall('some_future_tool', running())).toBe('some future tool');
  });

  it('never leaks underscores for any unknown name', () => {
    for (const name of ['a_b_c', 'x_y', 'weird_name_here']) {
      expect(describeToolCall(name, running())).not.toContain('_');
    }
  });

  it('ignores non-string arguments', () => {
    expect(describeToolCall('read', running({ filePath: 42 }))).toBe('Reading a file');
  });
});

describe('toolLine meta', () => {
  it('reports the exit code of a command', () => {
    expect(toolLine('bash', done({ command: 'ls' }, { metadata: { exit: 0 } })).meta).toBe('exit 0');
    expect(toolLine('bash', done({ command: 'ls' }, { metadata: { exit: 2 } })).meta).toBe('exit 2');
  });

  it('omits the exit code when the tool never reported one', () => {
    expect(toolLine('bash', done({ command: 'ls' })).meta).toBe('');
  });

  it('reports how many matches a search found', () => {
    expect(toolLine('grep', done({ pattern: 'a' }, { metadata: { matches: 12 } })).meta).toBe(
      '12 matches',
    );
    expect(toolLine('glob', done({ pattern: '*' }, { metadata: { matches: 3 } })).meta).toBe('3 files');
    expect(toolLine('grep', done({ pattern: 'a' }, { metadata: { matches: 0 } })).meta).toBe(
      'no matches',
    );
  });

  it('counts the lines a read returned', () => {
    const read = done({ filePath: '/w/a.rs' }, { output: '1: a\n2: b\n3: c\n' });
    expect(toolLine('read', read).meta).toBe('3 lines');
    expect(toolLine('read', done({ filePath: '/w/a.rs' }, { output: '' })).meta).toBe('empty');
  });

  it('ignores the wrapper lines around a read', () => {
    const read = done(
      { filePath: '/w/a.rs' },
      { output: '<path>/w/a.rs</path>\n<type>file</type>\n<content>\n1: a\n2: b\n</content>\n' },
    );
    expect(toolLine('read', read).meta).toBe('2 lines');
  });

  it('reports how much a write put on disk', () => {
    expect(toolLine('write', done({ filePath: '/w/a', content: 'hello' })).meta).toBe('5 B');
    expect(toolLine('write', done({ filePath: '/w/a', content: 'x'.repeat(2048) })).meta).toBe(
      '2.0 kB',
    );
  });

  it('calls out an edit that replaced every occurrence', () => {
    expect(toolLine('edit', done({ filePath: '/w/a' })).meta).toBe('');
    expect(toolLine('edit', done({ filePath: '/w/a', replaceAll: true })).meta).toBe(
      'all occurrences',
    );
  });

  it('names the subagent a task was given to', () => {
    expect(toolLine('task', done({ description: 'go', subagent_type: 'explore' })).meta).toBe(
      'explore',
    );
  });

  it('counts progress through the plan', () => {
    const todos = [
      { content: 'a', status: 'completed' },
      { content: 'b', status: 'in_progress' },
      { content: 'c', status: 'pending' },
    ];
    expect(toolLine('todowrite', done({ todos })).meta).toBe('1 of 3 done');
    expect(toolLine('todowrite', done({ todos: [] })).meta).toBe('');
  });

  it('counts web search results', () => {
    expect(toolLine('websearch', done({ query: 'a' }, { metadata: { numResults: 5 } })).meta).toBe(
      '5 results',
    );
    expect(toolLine('websearch', done({ query: 'a' })).meta).toBe('no results');
  });

  it('counts a multi part question', () => {
    expect(
      toolLine('question', done({ questions: [{ question: 'a' }, { question: 'b' }] })).meta,
    ).toBe('2 questions');
    expect(toolLine('question', done({ questions: [{ question: 'a' }] })).meta).toBe('');
  });

  it('says nothing while the call is still running', () => {
    expect(toolLine('bash', running({ command: 'ls' })).meta).toBe('');
  });

  it('uses the singular for a single one', () => {
    expect(toolLine('read', done({ filePath: '/w/a' }, { output: 'a' })).meta).toBe('1 line');
    expect(toolLine('read', done({ filePath: '/w/a' }, { output: 'a\nb' })).meta).toBe('2 lines');
    expect(toolLine('grep', done({ pattern: 'a' }, { metadata: { matches: 1 } })).meta).toBe(
      '1 match',
    );
    expect(toolLine('glob', done({ pattern: '*' }, { metadata: { matches: 1 } })).meta).toBe('1 file');
    expect(toolLine('websearch', done({ query: 'a' }, { metadata: { numResults: 1 } })).meta).toBe(
      '1 result',
    );
  });

  it('counts matches from the output when no count was reported', () => {
    const grep = done({ pattern: 'a' }, { output: 'src/a.ts:1: a\nsrc/b.ts:2: a' });
    expect(toolLine('grep', grep).meta).toBe('2 matches');
    expect(toolLine('grep', done({ pattern: 'a' }, { output: '' })).meta).toBe('no matches');
  });

  it('leaves the meta empty for an error so the error itself shows', () => {
    const failed: ToolState = {
      status: 'error',
      input: { command: 'ls' },
      error: 'command not found',
    };
    expect(toolLine('bash', failed).meta).toBe('');
  });
});

describe('groupNoun and groupSummary', () => {
  it('uses a noun map for known tools', () => {
    expect(groupNoun('read')).toBe('file reads');
    expect(groupNoun('bash')).toBe('commands');
  });

  it('falls back to words for unknown tools', () => {
    expect(groupNoun('mystery_tool')).toBe('mystery tool');
  });

  it('summarises a count', () => {
    expect(groupSummary('read', 4)).toBe('Ran 4 file reads');
    expect(groupSummary('bash', 2)).toBe('Ran 2 commands');
  });

  it('describes a single run rather than counting it', () => {
    expect(groupSummary('read', 1)).toBe('Reading a file');
    expect(groupSummary('todowrite', 1)).toBe('Updating the plan');
  });
});
