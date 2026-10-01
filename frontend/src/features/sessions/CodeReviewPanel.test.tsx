import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CodeReviewPanel } from './CodeReviewPanel';
import { S } from '../../strings/catalogue';

const USER = 'user-1';
const PROJECT = 'project-1';
const DIRECTORY = '/workspace';
const HEAD = 'abc123def456abc123def456abc123def456abcd';

function change(over: Partial<Record<string, unknown>> = {}) {
  return {
    file: 'src/a.rs',
    previous: null,
    additions: 1,
    deletions: 0,
    status: 'modified',
    binary: false,
    truncated: false,
    patch: '',
    ...over,
  };
}

const PATCH = '@@ -1,2 +1,2 @@\n a\n-b\n+c';

function mockDiff(byPath: Record<string, ReturnType<typeof change>>) {
  const fetchMock = vi.fn((input: RequestInfo | URL) => {
    const url = new URL(String(input), 'http://localhost');
    const path = url.searchParams.get('path');
    if (path === null) {
      return Promise.resolve(json({ head: HEAD, changes: Object.values(byPath) }));
    }
    const found = byPath[path];
    return Promise.resolve(json({ head: HEAD, changes: found ? [found] : [] }));
  });
  vi.stubGlobal('fetch', fetchMock);
}

function json(body: unknown) {
  return {
    ok: true,
    status: 200,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

function renderPanel() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <CodeReviewPanel userId={USER} projectId={PROJECT} directory={DIRECTORY} />
    </QueryClientProvider>,
  );
}

function list() {
  return within(screen.getByRole('list', { name: S.review.filesLabel }));
}

beforeEach(() => {
  vi.unstubAllGlobals();
});

describe('CodeReviewPanel from git', () => {
  it('lists exactly the files git reports', async () => {
    mockDiff({
      'src/a.rs': change(),
      'src/b.rs': change({ file: 'src/b.rs', status: 'untracked', additions: 12 }),
    });
    renderPanel();

    await waitFor(() => expect(list().getAllByRole('button')).toHaveLength(2));
    expect(list().getByRole('button', { name: 'src/a.rs' })).toBeInTheDocument();
    expect(list().getByRole('button', { name: 'src/b.rs' })).toBeInTheDocument();
  });

  it('shows the status git gave each file', async () => {
    mockDiff({
      'src/new.rs': change({ file: 'src/new.rs', status: 'untracked', additions: 4 }),
    });
    renderPanel();

    const row = await screen.findByRole('button', { name: 'src/new.rs' });
    expect(row.textContent).toContain('untracked');
    expect(row.textContent).toContain('+4');
  });

  it('never asks the session proxy for a diff', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json({ head: HEAD, changes: [change({ patch: PATCH })] }));
    });
    vi.stubGlobal('fetch', fetchMock);
    renderPanel();

    await screen.findByTestId('diff-viewer');
    const urls = fetchMock.mock.calls.map(([input]) => String(input));
    expect(urls.every((url) => !url.includes('/session/'))).toBe(true);
    expect(urls.every((url) => url.includes('/diff'))).toBe(true);
  });

  it('renders the patch git returned', async () => {
    mockDiff({ 'src/a.rs': change({ patch: PATCH }) });
    renderPanel();

    const viewer = await screen.findByTestId('diff-viewer');
    expect(viewer.textContent).toContain('-b');
    expect(viewer.textContent).toContain('+c');
  });

  it('renders an untracked file instead of saying there is no patch', async () => {
    const untracked = PATCH.replace('@@ -1,2 +1,2 @@', '@@ -0,0 +1,2 @@');
    mockDiff({
      'src/new.rs': change({ file: 'src/new.rs', status: 'untracked', additions: 2, patch: untracked }),
    });
    renderPanel();

    const viewer = await screen.findByTestId('diff-viewer');
    expect(viewer.textContent).toContain('+c');
    expect(screen.queryByText(S.review.noPatch)).not.toBeInTheDocument();
  });

  it('says so when a file is binary rather than showing an empty viewer', async () => {
    mockDiff({
      'logo.png': change({ file: 'logo.png', binary: true, additions: 0, deletions: 0 }),
    });
    renderPanel();

    expect(await screen.findByText(S.review.binaryFile)).toBeInTheDocument();
    expect(screen.queryByTestId('diff-viewer')).not.toBeInTheDocument();
  });

  it('warns when the patch was too large to send whole', async () => {
    mockDiff({ 'big.rs': change({ file: 'big.rs', truncated: true, patch: PATCH }) });
    renderPanel();

    expect(await screen.findByText(S.review.patchTruncated)).toBeInTheDocument();
  });

  it('names the commit it is comparing against', async () => {
    mockDiff({ 'src/a.rs': change({ patch: PATCH }) });
    renderPanel();

    const base = await screen.findByTestId('review-base');
    expect(base.textContent).toContain(HEAD.slice(0, 12));
  });

  it('switches the viewer when another file is picked', async () => {
    mockDiff({
      'src/a.rs': change({ patch: PATCH }),
      'src/b.rs': change({ file: 'src/b.rs', patch: '@@ -9,1 +9,1 @@\n-x\n+y' }),
    });
    const user = userEvent.setup();
    renderPanel();

    await screen.findByTestId('diff-viewer');
    await user.click(list().getByRole('button', { name: 'src/b.rs' }));

    await waitFor(() => {
      expect(screen.getByTestId('diff-viewer').textContent).toContain('+y');
    });
  });

  it('shows the empty state when git reports nothing', async () => {
    mockDiff({});
    renderPanel();

    expect(await screen.findByText(S.review.emptyTitle)).toBeInTheDocument();
  });

  it('surfaces a failure reading git', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() =>
        Promise.resolve({
          ok: false,
          status: 500,
          statusText: 'Server Error',
          text: async () => JSON.stringify({ error: { message: 'git failed' } }),
          json: async () => ({}),
        }),
      ),
    );
    renderPanel();

    expect(await screen.findByText('git failed')).toBeInTheDocument();
  });
});