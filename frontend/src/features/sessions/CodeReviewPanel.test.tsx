import { render, screen, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CodeReviewPanel } from './CodeReviewPanel';
import { S } from '../../strings/catalogue';

const SESSION = 'ses_abc123';

const VCS = [
  { file: 'src/main.rs', status: 'modified', additions: 12, deletions: 3 },
  { file: 'src/new.rs', status: 'added', additions: 40, deletions: 0 },
];

const DIFF = [
  {
    path: 'src/main.rs',
    additions: 2,
    deletions: 1,
    hunks: [
      {
        header: '@@ -1,3 +1,4 @@',
        lines: [
          { kind: 'context', text: 'fn main() {' },
          { kind: 'remove', text: '    old();' },
          { kind: 'add', text: '    setup();' },
          { kind: 'add', text: '    run();' },
        ],
      },
    ],
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

function mockApi(vcs: unknown, diff: unknown) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/vcs/status')) return Promise.resolve(jsonResponse(200, vcs));
      if (url.includes('/diff')) return Promise.resolve(jsonResponse(200, diff));
      return Promise.resolve(jsonResponse(404, { error: 'no route' }));
    }),
  );
}

function renderPanel() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, gcTime: 0 } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <CodeReviewPanel sessionId={SESSION} />
    </QueryClientProvider>,
  );
}

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('CodeReviewPanel', () => {
  it('lists changed files and renders the diff for the first file', async () => {
    mockApi(VCS, DIFF);
    const { container } = renderPanel();
    const table = within(await screen.findByRole('grid'));
    expect(table.getByText('src/main.rs')).toBeInTheDocument();
    expect(table.getByText('src/new.rs')).toBeInTheDocument();
    expect(table.getByText('modified')).toBeInTheDocument();
    expect(table.getByText('added')).toBeInTheDocument();
    await screen.findByText('fn main() {');
    expect(container.textContent).toContain('+    run();');
    expect(container.textContent).toContain('-    old();');
  });

  it('shows an empty state when the working tree is clean', async () => {
    mockApi([], []);
    renderPanel();
    expect(await screen.findByText(S.review.emptyTitle)).toBeInTheDocument();
  });

  it('surfaces a failure reading the diff', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        if (url.includes('/vcs/status')) {
          return Promise.resolve(jsonResponse(200, VCS));
        }
        return Promise.resolve(jsonResponse(500, { error: 'diff unavailable' }));
      }),
    );
    renderPanel();
    expect(await screen.findByTestId('review-diff-error')).toHaveTextContent(
      'diff unavailable',
    );
  });
});
