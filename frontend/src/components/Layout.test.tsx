import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { Layout } from './Layout';
import { axeOptions } from '../testing/axe';
import { useActiveProject } from '../stores/activeProject';
import { S } from '../strings/catalogue';

const PROJECT = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  org_id: 'org-1',
  name: 'Storefront',
  slug: 'storefront',
  description: null,
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
      return Promise.resolve(jsonResponse(200, []));
    }),
  );
  useActiveProject.setState({ projectId: '', section: 'code' });
});

describe('Layout', () => {
  it('renders the application shell with the top level navigation', async () => {
    const { container } = renderLayout();
    expect(screen.getByText(S.nav.projects)).toBeInTheDocument();
    expect(screen.getByText(S.nav.needsYou)).toBeInTheDocument();
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
});
