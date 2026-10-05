import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { ProjectPage } from './ProjectPage';
import { axeOptions } from '../../testing/axe';

const PROJECT = {
  id: 'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  name: 'Storefront',
  slug: 'storefront',
  description: 'Customer facing app',
  repository_url: 'git@github.com:acme/storefront.git',
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

function mockApi(projectStatus: number, projectBody: unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn(() => {
      return Promise.resolve(jsonResponse(projectStatus, projectBody));
    }),
  );
}

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter initialEntries={[`/projects/${PROJECT.id}`]}>
        <Routes>
          <Route path="/projects/:projectId" element={<ProjectPage />} />
        </Routes>
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('ProjectPage', () => {
  it('shows the project and links to previews', async () => {
    mockApi(200, PROJECT);
    const { container } = renderPage();
    expect(
      await screen.findByRole('heading', { name: 'Storefront' }),
    ).toBeInTheDocument();
    expect(screen.getByText('Customer facing app')).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: /Previews/ }),
    ).toHaveAttribute('href', `/projects/${PROJECT.id}/previews`);
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows a retryable error when the project is missing', async () => {
    mockApi(404, { error: 'project not found' });
    renderPage();
    expect(await screen.findByText('project not found')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });
});
