import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NotificationProvider } from '@canonical/react-components';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../../testing/axe';
import { PreviewsPage } from './PreviewsPage';
import { S } from '../../strings/catalogue';

const previews = [
  {
    id: 'prv-1',
    project_id: 'proj-1',
    commit_sha: 'abc123',
    branch: 'main',
    status: 'ready',
    mode: 'pinned',
    url: 'http://preview.dev',
    created_at: '2026-01-01T12:00:00Z',
  },
  {
    id: 'prv-2',
    project_id: 'proj-1',
    commit_sha: 'def456',
    status: 'starting',
    mode: 'live',
    url: '',
    created_at: '2026-01-02T12:00:00Z',
  },
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

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

function renderPage() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <NotificationProvider>
        <MemoryRouter initialEntries={['/projects/proj-1/previews']}>
          <Routes>
            <Route path="/projects/:projectId/previews" element={<PreviewsPage />} />
          </Routes>
        </MemoryRouter>
      </NotificationProvider>
    </QueryClientProvider>,
  );
}

describe('PreviewsPage', () => {
  it('lists previews in the table', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(200, previews)));
    const { container } = renderPage();
    expect(await screen.findByText(S.previews.modes.pinned)).toBeInTheDocument();
    expect(screen.getByText(S.previews.modes.live)).toBeInTheDocument();
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText(S.previews.branchDefault)).toBeInTheDocument();
    expect(screen.getByText(S.previews.statuses.ready)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows the empty state when there are no previews', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(200, [])));
    const { container } = renderPage();
    expect(await screen.findByText(S.previews.emptyTitle)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });

  it('shows a retryable error state when the request fails', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(jsonResponse(500, { error: { message: 'Previews are unavailable' } })));
    renderPage();
    expect(await screen.findByText('Previews are unavailable')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: S.dataState.retry }),
    ).toBeInTheDocument();
  });
});