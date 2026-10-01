import { describe, expect, it } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
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

  it('says so when there is no patch', () => {
    render(<DiffViewer file="src/main.rs" patch="" />);
    expect(screen.getByText(/no textual diff/i)).toBeInTheDocument();
  });
});