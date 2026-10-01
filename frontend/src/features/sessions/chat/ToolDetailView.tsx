import type { ReactNode } from 'react';
import type { ToolState } from '../../../lib/types';
import { asRecord } from '../../../lib/chat/normalise';
import { DiffPreview } from './DiffPreview';
import { CodeBlock } from '../CodeBlock';

const PREVIEW_LIMIT = 4000;

function str(fields: Record<string, unknown>, ...keys: string[]): string {
  for (const key of keys) {
    const value = fields[key];
    if (typeof value === 'string' && value.trim().length > 0) return value;
  }
  return '';
}

function outputOf(state: ToolState): string {
  return state.status === 'completed' ? state.output : '';
}

interface Section {
  label: string;
  body: ReactNode;
}

export interface ToolDetail {
  sections: Section[];
  note?: string;
}

function field(label: string, value: string): Section {
  return {
    label,
    body: (
      <div className="app-tv__row">
        <span className="app-tv__value">{value}</span>
      </div>
    ),
  };
}

function code(label: string, text: string, file?: string): Section {
  return {
    label,
    body: <CodeBlock className={file ? `language-${extensionOf(file)}` : undefined} text={text} />,
  };
}

function extensionOf(file: string): string {
  const dot = file.lastIndexOf('.');
  return dot === -1 ? '' : file.slice(dot + 1);
}

function list(label: string, items: string[]): Section {
  return {
    label,
    body: (
      <ul className="app-tv__list">
        {items.map((item, index) => (
          <li key={`${index}-${item}`}>{item}</li>
        ))}
      </ul>
    ),
  };
}

function lines(label: string, text: string, limit = 200): Section {
  const all = text.split('\n');
  const shown = all.slice(0, limit);
  return {
    label,
    body: (
      <>
        <pre className="app-tc__data">{shown.join('\n')}</pre>
        {all.length > limit ? (
          <p className="app-tv__note">{`Showing ${limit} of ${all.length} lines.`}</p>
        ) : null}
      </>
    ),
  };
}

export function toolDetail(tool: string, state: ToolState): ToolDetail {
  const input = state.input;
  const metadata = asRecord('metadata' in state ? state.metadata : undefined);
  const file = str(input, 'filePath', 'path', 'file');

  switch (tool) {
    case 'edit':
    case 'patch': {
      const diff = str(metadata, 'diff');
      if (diff) {
        return {
          sections: [
            ...(file ? [field('File', file)] : []),
            { label: 'Changes', body: <DiffPreview file={file} patch={diff} /> },
          ],
        };
      }
      const before = str(input, 'oldString');
      const after = str(input, 'newString');
      return {
        sections: [
          ...(file ? [field('File', file)] : []),
          ...(before ? [code('Replaced', before.slice(0, PREVIEW_LIMIT), file)] : []),
          ...(after ? [code('With', after.slice(0, PREVIEW_LIMIT), file)] : []),
        ],
      };
    }

    case 'write': {
      const content = str(input, 'content');
      return {
        sections: [
          ...(file ? [field('File', file)] : []),
          ...(content ? [code('Content', content.slice(0, PREVIEW_LIMIT), file)] : []),
        ],
        ...(content === '' && file ? { note: 'The file was created empty.' } : {}),
      };
    }

    case 'read':
      return {
        sections: [
          ...(file ? [field('File', file)] : []),
          ...(outputOf(state)
            ? [lines('Contents', outputOf(state).slice(0, 20_000))]
            : []),
        ],
      };

    case 'bash':
    case 'shell': {
      const command = str(input, 'command', 'cmd');
      const exit = typeof metadata.exit === 'number' ? metadata.exit : null;
      const output = outputOf(state);
      return {
        sections: [
          ...(command ? [code('Command', command, 'sh')] : []),
          ...(output ? [lines('Output', output.slice(0, 20_000))] : []),
        ],
        ...(exit !== null && exit !== 0
          ? { note: `The command exited with status ${exit}.` }
          : {}),
      };
    }

    case 'grep':
    case 'glob':
      return {
        sections: [
          ...(str(input, 'pattern') ? [field('Pattern', str(input, 'pattern'))] : []),
          ...(file ? [field('In', file)] : []),
          ...(outputOf(state)
            ? [
                lines(
                  'Matches',
                  outputOf(state)
                    .split('\n')
                    .filter((line) => line.trim().length > 0)
                    .slice(0, 200)
                    .join('\n'),
                ),
              ]
            : []),
        ],
      };

    case 'list':
      return {
        sections: [
          ...(str(input, 'path', 'directory') ? [field('Directory', str(input, 'path', 'directory'))] : []),
          ...(outputOf(state) ? [lines('Entries', outputOf(state).slice(0, 20_000))] : []),
        ],
      };

    case 'webfetch':
      return {
        sections: [
          ...(str(input, 'url') ? [field('URL', str(input, 'url'))] : []),
          ...(outputOf(state) ? [lines('Response', outputOf(state).slice(0, 20_000))] : []),
        ],
      };

    case 'websearch':
      return {
        sections: [
          ...(str(input, 'query') ? [field('Query', str(input, 'query'))] : []),
          ...(outputOf(state) ? [lines('Results', outputOf(state).slice(0, 20_000))] : []),
        ],
      };

    case 'todowrite': {
      const todos = Array.isArray(input.todos) ? input.todos : [];
      const items = todos.map((todo) => {
        const record = asRecord(todo);
        const status = str(record, 'status');
        const mark = status === 'completed' ? '[x]' : '[ ]';
        return `${mark} ${str(record, 'content', 'task', 'title')}`;
      });
      return {
        sections: [
          ...(items.length ? [list('Plan', items)] : []),
        ],
      };
    }

    case 'task':
      return {
        sections: [
          ...(str(input, 'description') ? [field('Subagent', str(input, 'description'))] : []),
          ...(str(input, 'subagent_type') ? [field('Type', str(input, 'subagent_type'))] : []),
          ...(outputOf(state) ? [lines('Report', outputOf(state).slice(0, 20_000))] : []),
        ],
      };

    case 'question': {
      const questions = Array.isArray(input.questions) ? input.questions : [];
      const items = questions.map((question) =>
        str(asRecord(question), 'question', 'header', 'text'),
      );
      return {
        sections: [
          ...(items.length ? [list('Asked', items)] : []),
          ...(str(input, 'answer') ? [field('Answer', str(input, 'answer'))] : []),
        ],
      };
    }

    case 'skill':
      return {
        sections: [...(str(input, 'name') ? [field('Skill', str(input, 'name'))] : [])],
      };

    case 'invalid':
      return {
        sections: [
          ...(str(input, 'tool') ? [field('Tool', str(input, 'tool'))] : []),
        ],
        ...(str(input, 'reason') ? { note: str(input, 'reason') } : {}),
      };

    default:
      return {
        sections: [lines('Input', JSON.stringify(input, null, 2))],
      };
  }
}