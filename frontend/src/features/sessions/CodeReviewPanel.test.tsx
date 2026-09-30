import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CodeReviewPanel } from './CodeReviewPanel';
import { S } from '../../strings/catalogue';

const SESSION = 'ses_abc123';

const MAIN_PATCH = `diff --git a/src/main.rs b/src/main.rs
index 1234567..89abcde 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
-    old();
+    setup();
+    run();
 }
`;

const NEW_PATCH = `diff --git a/src/new.rs b/src/new.rs
new file mode 100644
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1,2 @@
+fn helper() {
+}
`;

/** A user turn that reports the files it changed. */
function turn(
  id: string,
  diffs: Record<string, unknown>[],
): { info: Record<string, unknown>; parts: unknown[] } {
  return { info: { id, sessionID: SESSION, role: 'user', summary: { diffs } }, parts: [] };
}

/** The first file carries its patch, the second only its counts. */
const MESSAGES = [
  turn('msg_1', [
    { file: 'src/main.rs', additions: 2, deletions: 1, status: 'modified', patch: MAIN_PATCH },
    { file: 'src/new.rs', additions: 2, deletions: 0, status: 'added' },
  ]),
];

const PATCHES: Record<string, string> = { 'src/main.rs': MAIN_PATCH, 'src/new.rs': NEW_PATCH };

function jsonResponse(status: number, body: unknown) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'OK',
    text: async () => JSON.stringify(body),
    json: async () => body,
  };
}

/** The diff route only answers for a message, and only with a real patch. */
function mockApi(messages: unknown[], patches: Record<string, string> = PATCHES) {
  vi.stubGlobal(
    'fetch',
    vi.fn((input: RequestInfo | URL) => {
      const url = String(input);
      if (url.includes('/message')) return Promise.resolve(jsonResponse(200, messages));
      if (url.includes('/diff')) {
        const messageId = new URL(url, 'http://x').searchParams.get('messageID');
        const body = messageId
          ? Object.entries(patches).map(([file, patch]) => ({
              file,
              patch,
              additions: 2,
              deletions: 1,
            }))
          : [];
        return Promise.resolve(jsonResponse(200, body));
      }
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
  it('lists the files the session changed, with status and counts', async () => {
    mockApi(MESSAGES);
    renderPanel();
    const list = within(await screen.findByRole('list', { name: S.review.filesLabel }));
    const row = within(list.getByRole('button', { name: 'src/main.rs' }));

    expect(list.getByRole('button', { name: 'src/new.rs' })).toBeInTheDocument();
    expect(row.getByText('modified')).toBeInTheDocument();
    expect(row.getByText('+2')).toBeInTheDocument();
    expect(row.getByText('-1')).toBeInTheDocument();
  });

  it('renders the patch a message already carried', async () => {
    mockApi(MESSAGES);
    renderPanel();
    const viewer = await screen.findByTestId('diff-viewer');

    expect(within(viewer).getByText('fn main() {')).toBeInTheDocument();
    expect(viewer.textContent).toContain('+    run();');
    expect(viewer.textContent).toContain('-    old();');
  });

  it('loads the patch for a file whose summary only had counts', async () => {
    mockApi(MESSAGES);
    const user = userEvent.setup();
    renderPanel();
    const list = within(await screen.findByRole('list', { name: S.review.filesLabel }));

    await user.click(list.getByRole('button', { name: 'src/new.rs' }));

    expect(await screen.findByText('fn helper() {')).toBeInTheDocument();
    expect(screen.queryByText(S.review.noPatch)).not.toBeInTheDocument();
  });

  it('asks the diff route for the patch of the message that changed the file', async () => {
    const urls: string[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn((input: RequestInfo | URL) => {
        const url = String(input);
        urls.push(url);
        if (url.includes('/message')) return Promise.resolve(jsonResponse(200, MESSAGES));
        if (url.includes('/diff')) {
          return Promise.resolve(
            jsonResponse(200, [
              { file: 'src/main.rs', patch: MAIN_PATCH, additions: 2, deletions: 1 },
              { file: 'src/new.rs', patch: NEW_PATCH, additions: 2, deletions: 0 },
            ]),
          );
        }
        return Promise.resolve(jsonResponse(404, { error: 'no route' }));
      }),
    );
    const user = userEvent.setup();
    renderPanel();
    const list = within(await screen.findByRole('list', { name: S.review.filesLabel }));

    // main.rs already has a patch, so nothing is fetched until new.rs is picked.
    expect(urls.filter((url) => url.includes('/diff'))).toHaveLength(0);

    await user.click(list.getByRole('button', { name: 'src/new.rs' }));
    await screen.findByText('fn helper() {');

    const diffCalls = urls.filter((url) => url.includes('/diff'));
    expect(diffCalls).toHaveLength(1);
    expect(diffCalls[0]).toContain('messageID=msg_1');
  });

  it('switches the viewer when another file is picked', async () => {
    mockApi([
      turn('msg_1', [
        { file: 'src/main.rs', additions: 2, deletions: 1, patch: MAIN_PATCH },
        { file: 'src/new.rs', additions: 2, deletions: 0 },
      ]),
      turn('msg_2', [{ file: 'src/other.rs', additions: 1, deletions: 0 }]),
    ]);
    const user = userEvent.setup();
    renderPanel();
    const list = within(await screen.findByRole('list', { name: S.review.filesLabel }));

    await user.click(list.getByRole('button', { name: 'src/main.rs' }));

    expect(screen.getByText('fn main() {')).toBeInTheDocument();
  });

  it('says so when a file has no patch to show', async () => {
    // No summary carried a patch and the diff route has nothing either.
    mockApi(
      [turn('msg_1', [{ file: 'src/main.rs', additions: 2, deletions: 1, status: 'modified' }])],
      {},
    );
    renderPanel();
    const list = within(await screen.findByRole('list', { name: S.review.filesLabel }));

    expect(list.getByRole('button', { name: 'src/main.rs' })).toBeInTheDocument();
    expect(await screen.findByText(S.review.noPatch)).toBeInTheDocument();
  });

  it('shows an empty state when the session changed no files', async () => {
    mockApi([turn('msg_1', [])]);
    renderPanel();
    expect(await screen.findByText(S.review.emptyTitle)).toBeInTheDocument();
  });

  it('surfaces a failure reading the conversation', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(() => Promise.resolve(jsonResponse(500, { error: 'diff unavailable' }))),
    );
    renderPanel();
    expect(await screen.findByText('diff unavailable')).toBeInTheDocument();
  });
});
