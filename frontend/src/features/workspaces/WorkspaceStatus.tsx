import { Button, Chip } from '@canonical/react-components';
import type { WorkspaceState } from './useWorkspace';
import { S } from '../../strings/catalogue';

interface WorkspaceStatusProps {
  state: WorkspaceState;
  onRetry?: () => void;
  /**
   * The happy path adds no chrome at all, so a ready workspace says nothing
   * unless the surface asks for confirmation.
   */
  announceReady?: boolean;
}

function elapsedLabel(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

export function WorkspaceStatus({ state, onRetry, announceReady }: WorkspaceStatusProps) {
  if (!state.shouldAnnounce) {
    if (state.phase === 'ready' && announceReady) {
      return (
        <div className="app-workspace-status" data-testid="workspace-status">
          <Chip value={S.workspace.ready} appearance="positive" isReadOnly isDense />
        </div>
      );
    }
    return null;
  }

  if (state.phase === 'setting-up') {
    return (
      <div className="app-workspace-status" data-testid="workspace-status">
        <span className="p-icon--spinner u-animation--spin" role="progressbar" aria-label={S.workspace.settingUp} />
        <span className="app-workspace-status__text">
          {state.isSlow
            ? S.workspace.settingUpElapsed.replace('{elapsed}', elapsedLabel(state.elapsedMs))
            : S.workspace.settingUp}
        </span>
        {state.isSlow ? <span className="u-text--muted">{S.workspace.slowHint}</span> : null}
      </div>
    );
  }

  if (state.phase === 'failed') {
    const reason = state.lastError ?? state.error;
    return (
      <div
        className="app-workspace-status app-workspace-status--failed"
        data-testid="workspace-status"
      >
        <Chip value={S.workspace.failedTitle} appearance="negative" isReadOnly isDense />
        {reason ? <span className="app-workspace-status__reason">{reason}</span> : null}
        {onRetry ? (
          <Button appearance="link" onClick={onRetry}>
            {S.workspace.retry}
          </Button>
        ) : null}
      </div>
    );
  }

  if (state.phase === 'not-permitted') {
    return (
      <div className="app-workspace-status" data-testid="workspace-status">
        <Chip value={S.workspace.notAMemberTitle} appearance="caution" isReadOnly isDense />
        <span className="u-text--muted">{S.workspace.notAMemberBody}</span>
      </div>
    );
  }

  if (state.phase === 'unreachable') {
    return (
      <div
        className="app-workspace-status app-workspace-status--failed"
        data-testid="workspace-status"
      >
        <Chip value={S.workspace.unreachable} appearance="negative" isReadOnly isDense />
        {onRetry ? (
          <Button appearance="link" onClick={onRetry}>
            {S.workspace.retry}
          </Button>
        ) : null}
      </div>
    );
  }

  if (state.phase === 'idle') {
    return (
      <div className="app-workspace-status" data-testid="workspace-status">
        <Chip value={S.workspace.card.statuses.idle} appearance="caution" isReadOnly isDense />
        <span className="u-text--muted">{S.workspace.pauseHint}</span>
      </div>
    );
  }

  return null;
}


