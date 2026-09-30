import { describe, expect, it, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { AccountSection } from './AccountSection';
import { useAuthStore } from '../../stores/auth';
import { S } from '../../strings/catalogue';

interface Call {
  url: string;
  method: string;
  body: unknown;
}

let calls: Call[];
let devices: {
  id: string;
  provider: string;
  created_at: string;
  last_seen_at: string;
  current: boolean;
  user_agent: string | null;
}[];
let changeFails: string | null;

function response(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function section() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={client}>
      <NotificationProvider>
        <AccountSection />
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  calls = [];
  changeFails = null;
  devices = [
    {
      id: 'd1',
      provider: 'password',
      created_at: '2026-01-01T00:00:00Z',
      last_seen_at: '2026-01-02T00:00:00Z',
      current: true,
      user_agent: 'Firefox',
    },
    {
      id: 'd2',
      provider: 'password',
      created_at: '2026-01-01T00:00:00Z',
      last_seen_at: '2026-01-03T00:00:00Z',
      current: false,
      user_agent: 'Safari',
    },
  ];
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string, options: RequestInit = {}) => {
      const method = options.method ?? 'GET';
      calls.push({
        url: String(url),
        method,
        body: options.body ? JSON.parse(options.body as string) : undefined,
      });
      if (String(url).includes('/auth/sessions') && method === 'GET') {
        return Promise.resolve(response(200, { devices }));
      }
      if (String(url).includes('/password/change')) {
        if (changeFails) {
          return Promise.resolve(
            response(401, { error: { code: 'invalid', message: changeFails } }),
          );
        }
        return Promise.resolve(response(200, { ok: true }));
      }
      if (String(url).includes('/auth/sessions/') && method === 'DELETE') {
        return Promise.resolve(response(200, { ok: true }));
      }
      return Promise.resolve(response(200, { ok: true }));
    }),
  );
  useAuthStore.setState({
    user: {
      id: 'u1',
      kind: 'user',
      email: 'ada@acme.example',
      has_password: true,
    },
    recorded: true,
  });
});

describe('AccountSection', () => {
  it('shows the signed in account', async () => {
    section();
    expect(await screen.findByTestId('account-email')).toHaveTextContent(
      'ada@acme.example',
    );
  });

  it('lists the devices this account is signed in on', async () => {
    section();
    expect(await screen.findByText(S.auth.thisDevice)).toBeInTheDocument();
    expect(screen.getByText('Safari')).toBeInTheDocument();
  });

  it('does not offer to sign out the device in use', async () => {
    section();
    await screen.findByText(S.auth.thisDevice);
    expect(screen.getAllByRole('button', { name: S.auth.revoke })).toHaveLength(1);
  });

  it('signs another device out', async () => {
    section();
    await screen.findByText('Safari');
    await userEvent.click(screen.getByRole('button', { name: S.auth.revoke }));
    await waitFor(() =>
      expect(
        calls.some((call) => call.url.endsWith('/auth/sessions/d2') && call.method === 'DELETE'),
      ).toBe(true),
    );
  });

  it('changes the password', async () => {
    section();
    await userEvent.type(
      await screen.findByLabelText(S.auth.currentPassword),
      'old-password-here',
    );
    await userEvent.type(
      screen.getByLabelText(S.auth.newPassword),
      'a-brand-new-password',
    );
    await userEvent.type(
      screen.getByLabelText(S.auth.confirmPassword),
      'a-brand-new-password',
    );
    await userEvent.click(screen.getByRole('button', { name: S.auth.saveNewPassword }));

    await waitFor(() =>
      expect(
        calls.some(
          (call) => call.url.includes('/password/change') && call.method === 'POST',
        ),
      ).toBe(true),
    );
    const sent = calls.find((call) => call.url.includes('/password/change'));
    expect(sent?.body).toEqual({
      current_password: 'old-password-here',
      new_password: 'a-brand-new-password',
    });
  });

  it('refuses a confirmation that does not match', async () => {
    section();
    await userEvent.type(await screen.findByLabelText(S.auth.currentPassword), 'old-one');
    await userEvent.type(screen.getByLabelText(S.auth.newPassword), 'a-brand-new-password');
    await userEvent.type(screen.getByLabelText(S.auth.confirmPassword), 'something-else-entirely');
    await userEvent.click(screen.getByRole('button', { name: S.auth.saveNewPassword }));

    expect(await screen.findByTestId('account-error')).toHaveTextContent(
      S.auth.passwordsDoNotMatch,
    );
    expect(calls.some((call) => call.url.includes('/password/change'))).toBe(false);
  });

  it('refuses a password that is too short', async () => {
    section();
    await userEvent.type(await screen.findByLabelText(S.auth.currentPassword), 'old-one');
    await userEvent.type(screen.getByLabelText(S.auth.newPassword), 'short');
    await userEvent.type(screen.getByLabelText(S.auth.confirmPassword), 'short');
    await userEvent.click(screen.getByRole('button', { name: S.auth.saveNewPassword }));

    expect(await screen.findByTestId('account-error')).toHaveTextContent(
      S.auth.passwordTooShort.replace('{minimum}', '12'),
    );
    expect(calls.some((call) => call.url.includes('/password/change'))).toBe(false);
  });

  it('shows the refusal when the server refuses', async () => {
    changeFails = 'the current password is wrong';
    section();
    await userEvent.type(await screen.findByLabelText(S.auth.currentPassword), 'wrong-one');
    await userEvent.type(
      screen.getByLabelText(S.auth.newPassword),
      'a-brand-new-password',
    );
    await userEvent.type(
      screen.getByLabelText(S.auth.confirmPassword),
      'a-brand-new-password',
    );
    await userEvent.click(screen.getByRole('button', { name: S.auth.saveNewPassword }));

    expect(await screen.findByTestId('account-error')).toHaveTextContent(
      'the current password is wrong',
    );
  });

  it('says so when the account has no password to change', async () => {
    useAuthStore.setState({
      user: { id: 'u1', kind: 'user', email: 'a@b', has_password: false },
      recorded: true,
    });
    section();
    expect(await screen.findByTestId('no-password')).toBeInTheDocument();
    expect(screen.queryByLabelText(S.auth.currentPassword)).not.toBeInTheDocument();
  });
});
