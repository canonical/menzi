import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { ProjectsPage } from './ProjectsPage';
import { axeOptions } from '../../testing/axe';
import { S } from '../../strings/catalogue';

const PROJECTS = [
  {
    id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
    org_id: 'org-1',
    name: 'Storefront',
    slug: 'storefront',
    description: 'Customer facing app',
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-02T00:00:00Z',
  },
  {
    id: 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
    org_id: 'org-2',
    name: 'Billing Service',
    slug: 'billing-service',
    description: null,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-03T00:00:00Z',
  },
];

const ORGS = [
  { id: 'org-1', name: 'Acme Corp', slug: 'acme' },
  { id: 'org-2', name: 'Globex', slug: 'globex' },
];

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

type Handler = () => ReturnType<typeof jsonResponse>;

function mockApi(handlers: Record<string, Handler>) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? 'GET';
      const key =
        method === 'POST' && url.includes('/api/v1/projects')
          ? 'POST /api/v1/projects'
          : Object.keys(handlers).find((candidate) => url.includes(candidate));
      if (!key || !handlers[key]) {
        return Promise.resolve(jsonResponse(404, { error: 'no route' }));
      }
      return Promise.resolve(handlers[key]());
    }),
  );
}

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

const defaultHandlers = (): Record<string, Handler> => ({
  '/api/v1/orgs': () => jsonResponse(200, ORGS),
  '/api/v1/projects': () => jsonResponse(200, PROJECTS),
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('ProjectsPage', () => {
  it('lists projects with their org names', async () => {
    mockApi(defaultHandlers());
    const { container } = renderPage();
    expect(await screen.findByText('Storefront')).toBeInTheDocument();
    expect(screen.getByText('Billing Service')).toBeInTheDocument();
    const table = within(screen.getByRole('grid'));
    expect(table.getByText('Acme Corp')).toBeInTheDocument();
    expect(table.getByText('Globex')).toBeInTheDocument();
    expect(table.getByText('storefront')).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows an empty state when there are no projects', async () => {
    mockApi({
      '/api/v1/orgs': () => jsonResponse(200, ORGS),
      '/api/v1/projects': () => jsonResponse(200, []),
    });
    renderPage();
    expect(await screen.findByText(S.projects.emptyTitle)).toBeInTheDocument();
  });

  it('re-queries with the search term', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, ORGS));
      }
      return Promise.resolve(jsonResponse(200, url.includes('search=bill') ? [PROJECTS[1]] : PROJECTS));
    });
    vi.stubGlobal('fetch', fetchMock);
    renderPage();
    await screen.findByText('Storefront');

    fireEvent.change(screen.getByLabelText(S.projects.searchLabel), {
      target: { value: 'bill' },
    });

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some((call) => String(call[0]).includes('search=bill')),
      ).toBe(true);
    });
    expect(await screen.findByText('Billing Service')).toBeInTheDocument();
  });

  it('validates the create form before calling the api', async () => {
    mockApi(defaultHandlers());
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    const confirmButtons = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(confirmButtons[confirmButtons.length - 1]);

    expect(await screen.findByText(S.projects.errors.nameRequired)).toBeInTheDocument();
    expect(screen.queryByText(S.projects.errors.orgRequired)).not.toBeInTheDocument();
  });

  it('preselects the only org so it does not have to be chosen', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, [ORGS[0]]));
      }
      return Promise.resolve(jsonResponse(200, [PROJECTS[0]]));
    });
    vi.stubGlobal('fetch', fetchMock);
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    expect(await screen.findByLabelText(S.projects.orgLabel)).toHaveValue('org-1');
  });

  it('creates a project with a single org without touching the dropdown', async () => {
    const posted: unknown[] = [];
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, [ORGS[0]]));
      }
      if ((init?.method ?? 'GET') === 'POST') {
        posted.push(JSON.parse(String(init?.body)));
        return Promise.resolve(jsonResponse(201, PROJECTS[0]));
      }
      return Promise.resolve(jsonResponse(200, [PROJECTS[0]]));
    });
    vi.stubGlobal('fetch', fetchMock);
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    fireEvent.change(await screen.findByLabelText(S.projects.nameLabel), {
      target: { value: 'Payments' },
    });

    const confirmButtons = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(posted).toHaveLength(1));
    expect(posted[0]).toMatchObject({ org_id: 'org-1', name: 'Payments' });
  });

  it('preselects the first org when there are several', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, ORGS));
      }
      return Promise.resolve(jsonResponse(200, [PROJECTS[0]]));
    });
    vi.stubGlobal('fetch', fetchMock);
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    expect(await screen.findByLabelText(S.projects.orgLabel)).toHaveValue('org-1');
  });

  it('still lets a second org be chosen', async () => {
    const posted: unknown[] = [];
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, ORGS));
      }
      if ((init?.method ?? 'GET') === 'POST') {
        posted.push(JSON.parse(String(init?.body)));
        return Promise.resolve(jsonResponse(201, PROJECTS[0]));
      }
      return Promise.resolve(jsonResponse(200, [PROJECTS[0]]));
    });
    vi.stubGlobal('fetch', fetchMock);
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    fireEvent.change(await screen.findByLabelText(S.projects.nameLabel), {
      target: { value: 'Payments' },
    });
    fireEvent.change(screen.getByLabelText(S.projects.orgLabel), {
      target: { value: 'org-2' },
    });

    const confirmButtons = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => expect(posted).toHaveLength(1));
    expect(posted[0]).toMatchObject({ org_id: 'org-2' });
  });

  it('creates a project and refreshes the list', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        return Promise.resolve(jsonResponse(200, ORGS));
      }
      if ((init?.method ?? 'GET') === 'POST') {
        return Promise.resolve(jsonResponse(201, PROJECTS[0]));
      }
      return Promise.resolve(jsonResponse(200, [PROJECTS[0]]));
    });
    vi.stubGlobal('fetch', fetchMock);
    const user = userEvent.setup();
    renderPage();
    await screen.findByText('Storefront');

    const [openButton] = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(openButton);

    fireEvent.change(await screen.findByLabelText(S.projects.nameLabel), {
      target: { value: 'Payments' },
    });
    fireEvent.change(screen.getByLabelText(S.projects.orgLabel), {
      target: { value: 'org-1' },
    });

    const confirmButtons = await screen.findAllByRole('button', { name: S.projects.create });
    await user.click(confirmButtons[confirmButtons.length - 1]);

    await waitFor(() => {
      expect(
        fetchMock.mock.calls.some((call) => (call[1] as RequestInit | undefined)?.method === 'POST'),
      ).toBe(true);
    });
  });
});
