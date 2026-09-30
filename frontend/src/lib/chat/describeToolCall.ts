import { oneLine } from '../format/text';
import { asRecord } from './normalise';
import type { ToolState } from '../types';

const MAX_COMMAND = 52;
const MAX_QUERY = 60;
const MAX_QUESTION = 60;

type Fields = Record<string, unknown>;

interface Wordings {
  gerund: string;
  object: string;
}

const TOOLS: Record<string, Wordings> = {
  read: { gerund: 'Reading', object: 'a file' },
  write: { gerund: 'Writing', object: 'a file' },
  edit: { gerund: 'Editing', object: 'a file' },
  patch: { gerund: 'Editing', object: 'a file' },
  bash: { gerund: 'Running', object: 'a command' },
  shell: { gerund: 'Running', object: 'a command' },
  grep: { gerund: 'Searching', object: 'a pattern' },
  glob: { gerund: 'Matching', object: 'files' },
  list: { gerund: 'Listing', object: 'a directory' },
  webfetch: { gerund: 'Fetching', object: 'a page' },
  websearch: { gerund: 'Searching the web', object: '' },
  task: { gerund: 'Delegating', object: 'a subagent' },
  question: { gerund: 'Asking', object: 'a question' },
  skill: { gerund: 'Loading skill', object: '' },
  todowrite: { gerund: 'Updating the plan', object: '' },
  todoread: { gerund: 'Reading the plan', object: '' },
  invalid: { gerund: 'Skipped', object: 'a tool' },
};

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

export interface ToolLine {
  gerund: string;
  subject: string;
  meta: string;
}

function str(fields: Fields, ...keys: string[]): string {
  for (const key of keys) {
    const value = fields[key];
    if (typeof value === 'string' && value.trim().length > 0) return value.trim();
  }
  return '';
}

function num(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function outputOf(state: ToolState): string {
  return state.status === 'completed' ? state.output : '';
}

function titleOf(state: ToolState): string {
  if (!('title' in state)) return '';
  return typeof state.title === 'string' ? state.title.trim() : '';
}

export function shortPath(value: string): string {
  if (!value.startsWith('/')) return value;
  const parts = value.split('/').filter(Boolean);
  if (parts.length < 2) return value;
  return parts.slice(1).join('/');
}

function firstQuestion(fields: Fields): string {
  const questions = fields.questions;
  if (!Array.isArray(questions) || questions.length === 0) return '';
  return str(asRecord(questions[0]), 'question', 'header', 'text');
}

function todoCounts(fields: Fields): { done: number; total: number } {
  const todos = fields.todos;
  if (!Array.isArray(todos)) return { done: 0, total: 0 };
  const done = todos.filter((todo) => str(asRecord(todo), 'status') === 'completed').length;
  return { done, total: todos.length };
}

function contentLines(state: ToolState, metadata: Fields): number {
  const display = asRecord(metadata.display);
  const text = typeof display.text === 'string' ? display.text : outputOf(state);
  return text
    .split('\n')
    .filter((line) => line.trim().length > 0 && !line.startsWith('<')).length;
}

function bytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} kB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function subjectOf(name: string, state: ToolState): string {
  const input = state.input;
  switch (name) {
    case 'read':
    case 'write':
    case 'edit':
    case 'patch':
      return shortPath(str(input, 'filePath', 'path') || titleOf(state));
    case 'bash':
    case 'shell':
      return oneLine(str(input, 'command', 'cmd') || titleOf(state), MAX_COMMAND);
    case 'grep':
    case 'glob':
      return oneLine(str(input, 'pattern') || titleOf(state), MAX_QUERY);
    case 'list':
      return shortPath(str(input, 'path', 'directory') || titleOf(state));
    case 'webfetch':
      return oneLine(str(input, 'url') || titleOf(state), MAX_QUERY);
    case 'websearch':
      return oneLine(str(input, 'query') || titleOf(state), MAX_QUERY);
    case 'task':
      return oneLine(str(input, 'description') || titleOf(state), MAX_QUERY);
    case 'question':
      return oneLine(firstQuestion(input), MAX_QUESTION);
    case 'skill':
      return str(input, 'name') || titleOf(state);
    case 'invalid':
      return str(input, 'tool') || 'tool';
    default:
      return '';
  }
}

function metaOf(name: string, state: ToolState): string {
  if (state.status !== 'completed') return '';
  const metadata = asRecord(state.metadata);
  const input = state.input;

  switch (name) {
    case 'bash':
    case 'shell': {
      const code = num(metadata.exit);
      return code === null ? '' : `exit ${code}`;
    }
    case 'grep':
    case 'glob': {
      const [one, many] = name === 'glob' ? ['file', 'files'] : ['match', 'matches'];
      const matches = num(metadata.matches) ?? contentLines(state, metadata);
      if (matches === 0) return `no ${many}`;
      return `${matches} ${matches === 1 ? one : many}`;
    }
    case 'read': {
      const lines = contentLines(state, metadata);
      if (lines === 0) return 'empty';
      return lines === 1 ? '1 line' : `${lines} lines`;
    }
    case 'write': {
      const length = str(input, 'content').length;
      return length === 0 ? '' : bytes(length);
    }
    case 'edit':
    case 'patch':
      return input.replaceAll === true ? 'all occurrences' : '';
    case 'task':
      return str(input, 'subagent_type');
    case 'todowrite': {
      const { done, total } = todoCounts(input);
      if (total === 0) return '';
      return `${done} of ${total} done`;
    }
    case 'websearch': {
      const results = num(metadata.numResults) ?? contentLines(state, metadata);
      if (results === 0) return 'no results';
      return results === 1 ? '1 result' : `${results} results`;
    }
    case 'question': {
      const questions = input.questions;
      if (!Array.isArray(questions) || questions.length < 2) return '';
      return `${questions.length} questions`;
    }
    default:
      return '';
  }
}

function wordingsFor(name: string): Wordings {
  return TOOLS[name] ?? { gerund: name.replace(/_/g, ' '), object: '' };
}

export function toolLine(name: string, state: ToolState): ToolLine {
  const wording = wordingsFor(name);
  return {
    gerund: wording.gerund,
    subject: subjectOf(name, state),
    meta: metaOf(name, state),
  };
}

export function describeToolCall(name: string, state: ToolState): string {
  const { gerund, subject } = toolLine(name, state);
  return subject ? `${gerund}: ${subject}` : toolPhrase(name);
}

export function toolPhrase(name: string): string {
  const wording = wordingsFor(name);
  return wording.object ? `${wording.gerund} ${wording.object}` : wording.gerund;
}

export function groupNoun(name: string): string {
  return GROUP_NOUNS[name] ?? name.replace(/_/g, ' ');
}

export function groupSummary(name: string, count: number): string {
  if (count > 1) return `Ran ${count} ${groupNoun(name)}`;
  return toolPhrase(name);
}
