import { Icon } from '@canonical/react-components';
import { oneLine } from '../../../lib/format/text';
import type { CompactionPart } from '../../../lib/types';
import { S } from '../../../strings/catalogue';

const PREVIEW_LIMIT = 72;

export function ThinkingBlock({ text }: { text: string }) {
  if (!text.trim()) return null;
  return (
    <details className="app-thinking">
      <summary className="app-thinking__summary" title={text.trim()}>
        <Icon className="app-thinking__chevron" name="chevron-down" />
        <span className="app-thinking__label">{S.chatSteps.thinking}</span>
        <span className="app-thinking__preview">{oneLine(text, PREVIEW_LIMIT)}</span>
      </summary>
      <p className="app-thinking__body">{text}</p>
    </details>
  );
}

function compactionLabel(part: CompactionPart): string {
  if (part.error) return S.chatSteps.compaction.error;
  if (part.status === 'running') return S.chatSteps.compaction.running;
  return S.chatSteps.compaction.completed;
}

export function CompactionMarker({ part }: { part: CompactionPart }) {
  return (
    <p className={`app-compaction${part.error ? ' app-compaction--error' : ''}`}>
      <Icon name={part.error ? 'error' : 'compress'} />
      {compactionLabel(part)}
    </p>
  );
}
