import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { NotificationProvider } from '@canonical/react-components';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../../testing/axe';
import { SettingsPage } from './SettingsPage';
import { S } from '../../strings/catalogue';

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: 'OK',
      text: async () => JSON.stringify({ devices: [] }),
      json: async () => ({ devices: [] }),
    }),
  );
  return render(
    <QueryClientProvider client={client}>
      <NotificationProvider>
        <MemoryRouter>
          <SettingsPage />
        </MemoryRouter>
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

function getForm(): HTMLFormElement {
  const form = document.querySelector('form');
  if (!form) throw new Error('form not found');
  return form;
}

async function openProfileTab() {
  await userEvent.click(screen.getByRole('tab', { name: S.settings.tabs.profile }));
}

describe('SettingsPage', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('validates the profile form', async () => {
    const { container } = renderPage();
    await openProfileTab();
    fireEvent.submit(getForm());
    expect(screen.getByText(S.settings.profile.errors.nameRequired)).toBeInTheDocument();
    expect(screen.getByText(S.settings.profile.errors.emailInvalid)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('saves a valid profile', () => {
    vi.useFakeTimers();
    renderPage();
    fireEvent.click(screen.getByRole('tab', { name: S.settings.tabs.profile }));
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