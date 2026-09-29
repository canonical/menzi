import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { NotificationProvider } from '@canonical/react-components';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../../testing/axe';
import { SettingsPage } from './SettingsPage';
import { S } from '../../strings/catalogue';

function renderPage() {
  return render(
    <NotificationProvider>
      <SettingsPage />
    </NotificationProvider>,
  );
}

function getForm(): HTMLFormElement {
  const form = document.querySelector('form');
  if (!form) throw new Error('form not found');
  return form;
}

describe('SettingsPage', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('validates the profile form', async () => {
    const { container } = renderPage();
    fireEvent.submit(getForm());
    expect(screen.getByText(S.settings.profile.errors.nameRequired)).toBeInTheDocument();
    expect(screen.getByText(S.settings.profile.errors.emailInvalid)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('saves a valid profile', () => {
    vi.useFakeTimers();
    renderPage();
    fireEvent.change(screen.getByLabelText(S.settings.profile.nameLabel), {
      target: { value: 'Ada' },
    });
    fireEvent.change(screen.getByLabelText(S.settings.profile.emailLabel), {
      target: { value: 'ada@example.com' },
    });
    fireEvent.submit(getForm());
    expect(screen.getByText(S.dataState.loading)).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(screen.queryByText(S.dataState.loading)).not.toBeInTheDocument();
  });

  it('switches between settings tabs', async () => {
    const user = userEvent.setup();
    renderPage();
    await user.click(screen.getByRole('tab', { name: S.settings.tabs.modelAccounts }));
    expect(screen.getByText(S.settings.modelAccounts.intro)).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: S.settings.tabs.notifications }));
    expect(screen.getByText(S.settings.notifications.intro)).toBeInTheDocument();
    await user.click(screen.getByRole('tab', { name: S.settings.tabs.profile }));
    expect(screen.getByLabelText(S.settings.profile.nameLabel)).toBeInTheDocument();
  });
});