import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { WorkspaceStatus } from './WorkspaceStatus';
import { blockReason } from './reasons';
import type { WorkspaceState } from './useWorkspace';

function state(overrides: Partial<WorkspaceState>): WorkspaceState {
  return {
    phase: 'ready',
    workspace: null,
    shouldAnnounce: false,
    elapsedMs: 0,
    isSlow: false,
    error: null,
    lastError: null,
    canStartSession: true,
    reason: null,
    ...overrides,
  };
}

describe('WorkspaceStatus', () => {
  it('says nothing when everything is ready', () => {
    const { container } = render(<WorkspaceStatus state={state({})} />);
    expect(container).toBeEmptyDOMElement();
  });

  it('confirms a ready workspace only when asked to', () => {
    render(
      <WorkspaceStatus state={state({ phase: 'ready', workspace: {} as never })} announceReady />,
    );
    expect(screen.getByTestId('workspace-status')).toHaveTextContent('Workspace ready');
  });

  it('explains that setting up takes about thirty seconds', () => {
    render(
      <WorkspaceStatus
        state={state({ phase: 'setting-up', shouldAnnounce: true, elapsedMs: 5000 })}
      />,
    );
    expect(screen.getByTestId('workspace-status')).toHaveTextContent(
      'Setting up your workspace',
    );
    expect(screen.getByTestId('workspace-status')).not.toHaveTextContent('Still setting up');
  });

  it('switches to a slower message once it has been going a while', () => {
    render(
      <WorkspaceStatus
        state={state({
          phase: 'setting-up',
          shouldAnnounce: true,
          elapsedMs: 125_000,
          isSlow: true,
        })}
      />,
    );
    const status = screen.getByTestId('workspace-status');
    expect(status).toHaveTextContent('Still setting up (2m 5s)');
    expect(status).toHaveTextContent('You can keep browsing');
  });

  it('shows the reason a setup failed and offers a retry', () => {
    const onRetry = () => {};
    render(
      <WorkspaceStatus
        state={state({
          phase: 'failed',
          shouldAnnounce: true,
          lastError: 'lxd: the image could not be copied',
        })}
        onRetry={onRetry}
      />,
    );
    const status = screen.getByTestId('workspace-status');
    expect(status).toHaveTextContent('Workspace setup failed');
    expect(status).toHaveTextContent('lxd: the image could not be copied');
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('offers a retry when the service is unreachable', () => {
    render(
      <WorkspaceStatus
        state={state({ phase: 'unreachable', shouldAnnounce: true })}
        onRetry={() => {}}
      />,
    );
    expect(screen.getByTestId('workspace-status')).toHaveTextContent(
      "Can't reach the workspace service.",
    );
  });

  it('treats missing access as a permission rather than a fault', () => {
    render(
      <WorkspaceStatus state={state({ phase: 'not-permitted', shouldAnnounce: true })} />,
    );
    const status = screen.getByTestId('workspace-status');
    expect(status).toHaveTextContent('No workspace for you in this project');
    expect(status).toHaveTextContent('Ask a project maintainer');
    expect(status).not.toHaveTextContent('Try again');
  });

  it('says a paused workspace will wake on the first session', () => {
    render(<WorkspaceStatus state={state({ phase: 'idle', shouldAnnounce: true })} />);
    expect(screen.getByTestId('workspace-status')).toHaveTextContent(
      'Starting a session will wake it',
    );
  });
});

describe('blockReason', () => {
  it('is silent when there is no reason to give', () => {
    expect(blockReason(null)).toBeNull();
  });

  it('explains why a session is blocked', () => {
    expect(blockReason('setting-up')).toBe('Your workspace is still being set up.');
  });

  it('explains a permission block without alarming', () => {
    expect(blockReason('not-permitted')).toBe(
      'You do not have access to a workspace in this project.',
    );
  });

  it('falls back to nothing for a reason it does not know', () => {
    expect(blockReason('invented-later')).toBeNull();
  });
});
