const MAX_COMMAND_LENGTH = 28;

function trimCommand(value: unknown, fallback: string): string {
  if (typeof value !== 'string' || value.trim().length === 0) return fallback;
  const trimmed = value.trim();
  return trimmed.length > MAX_COMMAND_LENGTH
    ? `${trimmed.slice(0, MAX_COMMAND_LENGTH).trimEnd()}...`
    : trimmed;
}

function basename(value: unknown, fallback: string): string {
  if (typeof value !== 'string' || value.trim().length === 0) return fallback;
  const parts = value.split('/').filter(Boolean);
  return parts.length > 0 ? parts[parts.length - 1] : fallback;
}

function str(input: Record<string, unknown>, key: string): string | null {
  const value = input[key];
  return typeof value === 'string' && value.trim().length > 0 ? value.trim() : null;
}

export function describeToolCall(name: string, input: Record<string, unknown>): string {
  switch (name) {
    case 'read':
    case 'Read':
      return `Reading ${basename(str(input, 'filePath') ?? str(input, 'path'), 'file')}`;
    case 'write':
    case 'Write':
      return `Writing ${basename(str(input, 'filePath') ?? str(input, 'path'), 'file')}`;
    case 'edit':
    case 'Edit':
    case 'patch':
    case 'Patch':
    case 'multiedit': {
      const target = basename(str(input, 'filePath') ?? str(input, 'path'), 'file');
      return `Editing ${target}`;
    }
    case 'bash':
    case 'Bash':
    case 'shell': {
      const command = trimCommand(str(input, 'command') ?? str(input, 'cmd'), 'command');
      const cwd = str(input, 'workdir') ?? str(input, 'cwd');
      return cwd ? `Running "${command}" in ${basename(cwd, 'directory')}` : `Running "${command}"`;
    }
    case 'grep':
    case 'Grep': {
      const pattern = str(input, 'pattern') ?? 'pattern';
      const where = str(input, 'path');
      return where ? `Searching for "${pattern}" in ${basename(where, 'directory')}` : `Searching for "${pattern}"`;
    }
    case 'glob':
    case 'Glob': {
      const pattern = str(input, 'pattern') ?? 'files';
      return `Matching files against ${pattern}`;
    }
    case 'list':
    case 'List':
      return `Listing ${basename(str(input, 'path'), 'directory')}`;
    case 'todowrite':
    case 'TodoWrite':
      return 'Updating the task list';
    case 'todoread':
    case 'TodoRead':
      return 'Reading the task list';
    case 'webfetch':
    case 'WebFetch':
      return `Fetching ${str(input, 'url') ?? 'a page'}`;
    case 'switchModel':
    case 'switchAgent':
      return `Switching ${name.replace('switch', '').toLowerCase()}`;
    case 'interrupt':
      return 'Interrupting the current run';
    default:
      return name.replace(/_/g, ' ');
  }
}

const GROUP_NOUNS: Record<string, string> = {
  read: 'file reads',
  write: 'file writes',
  edit: 'edits',
  patch: 'patches',
  bash: 'commands',
  shell: 'commands',
  grep: 'searches',
  glob: 'file matches',
  list: 'directory listings',
};

export function groupNoun(name: string): string {
  return GROUP_NOUNS[name] ?? name.replace(/_/g, ' ');
}

export function groupSummary(name: string, count: number): string {
  return count === 1
    ? describeToolCall(name, {})
    : `Ran ${count} ${groupNoun(name)}`;
}
