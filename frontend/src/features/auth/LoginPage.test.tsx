import { describe, expect, it, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { navigateTo } from '../../lib/navigation';
import { LoginPage } from './LoginPage';
import { RegisterPage } from './RegisterPage';
import { S } from '../../strings/catalogue';

function client() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
}

interface Call {
  url: string;
  method: string;
  body: unknown;
}

vi.mock('../../lib/navigation', () => ({ navigateTo: vi.fn() }));

let calls: Call[];
let providers: { registration: string; password: boolean; oidc: { id: string; label: string }[] };
let failures: Record<string, { status: number; message: string }>;

function response(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function page(node: React.ReactNode, entry = '/login', state?: unknown) {
  return render(
    <QueryClientProvider client={client()}>
      <MemoryRouter initialEntries={[{ pathname: entry, state }]}>{node}</MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  calls = [];
  failures = {};
  providers = { registration: 'closed', password: true, oidc: [] };
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string, options: RequestInit = {}) => {
      const path = String(url);
      const method = options.method ?? 'GET';
      calls.push({
        url: path,
        method,
        body: options.body ? JSON.parse(options.body as string) : undefined,
      });
      if (path.includes('/auth/providers')) return Promise.resolve(response(200, providers));
      const failure = failures[`${method} ${path}`];
      if (failure) {
        return Promise.resolve(
          response(failure.status, { error: { code: 'invalid', message: failure.message } }),
        );
      }
      return Promise.resolve(response(200, { ok: true }));
    }),
  );
});

describe('LoginPage', () => {
  it('renders the email and password fields', async () => {
    page(<LoginPage />);
    expect(await screen.findByLabelText(S.auth.email)).toBeInTheDocument();
    expect(screen.getByLabelText(S.auth.password)).toBeInTheDocument();
  });

  it('signs in and returns to the page that was asked for', async () => {
    page(<LoginPage />, '/login', { from: '/projects/abc/code' });
    await userEvent.type(await screen.findByLabelText(S.auth.email), 'a@b');
    await userEvent.type(screen.getByLabelText(S.auth.password), 'secret');
    await userEvent.click(screen.getByRole('button', { name: S.auth.signIn }));

    await waitFor(() =>
      expect(calls.some((call) => call.url === '/api/v1/auth/login')).toBe(true),
    );
    expect(vi.mocked(navigateTo)).toHaveBeenCalledWith('/projects/abc/code');
  });

  it('shows the server message in the form when a sign in fails', async () => {
    failures['POST /api/v1/auth/login'] = {
      status: 401,
      message: 'the email or password is wrong',
    };
    page(<LoginPage />);
    await userEvent.type(await screen.findByLabelText(S.auth.email), 'a@b');
    await userEvent.type(screen.getByLabelText(S.auth.password), 'nope');
    await userEvent.click(screen.getByRole('button', { name: S.auth.signIn }));

    expect(
      await screen.findByTestId('auth-error', {}, { timeout: 5000 }),
    ).toHaveTextContent('the email or password is wrong');
  });

  it('offers no create-account link when registration is closed', async () => {
    providers = { registration: 'closed', password: true, oidc: [] };
    page(<LoginPage />);
    await screen.findByLabelText(S.auth.email);
    expect(screen.queryByRole('link', { name: S.auth.createAccount })).not.toBeInTheDocument();
  });

  it('offers a create-account link when registration is open', async () => {
    providers = { registration: 'open', password: true, oidc: [] };
    page(<LoginPage />);
    expect(
      await screen.findByRole('link', { name: S.auth.createAccount }),
    ).toBeInTheDocument();
  });

  it('renders no provider buttons when none is configured', async () => {
    page(<LoginPage />);
    await screen.findByLabelText(S.auth.email);
    expect(screen.queryByText(/Continue with/)).not.toBeInTheDocument();
  });

  it('renders a button for each configured provider', async () => {
    providers = {
      registration: 'closed',
      password: true,
      oidc: [{ id: 'google', label: 'Google' }],
    };
    page(<LoginPage />);
    const button = await screen.findByRole('link', { name: 'Continue with Google' });
    expect(button).toHaveAttribute(
      'href',
      '/api/v1/auth/oidc/google/start?redirect_to=%2Fprojects',
    );
  });

  it('links to the password reset page', async () => {
    page(<LoginPage />);
    expect(
      await screen.findByRole('link', { name: S.auth.forgotPassword }),
    ).toHaveAttribute('href', '/forgot-password');
  });
});

describe('RegisterPage', () => {
  it('says so when the deployment is not accepting accounts', async () => {
    providers = { registration: 'closed', password: true, oidc: [] };
    page(<RegisterPage />);
    expect(await screen.findByTestId('registration-closed')).toBeInTheDocument();
  });

  it('refuses a password that is too short', async () => {
    providers = { registration: 'open', password: true, oidc: [] };
    page(<RegisterPage />);
    await userEvent.type(await screen.findByLabelText(S.auth.name), 'A Person');
    await userEvent.type(screen.getByLabelText(S.auth.email), 'a@b');
    await userEvent.type(screen.getByLabelText(S.auth.password), 'short');
    await userEvent.type(screen.getByLabelText(S.auth.confirmPassword), 'short');
    await userEvent.click(screen.getByRole('button', { name: S.auth.createAccount }));

    expect(await screen.findByTestId('auth-error')).toHaveTextContent(
      S.auth.passwordTooShort.replace('{minimum}', '12'),
    );
    expect(calls.some((call) => call.url === '/api/v1/auth/register')).toBe(false);
  });

  it('registers and lands on the projects page', async () => {
    providers = { registration: 'open', password: true, oidc: [] };
    page(<RegisterPage />);
    await userEvent.type(await screen.findByLabelText(S.auth.name), 'A Person');
    await userEvent.type(screen.getByLabelText(S.auth.email), 'a@b');
    await userEvent.type(screen.getByLabelText(S.auth.password), 'a-long-enough-password');
    await userEvent.type(
      screen.getByLabelText(S.auth.confirmPassword),
      'a-long-enough-password',
    );
    await userEvent.click(screen.getByRole('button', { name: S.auth.createAccount }));

    await waitFor(() =>
      expect(
        calls.some((call) => call.url === '/api/v1/auth/register' && call.method === 'POST'),
      ).toBe(true),
    );
    expect(vi.mocked(navigateTo)).toHaveBeenCalledWith('/projects');
  });

  it('refuses a confirmation that does not match', async () => {
    providers = { registration: 'open', password: true, oidc: [] };
    page(<RegisterPage />);
    await userEvent.type(await screen.findByLabelText(S.auth.name), 'A Person');
    await userEvent.type(screen.getByLabelText(S.auth.email), 'a@b');
    await userEvent.type(screen.getByLabelText(S.auth.password), 'a-long-enough-password');
    await userEvent.type(screen.getByLabelText(S.auth.confirmPassword), 'different-password');
    await userEvent.click(screen.getByRole('button', { name: S.auth.createAccount }));

    expect(await screen.findByTestId('auth-error')).toHaveTextContent(
      S.auth.passwordsDoNotMatch,
    );
    expect(calls.some((call) => call.url === '/api/v1/auth/register')).toBe(false);
  });
});
