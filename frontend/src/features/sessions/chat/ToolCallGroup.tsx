import { useEffect, useState } from 'react';
import { Icon } from '@canonical/react-components';
import { groupSummary } from '../../../lib/chat/describeToolCall';
import { isActive, normaliseToolState, toolNameOf } from '../../../lib/chat/normalise';
import type { ToolPart } from '../../../lib/types';
import { ToolCallStep } from './ToolCallStep';

export function ToolCallGroup({ parts }: { parts: ToolPart[] }) {
  const running = parts.some((part) => isActive(normaliseToolState(part)));
  const [open, setOpen] = useState(running);

  useEffect(() => {
    if (running) setOpen(true);
  }, [running]);

  return (
    <details
      className={`app-tcg${running ? ' app-tcg--running' : ''}`}
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary className="app-tcg__summary">
        <Icon className="app-tcg__chevron" name="chevron-down" />
        <span className="app-tcg__text">{groupSummary(toolNameOf(parts[0]), parts.length)}</span>
      </summary>
      <div className="app-tcg__items">
        {parts.map((part, index) => (
          <ToolCallStep key={part.id ?? index} part={part} />
        ))}
      </div>
    </details>
  );
}
