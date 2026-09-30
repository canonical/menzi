import { describe, expect, it, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RequireAnonymous, RequireAuth } from './RequireAuth';
import { useAuthStore } from '../stores/auth';
import { queryKeys, routes } from '../lib/routes';

function client() {
  return new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
}

function respond(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function signInOrOut(signedIn: boolean) {
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue(
      signedIn
        ? respond(200, { user: { id: 'u1', kind: 'user', email: 'a@b' }, csrf_token: 'c' })
        : respond(401, { error: 'no session' }),
    ),
  );
}

function protectedTree(entry: string) {
  return (
    <QueryClientProvider client={client()}>
      <MemoryRouter initialEntries={[entry]}>
        <Routes>
          <Route path="/login" element={<p>the login page</p>} />
          <Route element={<RequireAuth />}>
            <Route path="/private" element={<p>the private page</p>} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

function anonymousTree(entry: string) {
  return (
    <QueryClientProvider client={client()}>
      <MemoryRouter initialEntries={[entry]}>
        <Routes>
          <Route path="/projects" element={<p>the projects page</p>} />
          <Route element={<RequireAnonymous />}>
            <Route path="/login" element={<p>the login page</p>} />
          </Route>
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>
  );
}

beforeEach(() => {
  useAuthStore.setState({ user: null, recorded: false });
  signInOrOut(false);
});

describe('RequireAuth', () => {
  it('shows a spinner while the session is unknown', () => {
    render(protectedTree('/private'));
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  it('renders the page once the gateway reports a caller', async () => {
    signInOrOut(true);
    render(protectedTree('/private'));
    expect(await screen.findByText('the private page')).toBeInTheDocument();
  });

  it('sends an anonymous caller to the login page', async () => {
    render(protectedTree('/private'));
    expect(await screen.findByText('the login page')).toBeInTheDocument();
  });

  it('never renders the protected page for an anonymous caller', async () => {
    render(protectedTree('/private'));
    await screen.findByText('the login page');
    expect(screen.queryByText('the private page')).not.toBeInTheDocument();
  });

  it('records the caller the gateway resolved', async () => {
    signInOrOut(true);
    render(protectedTree('/private'));
    await screen.findByText('the private page');
    await waitFor(() => expect(useAuthStore.getState().user?.id).toBe('u1'));
  });

  it('records an anonymous caller as anonymous', async () => {
    render(protectedTree('/private'));
    await screen.findByText('the login page');
    await waitFor(() => expect(useAuthStore.getState().recorded).toBe(true));
    expect(useAuthStore.getState().user).toBeNull();
  });
});

describe('RequireAnonymous', () => {
  it('renders the login page for an anonymous caller', async () => {
    render(anonymousTree('/login'));
    expect(await screen.findByText('the login page')).toBeInTheDocument();
  });

  it('sends a signed in caller to the projects page', async () => {
    signInOrOut(true);
    render(anonymousTree('/login'));
    expect(await screen.findByText('the projects page')).toBeInTheDocument();
  });
});

describe('session query', () => {
  it('is a single query so one request answers for the whole app', () => {
    expect(queryKeys.session()).toEqual(['session']);
    expect(routes.auth.login()).toBe('/login');
    expect(routes.auth.register()).toBe('/register');
  });
});
