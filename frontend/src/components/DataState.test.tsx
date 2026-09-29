import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../testing/axe';
import { DataState } from './DataState';
import { S } from '../strings/catalogue';

describe('DataState', () => {
  it('shows a loading spinner while loading', async () => {
    const { container } = render(
      <DataState loading error={null} empty={false} onRetry={vi.fn()}>
        <p>content</p>
      </DataState>,
    );
    expect(screen.getByText(S.dataState.loading)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows the error with a retry action', async () => {
    const onRetry = vi.fn();
    const { container } = render(
      <DataState loading={false} error="boom" empty={false} onRetry={onRetry}>
        <p>content</p>
      </DataState>,
    );
    expect(screen.getByText('boom')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: S.dataState.retry }));
    expect(onRetry).toHaveBeenCalledTimes(1);
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows the empty state', async () => {
    const { container } = render(
      <DataState
        loading={false}
        error={null}
        empty
        emptyTitle="Nothing here"
        emptyBody="Try again later"
      >
        <p>content</p>
      </DataState>,
    );
    expect(screen.getByText('Nothing here')).toBeInTheDocument();
    expect(screen.getByText('Try again later')).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('renders children when there is content', () => {
    render(
      <DataState loading={false} error={null} empty={false}>
        <p>content</p>
      </DataState>,
    );
    expect(screen.getByText('content')).toBeInTheDocument();
  });
});