import { describe, expect, it, beforeEach, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter } from 'react-router-dom';
import { ProjectsPage } from './ProjectsPage';
import { S } from '../../strings/catalogue';

let orgs: { id: string; name: string; slug: string }[];
let projects: { id: string; org_id: string; name: string; slug: string; updated_at: string }[];

function response(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function page() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={client}>
      <NotificationProvider>
        <MemoryRouter>
          <ProjectsPage />
        </MemoryRouter>
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

beforeEach(() => {
  orgs = [{ id: 'o1', name: 'Acme', slug: 'acme' }];
  projects = [
    {
      id: 'p1',
      org_id: 'o1',
      name: 'Storefront',
      slug: 'storefront',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ];
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL, init: RequestInit = {}) => {
      const url = String(input);
      if (url.includes('/api/v1/orgs')) {
        if (init.method === 'POST') {
          orgs = [{ id: 'o2', name: 'New Company', slug: 'new-company' }];
          return Promise.resolve(response(201, orgs[0]));
        }
        return Promise.resolve(response(200, orgs));
      }
      if (url.includes('/api/v1/projects')) return Promise.resolve(response(200, projects));
      return Promise.resolve(response(404, { error: 'no route' }));
    }),
  );
});

describe('ProjectsPage', () => {
  it('lists projects for an account that has an organisation', async () => {
    page();
    expect(await screen.findByText('Storefront')).toBeInTheDocument();
    expect(screen.queryByTestId('no-org')).not.toBeInTheDocument();
  });

  it('offers to create an organisation when there is none', async () => {
    orgs = [];
    projects = [];
    page();
    expect(await screen.findByTestId('no-org')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: S.projectsFirstOrg.firstOrgTitle })).toBeInTheDocument();
  });

  it('suggests a slug from the organisation name', async () => {
    orgs = [];
    projects = [];
    page();
    await screen.findByTestId('no-org');
    await userEvent.type(screen.getByLabelText(S.projectsFirstOrg.orgName), 'New Company');
    expect(screen.getByLabelText(S.projectsFirstOrg.orgSlug)).toHaveValue('new-company');
  });

  it('keeps a slug the user typed', async () => {
    orgs = [];
    projects = [];
    page();
    await screen.findByTestId('no-org');
    await userEvent.type(screen.getByLabelText(S.projectsFirstOrg.orgName), 'New Company');
    await userEvent.clear(screen.getByLabelText(S.projectsFirstOrg.orgSlug));
    await userEvent.type(screen.getByLabelText(S.projectsFirstOrg.orgSlug), 'chosen');
    await userEvent.type(screen.getByLabelText(S.projectsFirstOrg.orgName), ' Ltd');
    expect(screen.getByLabelText(S.projectsFirstOrg.orgSlug)).toHaveValue('chosen');
  });

  it('refuses to submit without a name', async () => {
    orgs = [];
    projects = [];
    page();
    await screen.findByTestId('no-org');
    await userEvent.click(screen.getByRole('button', { name: S.projectsFirstOrg.createOrg }));
    expect(screen.getByTestId('no-org')).toBeInTheDocument();
  });

  it('shows the projects page once the organisation exists', async () => {
    orgs = [];
    projects = [];
    page();
    await screen.findByTestId('no-org');
    await userEvent.type(screen.getByLabelText(S.projectsFirstOrg.orgName), 'New Company');
    await userEvent.click(screen.getByRole('button', { name: S.projectsFirstOrg.createOrg }));
    await waitFor(() => expect(screen.queryByTestId('no-org')).not.toBeInTheDocument());
  });
});
