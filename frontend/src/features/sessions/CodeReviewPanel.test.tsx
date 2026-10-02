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
const VERSION = 'v123';

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
      return Promise.resolve(json({ head: HEAD, version: VERSION, changes: Object.values(byPath) }));
    }
    const found = byPath[path];
    return Promise.resolve(json({ head: HEAD, version: VERSION, changes: found ? [found] : [] }));
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
  return within(screen.getByRole('tree', { name: S.review.filesLabel }));
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

    await screen.findByRole('treeitem', { name: 'src/a.rs' });
    expect(list().getAllByRole('treeitem').filter((item) => item.hasAttribute('aria-selected'))).toHaveLength(2);
    expect(list().getByRole('treeitem', { name: 'src/a.rs' })).toBeInTheDocument();
    expect(list().getByRole('treeitem', { name: 'src/b.rs' })).toBeInTheDocument();
    expect(screen.getByText(S.review.filesCount.replace('{count}', '2'))).toBeInTheDocument();
    expect(screen.queryByText(S.review.title)).not.toBeInTheDocument();
    expect(screen.queryByTestId('review-base')).not.toBeInTheDocument();
  });

  it('shows a compact filename without counts and status', async () => {
    mockDiff({
      'src/new.rs': change({ file: 'src/new.rs', status: 'untracked', additions: 4 }),
    });
    renderPanel();

    const row = await screen.findByRole('treeitem', { name: 'src/new.rs' });
    expect(row.textContent).toContain('new.rs');
    expect(row.textContent).not.toContain('untracked');
    expect(row.textContent).not.toContain('+4');
  });

  it('never asks the session proxy for a diff', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      void input;
      return Promise.resolve(json({ head: HEAD, version: VERSION, changes: [change({ patch: PATCH })] }));
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

  it('keeps only the change summary in the panel header', async () => {
    mockDiff({ 'src/a.rs': change({ patch: PATCH }) });
    renderPanel();

    await screen.findByTestId('diff-viewer');
    expect(screen.getByText(S.review.filesCount.replace('{count}', '1'))).toBeInTheDocument();
    expect(screen.queryByText(S.review.title)).not.toBeInTheDocument();
    expect(screen.queryByTestId('review-base')).not.toBeInTheDocument();
  });

  it('switches the viewer when another file is picked', async () => {
    mockDiff({
      'src/a.rs': change({ patch: PATCH }),
      'src/b.rs': change({ file: 'src/b.rs', patch: '@@ -9,1 +9,1 @@\n-x\n+y' }),
    });
    const user = userEvent.setup();
    renderPanel();

    await screen.findByTestId('diff-viewer');
    await user.click(list().getByRole('treeitem', { name: 'src/b.rs' }));

    await waitFor(() => {
      expect(screen.getByTestId('diff-viewer').textContent).toContain('+y');
    });
  });

  it('keeps the file navigator visible while navigating between files', async () => {
    mockDiff({
      'src/a.rs': change({ patch: PATCH }),
      'src/b.rs': change({ file: 'src/b.rs', patch: '@@ -1,1 +1,1 @@\n-old\n+new' }),
    });
    const user = userEvent.setup();
    renderPanel();
    await screen.findByTestId('diff-viewer');
    expect(screen.getByRole('button', { name: S.review.previousFile })).toHaveAttribute('aria-disabled', 'true');
    expect(screen.getByRole('tree', { name: S.review.filesLabel })).toBeVisible();
    expect(screen.queryByRole('button', { name: S.review.hideFiles })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: S.review.nextFile }));
    await waitFor(() => expect(screen.getByTestId('diff-viewer')).toHaveAttribute('aria-label', 'src/b.rs'));
    expect(screen.getByRole('button', { name: S.review.nextFile })).toHaveAttribute('aria-disabled', 'true');
    await user.click(screen.getByRole('button', { name: S.review.previousFile }));
    await waitFor(() => expect(screen.getByTestId('diff-viewer')).toHaveAttribute('aria-label', 'src/a.rs'));
    expect(screen.getByRole('tree', { name: S.review.filesLabel })).toBeVisible();
  });

  it('allows changing diff mode and always wraps lines', async () => {
    mockDiff({ 'src/a.rs': change({ patch: PATCH }) });
    const user = userEvent.setup();
    renderPanel();
    await screen.findByTestId('diff-viewer');
    await user.selectOptions(screen.getByRole('combobox', { name: S.review.viewMode }), 'split');
    expect(screen.getByTestId('diff-viewer')).toHaveClass('app-diff--split');
    await user.selectOptions(screen.getByRole('combobox', { name: S.review.viewMode }), 'unified');
    expect(screen.getByTestId('diff-viewer')).toHaveClass('app-diff--unified');
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(screen.getByTestId('diff-viewer')).toHaveClass('app-diff--wrap');
  });

  it('shows a patch loading state instead of an empty patch', async () => {
    vi.stubGlobal('fetch', vi.fn((input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost');
      if (url.searchParams.has('path')) return new Promise(() => {});
      return Promise.resolve(json({ head: HEAD, version: VERSION, changes: [change()] }));
    }));
    renderPanel();
    expect(await screen.findByText(S.review.patchLoading)).toBeInTheDocument();
    expect(screen.queryByText(S.review.noPatch)).not.toBeInTheDocument();
  });

  it('retries a failed patch request', async () => {
    let failed = true;
    vi.stubGlobal('fetch', vi.fn((input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost');
      if (url.searchParams.has('path') && failed) {
        return Promise.resolve({ ...json({ error: { message: 'patch failed' } }), ok: false, status: 500 });
      }
      return Promise.resolve(json({ head: HEAD, version: VERSION, changes: [change({ patch: url.searchParams.has('path') ? PATCH : '' })] }));
    }));
    const user = userEvent.setup();
    renderPanel();
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('patch failed'));
    expect(screen.queryByText(S.review.noPatch)).not.toBeInTheDocument();
    failed = false;
    await user.click(screen.getByRole('button', { name: S.dataState.retry }));
    expect(await screen.findByTestId('diff-viewer')).toBeInTheDocument();
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

  it('pins patch requests to the list version', async () => {
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost');
      if (url.searchParams.has('path')) {
        expect(url.searchParams.get('version')).toBe(VERSION);
      }
      return Promise.resolve(json({ head: HEAD, version: VERSION, changes: [change({ patch: PATCH })] }));
    });
    vi.stubGlobal('fetch', fetchMock);
    renderPanel();
    await screen.findByTestId('diff-viewer');
  });

  it('refreshes the list when the patch version is stale', async () => {
    let listCalls = 0;
    const fetchMock = vi.fn((input: RequestInfo | URL) => {
      const url = new URL(String(input), 'http://localhost');
      if (!url.searchParams.has('path')) {
        listCalls += 1;
        const version = listCalls > 1 ? 'v2' : 'v1';
        return Promise.resolve(json({ head: HEAD, version, changes: [change({ patch: '' })] }));
      }
      if (url.searchParams.get('version') === 'v1') {
        return Promise.resolve({
          ...json({ error: { message: 'stale diff version' } }),
          ok: false,
          status: 409,
          text: async () => JSON.stringify({ error: { message: 'stale diff version' } }),
        });
      }
      return Promise.resolve(json({ head: HEAD, version: 'v2', changes: [change({ patch: PATCH })] }));
    });
    vi.stubGlobal('fetch', fetchMock);
    renderPanel();
    expect(await screen.findByTestId('diff-viewer')).toBeInTheDocument();
    expect(listCalls).toBeGreaterThan(1);
  });
});
