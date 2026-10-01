import { useState } from 'react';
import { Button, Icon } from '@canonical/react-components';
import { describeToolCall, toolLine, toolPhrase } from '../../../lib/chat/describeToolCall';
import { hasDetail, normaliseToolState, toolNameOf } from '../../../lib/chat/normalise';
import { toolDetail } from './ToolDetailView';
import type { ToolPart, ToolState } from '../../../lib/types';

function iconFor(state: ToolState): { name: string; modifier: string } {
  switch (state.status) {
    case 'pending':
    case 'running':
      return { name: 'spinner', modifier: 'running' };
    case 'error':
      return { name: 'error', modifier: 'error' };
    default:
      return { name: 'tick', modifier: 'completed' };
  }
}

export function ToolCallStep({ part }: { part: ToolPart }) {
  const [open, setOpen] = useState(false);
  const state = normaliseToolState(part);
  const detail = hasDetail(state);
  const tool = toolNameOf(part);
  const line = toolLine(tool, state);
  const icon = iconFor(state);
  const error = state.status === 'error' ? state.error : '';
  const meta = error || line.meta;
  const view = open && detail ? toolDetail(tool, state) : null;

  const row = (
    <>
      <Icon className={`app-tc__icon app-tc__icon--${icon.modifier}`} name={icon.name} />
      <span className="app-tc__label">
        {line.subject ? (
          <>
            <span className="app-tc__gerund">{line.gerund}</span>
            <span className="app-tc__sep">: </span>
            <span className="app-tc__subject">{line.subject}</span>
          </>
        ) : (
          <span className="app-tc__gerund">{toolPhrase(tool)}</span>
        )}
      </span>
      {meta ? (
        <span className={`app-tc__meta${error ? ' app-tc__meta--error' : ''}`}>{meta}</span>
      ) : null}
      {detail ? <Icon className="app-tc__chevron" name="chevron-down" /> : null}
    </>
  );

  return (
    <div className={`app-tc app-tc--${state.status}${open ? ' app-tc--open' : ''}`}>
      {detail ? (
        <Button
          appearance="link"
          className="app-tc__row"
          aria-expanded={open}
          title={describeToolCall(tool, state)}
          onClick={() => setOpen((value) => !value)}
        >
          {row}
        </Button>
      ) : (
        <div className="app-tc__row">{row}</div>
      )}

      {view ? (
        <div className="app-tc__detail">
          <span className="app-tc__tool-tag">{tool}</span>
          {view.sections.map((section) => (
            <div className="app-tc__section" key={section.label}>
              <div className="app-tc__section-label">{section.label}</div>
              {section.body}
            </div>
          ))}
          {view.note ? <p className="app-tv__note">{view.note}</p> : null}
          {error ? <p className="app-tc__error">{error}</p> : null}
        </div>
      ) : null}
    </div>
  );
}
