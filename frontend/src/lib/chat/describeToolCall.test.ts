import { describe, expect, it } from 'vitest';
import { describeToolCall, groupNoun, groupSummary } from './describeToolCall';

describe('describeToolCall', () => {
  it('describes file reads with the basename', () => {
    expect(describeToolCall('read', { filePath: 'src/lib/chat.ts' })).toBe('Reading chat.ts');
  });

  it('describes writes and edits', () => {
    expect(describeToolCall('write', { path: '/tmp/out.txt' })).toBe('Writing out.txt');
    expect(describeToolCall('edit', { filePath: 'a/b/c.rs' })).toBe('Editing c.rs');
    expect(describeToolCall('patch', { filePath: 'a/b/c.rs' })).toBe('Editing c.rs');
  });

  it('describes commands and truncates long ones', () => {
    expect(describeToolCall('bash', { command: 'ls -la' })).toBe('Running "ls -la"');
    const long = 'cargo test --workspace --all-features';
    const label = describeToolCall('bash', { command: long });
    const quoted = label.slice(label.indexOf('"') + 1, label.lastIndexOf('"'));
    expect(label.startsWith('Running "')).toBe(true);
    expect(quoted.endsWith('...')).toBe(true);
    expect(quoted.length).toBeLessThanOrEqual(31);
  });

  it('mentions the working directory when given', () => {
    expect(describeToolCall('bash', { command: 'ls', workdir: '/repo/frontend' })).toBe(
      'Running "ls" in frontend',
    );
  });

  it('describes searches and file matching', () => {
    expect(describeToolCall('grep', { pattern: 'TODO', path: 'src/app' })).toBe(
      'Searching for "TODO" in app',
    );
    expect(describeToolCall('glob', { pattern: '**/*.ts' })).toBe('Matching files against **/*.ts');
  });

  it('describes directories and task lists', () => {
    expect(describeToolCall('list', { path: 'src/features' })).toBe('Listing features');
    expect(describeToolCall('todowrite', {})).toBe('Updating the task list');
  });

  it('falls back to words for an unknown tool', () => {
    expect(describeToolCall('some_future_tool', {})).toBe('some future tool');
  });

  it('never leaks underscores for any unknown name', () => {
    for (const name of ['a_b_c', 'x_y', 'weird_name_here']) {
      expect(describeToolCall(name, {})).not.toContain('_');
    }
  });

  it('handles missing arguments without throwing', () => {
    expect(describeToolCall('read', {})).toBe('Reading file');
    expect(describeToolCall('bash', {})).toBe('Running "command"');
  });

  it('ignores non-string arguments', () => {
    expect(describeToolCall('read', { filePath: 42 })).toBe('Reading file');
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
    expect(groupSummary('read', 1)).toBe('Reading file');
  });
});
