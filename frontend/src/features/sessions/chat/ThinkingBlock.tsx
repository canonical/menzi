import { Icon } from '@canonical/react-components';
import type { CompactionPart } from '../../../lib/types';
import { S } from '../../../strings/catalogue';

export function ThinkingBlock({ text }: { text: string }) {
  if (!text.trim()) return null;
  return (
    <details className="app-thinking">
      <summary className="app-thinking__summary">
        <Icon className="app-thinking__chevron" name="chevron-down" />
        {S.chatSteps.thinking}
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
