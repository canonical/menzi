import { Icon } from '@canonical/react-components';
import { formatDuration } from '../../../lib/format/duration';
import type { CompactionPart, ReasoningPart } from '../../../lib/types';
import { S } from '../../../strings/catalogue';

function durationOf(part: ReasoningPart): string {
  const start = part.time?.start;
  const end = part.time?.end;
  if (typeof start !== 'number' || typeof end !== 'number') return '';
  return formatDuration(end - start);
}

export function ThinkingBlock({ part }: { part: ReasoningPart }) {
  if (!part.text.trim()) return null;
  const duration = durationOf(part);
  const label = duration
    ? S.chatSteps.thoughtFor.replace('{duration}', duration)
    : S.chatSteps.thinking;
  return (
    <details className="app-thinking">
      <summary className="app-thinking__summary" title={part.text.trim()}>
        <Icon className="app-thinking__chevron" name="chevron-down" />
        <span className="app-thinking__label">{label}</span>
      </summary>
      <p className="app-thinking__body">{part.text}</p>
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
