import { describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import { S } from '../../strings/catalogue';
import { DiffViewer } from './DiffViewer';

const PATCH = [
  'Index: /workspace/src/main.rs',
  '===...===',
  '@@ -1,3 +1,3 @@',
  ' fn main() {',
  '-    println!("old");',
  '+    println!("new");',
  ' }',
].join('\n');

describe('DiffViewer', () => {
  it('puts the removed line on the left and the added one on the right', async () => {
    render(<DiffViewer file="src/main.rs" patch={PATCH} />);

    const removed = document.querySelector('[data-side="left"][data-kind="remove"]');
    const added = document.querySelector('[data-side="right"][data-kind="add"]');

    expect(removed?.textContent).toContain('old');
    expect(added?.textContent).toContain('new');
    await waitFor(() => expect(removed).not.toBeNull());
  });

  it('marks the left column negative and the right positive', () => {
    render(<DiffViewer file="src/main.rs" patch={PATCH} />);

    const removed = document.querySelector('[data-side="left"][data-kind="remove"]');
    const added = document.querySelector('[data-side="right"][data-kind="add"]');

    expect(removed?.className).toContain('app-diff__cell--left');
    expect(added?.className).toContain('app-diff__cell--right');
    expect(removed?.textContent).toContain('-');
    expect(added?.textContent).toContain('+');
  });

  it('shows the old line number on the left and the new on the right', () => {
    render(<DiffViewer file="src/main.rs" patch={PATCH} />);

    const rows = document.querySelectorAll('.app-diff__row');
    const changed = rows[1];
    const gutters = changed.querySelectorAll('.app-diff__gutter');

    expect(gutters[0].textContent).toBe('2');
    expect(gutters[1].textContent).toBe('2');
  });

  it('colours the code once the grammar has loaded', async () => {
    render(<DiffViewer file="src/main.rs" patch={PATCH} />);

    await waitFor(
      () => {
        expect(document.querySelector('.app-diff__content .hljs-keyword')).not.toBeNull();
      },
      { timeout: 5_000 },
    );
  });

  it('defaults to split mode with column headings and one shared scroll body', () => {
    render(<DiffViewer file="src/main.rs" patch={PATCH} />);

    const viewer = screen.getByTestId('diff-viewer');
    expect(viewer).toHaveClass('app-diff--split');
    expect(viewer).toHaveClass('app-diff--wrap');
    expect(viewer.querySelector(':scope > .app-diff__body')).not.toBeNull();
    expect(screen.getByText(S.review.before)).toBeInTheDocument();
    expect(screen.getByText(S.review.after)).toBeInTheDocument();
  });

  it('preserves unified ordering and shows context only once', () => {
    const patch = [
      '@@ -1,4 +1,4 @@',
      ' start',
      '-old first',
      '-old second',
      '+new first',
      '+new second',
      ' end',
    ].join('\n');
    render(<DiffViewer file="file.txt" patch={patch} mode="unified" />);

    const viewer = screen.getByTestId('diff-viewer');
    expect(viewer).toHaveClass('app-diff--unified');
    expect(Array.from(viewer.querySelectorAll('.app-diff__content'), (line) => line.textContent))
      .toEqual(['start', 'old first', 'old second', 'new first', 'new second', 'end']);
    expect(viewer.querySelectorAll('[data-kind="context"]')).toHaveLength(2);
    const rows = viewer.querySelectorAll('.app-diff__row');
    expect(Array.from(rows[0].querySelectorAll('.app-diff__gutter'), (gutter) => gutter.textContent))
      .toEqual(['1', '1']);
    expect(Array.from(rows[1].querySelectorAll('.app-diff__gutter'), (gutter) => gutter.textContent))
      .toEqual(['2', '']);
    expect(Array.from(rows[3].querySelectorAll('.app-diff__gutter'), (gutter) => gutter.textContent))
      .toEqual(['', '2']);
    expect(rows[1].querySelector('.app-diff__mark')).toHaveTextContent('-');
    expect(rows[3].querySelector('.app-diff__mark')).toHaveTextContent('+');
  });

  it('highlights unified removals and additions using their own source sides', async () => {
    const patch = [
      '@@ -1,3 +1,3 @@',
      '-/* old comment',
      '-still a comment',
      '-*/',
      '+const first = 1;',
      '+const second = 2;',
      '+const third = 3;',
    ].join('\n');
    render(<DiffViewer file="file.js" patch={patch} mode="unified" />);

    await waitFor(() => {
      const removed = document.querySelectorAll('[data-kind="remove"]');
      const added = document.querySelectorAll('[data-kind="add"]');
      expect(removed[1].querySelector('.hljs-comment')).not.toBeNull();
      expect(added[0].querySelector('.hljs-keyword')).not.toBeNull();
      expect(added[0].querySelector('.hljs-comment')).toBeNull();
    });
  });

  it('updates explicit mode and wrapping without changing the patch', () => {
    const { rerender } = render(<DiffViewer file="file.txt" patch={PATCH} mode="unified" wrapLines />);
    expect(screen.getByTestId('diff-viewer')).toHaveClass('app-diff--unified', 'app-diff--wrap');

    rerender(<DiffViewer file="file.txt" patch={PATCH} mode="split" wrapLines={false} />);
    expect(screen.getByTestId('diff-viewer')).toHaveClass('app-diff--split');
    expect(screen.getByTestId('diff-viewer')).not.toHaveClass('app-diff--wrap', 'app-diff--unified');
  });

  it('chooses automatic mode from the viewer width and disconnects the observer', () => {
    let callback: ResizeObserverCallback = () => {};
    const observe = vi.fn();
    const disconnect = vi.fn();
    vi.stubGlobal('ResizeObserver', class {
      constructor(value: ResizeObserverCallback) { callback = value; }
      observe = observe;
      disconnect = disconnect;
    });
    try {
      const { unmount } = render(<DiffViewer file="file.txt" patch={PATCH} mode="auto" />);
      const viewer = screen.getByTestId('diff-viewer');
      expect(viewer).toHaveClass('app-diff--unified');
      expect(observe).toHaveBeenCalledWith(viewer);
      const resize = (width: number) => act(() => callback([
        {
          target: viewer,
          contentRect: new DOMRect(0, 0, width, 0),
          borderBoxSize: [],
          contentBoxSize: [],
          devicePixelContentBoxSize: [],
        },
      ], {} as ResizeObserver));
      resize(850);
      expect(viewer).toHaveClass('app-diff--split');
      resize(849);
      expect(viewer).toHaveClass('app-diff--unified');
      unmount();
      expect(disconnect).toHaveBeenCalledOnce();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('says so when there is no patch', () => {
    render(<DiffViewer file="src/main.rs" patch="" />);
    expect(screen.getByText(/no textual diff/i)).toBeInTheDocument();
  });
});