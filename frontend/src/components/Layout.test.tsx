import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { Layout } from './Layout';
import { axeOptions } from '../testing/axe';
import { useActiveProject } from '../stores/activeProject';
import { useAuthStore } from '../stores/auth';
import { S } from '../strings/catalogue';

const PROJECT = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  name: 'Storefront',
  slug: 'storefront',
  description: null,
  repository_url: null,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-02T00:00:00Z',
};

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function renderLayout(path = '/projects') {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[path]}>
        <NotificationProvider>
          <Layout>
            <div>page content</div>
          </Layout>
        </NotificationProvider>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/api/v1/projects')) {
        return Promise.resolve(jsonResponse(200, [PROJECT]));
      }
      if (url.includes('/api/v1/me')) {
        return Promise.resolve(
          jsonResponse(200, { id: 'u1', kind: 'user', email: 'a@b.c', name: 'Ada' }),
        );
      }
      if (url.includes('/api/v1/auth/session')) {
        return Promise.resolve(
          jsonResponse(200, {
            user: { id: 'u1', kind: 'user', email: 'a@b.c', name: 'Ada' },
          }),
        );
      }
      return Promise.resolve(jsonResponse(200, []));
    }),
  );
  useActiveProject.setState({ projectId: '', section: 'code' });
  useAuthStore.getState().setSession({ id: 'u1', kind: 'user', email: 'a@b.c', name: 'Ada' });
  localStorage.clear();
});

describe('Layout', () => {
  it('renders the application shell with the top level navigation', async () => {
    const { container } = renderLayout();
    expect(screen.getByText(S.nav.projects)).toBeInTheDocument();
    expect(screen.getByText(S.nav.settings)).toBeInTheDocument();
    expect(screen.getByText('page content')).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows the active project on the selector toggle', async () => {
    useActiveProject.setState({ projectId: PROJECT.id, section: 'code' });
    renderLayout(`/projects/${PROJECT.id}/code`);
    expect(await screen.findByText(S.projects.selector.label)).toBeInTheDocument();
    expect(await screen.findByText('Storefront')).toBeInTheDocument();
  });

  it('shows the project sections once a project is active', async () => {
    useActiveProject.setState({ projectId: PROJECT.id, section: 'code' });
    renderLayout(`/projects/${PROJECT.id}/code`);
    expect(screen.getByText(S.sections.code)).toBeInTheDocument();
    expect(screen.getByText(S.sections.design)).toBeInTheDocument();
    expect(screen.getByText(S.sections.project)).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: S.sections.design }),
    ).toHaveAttribute('href', `/projects/${PROJECT.id}/design`);
  });

  it('keeps the theme control in the navigation and drops the status bar', async () => {
    const { container } = renderLayout();
    const control = await screen.findByRole('button', { name: S.theme.system });
    const nav = container.querySelector('.l-navigation');

    expect(nav).toContainElement(control);
    expect(container.querySelector('.l-status')).toBeNull();
  });

  it('puts the sign out control in the navigation menu', async () => {
    const { container } = renderLayout();
    const signOut = await screen.findByRole('button', { name: S.app.signOut });
    const nav = container.querySelector('.l-navigation');

    expect(nav).toContainElement(signOut);
  });

  it('keeps the sign out control in the same navigation when collapsed', async () => {
    const user = userEvent.setup();
    const { container } = renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));

    const nav = container.querySelector('.l-navigation');
    expect(nav).toHaveClass('app-navigation--collapsed');
    expect(nav).toContainElement(screen.getByRole('button', { name: S.app.signOut }));
  });

  it('collapses the navigation to icon width without unmounting it', async () => {
    const user = userEvent.setup();
    const { container } = renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));

    expect(container.querySelector('.l-navigation')).toBeInTheDocument();
    expect(container.querySelector('.l-navigation')).toHaveClass('app-navigation--collapsed');
    expect(container.querySelector('.app-collapsed-bar')).toBeNull();
    expect(container.querySelector('.app-drawer')).toBeNull();
    expect(container.querySelector('.app-content--rail')).toBeNull();
  });

  it('keeps destinations reachable by their accessible names when collapsed', async () => {
    const user = userEvent.setup();
    const { container } = renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));

    const nav = container.querySelector('.l-navigation');
    expect(nav).toHaveClass('app-navigation--collapsed');
    expect(screen.getByRole('link', { name: S.nav.projects })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: S.nav.settings })).toBeInTheDocument();
  });

  it('expands the navigation from the collapsed state', async () => {
    const user = userEvent.setup();
    const { container } = renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));
    await user.click(screen.getByRole('button', { name: S.nav.showNavigation }));

    expect(container.querySelector('.l-navigation')).toBeInTheDocument();
    expect(container.querySelector('.l-navigation')).not.toHaveClass('app-navigation--collapsed');
    expect(container.querySelector('.app-content--rail')).toBeNull();
  });

  it('offers the show-navigation control while collapsed', async () => {
    const user = userEvent.setup();
    renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));

    expect(screen.getByRole('button', { name: S.nav.showNavigation })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: S.nav.hideNavigation })).not.toBeInTheDocument();
  });

  it('starts with the navigation open again after a remount', async () => {
    const user = userEvent.setup();
    const { unmount } = renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));
    unmount();
    const { container } = renderLayout();

    expect(container.querySelector('.app-collapsed-bar')).toBeNull();
    expect(container.querySelector('.l-navigation')).toBeInTheDocument();
  });

  it('keeps nothing about the collapse in storage', async () => {
    const user = userEvent.setup();
    renderLayout();

    await user.click(await screen.findByRole('button', { name: S.nav.hideNavigation }));

    const stored = Object.keys(localStorage).filter((key) => key.includes('nav'));
    expect(stored).toEqual([]);
  });
});
