import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ReviewFileTree } from './ReviewFileTree';
import { S } from '../../strings/catalogue';

const FILES = ['README.md', 'src/z.ts', 'src/lib/api.json', 'src/a.tsx', 'logo.png', 'unknown.xyz'];

function setup(selected = 'src/a.tsx') {
  const onSelect = vi.fn();
  const rendered = render(<ReviewFileTree files={FILES} selected={selected} onSelect={onSelect} />);
  return { ...rendered, onSelect };
}

describe('ReviewFileTree', () => {
  it('groups files into sorted directories and compact rows', () => {
    setup();
    const tree = screen.getByRole('tree', { name: S.review.filesLabel });
    expect(within(tree).getAllByRole('treeitem').map((item) => item.getAttribute('aria-label'))).toEqual([
      'src', 'src/lib', 'src/lib/api.json', 'src/a.tsx', 'src/z.ts', 'logo.png', 'README.md', 'unknown.xyz',
    ]);
    expect(screen.getByRole('treeitem', { name: 'src/a.tsx' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('treeitem', { name: 'src/lib/api.json' })).toHaveAttribute('aria-level', '3');
  });

  it('collapses nested directories independently and retains selection', async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getByRole('treeitem', { name: 'src/lib' }));
    expect(screen.queryByRole('treeitem', { name: 'src/lib/api.json' })).not.toBeInTheDocument();
    expect(screen.getByRole('treeitem', { name: 'src/lib' })).toHaveAttribute('aria-expanded', 'false');
    await user.click(screen.getByRole('treeitem', { name: 'src' }));
    expect(screen.queryByRole('treeitem', { name: 'src/a.tsx' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('treeitem', { name: 'src' }));
    expect(screen.getByRole('treeitem', { name: 'src/a.tsx' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByRole('treeitem', { name: 'src/lib/api.json' })).not.toBeInTheDocument();
  });

  it('selects files without selecting directories', async () => {
    const user = userEvent.setup();
    const { onSelect } = setup();
    await user.click(screen.getByRole('treeitem', { name: 'src/lib' }));
    expect(onSelect).not.toHaveBeenCalled();
    await user.click(screen.getByRole('treeitem', { name: 'README.md' }));
    expect(onSelect).toHaveBeenCalledWith('README.md');
  });

  it('reveals an externally selected file in a collapsed directory', async () => {
    const user = userEvent.setup();
    const { rerender, onSelect } = setup();
    await user.click(screen.getByRole('treeitem', { name: 'src' }));
    rerender(<ReviewFileTree files={FILES} selected="src/lib/api.json" onSelect={onSelect} />);
    expect(screen.getByRole('treeitem', { name: 'src/lib/api.json' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('treeitem', { name: 'src' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('uses extension icons with a fallback for unknown files', () => {
    setup();
    expect(screen.getByRole('treeitem', { name: 'src/a.tsx' }).querySelector('svg')).toHaveAttribute('data-extension', 'tsx');
    expect(screen.getByRole('treeitem', { name: 'src/lib/api.json' }).querySelector('svg')).toHaveClass('app-review-tree__icon--data');
    expect(screen.getByRole('treeitem', { name: 'logo.png' }).querySelector('svg')).toHaveClass('app-review-tree__icon--image');
    expect(screen.getByRole('treeitem', { name: 'unknown.xyz' }).querySelector('svg')).toHaveClass('app-review-tree__icon--file');
  });

  it('supports arrow navigation, expand/collapse, home and end', async () => {
    const user = userEvent.setup();
    setup();
    screen.getByRole('treeitem', { name: 'src' }).focus();
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('treeitem', { name: 'src/lib' })).toHaveFocus();
    await user.keyboard('{ArrowLeft}');
    expect(screen.getByRole('treeitem', { name: 'src/lib' })).toHaveAttribute('aria-expanded', 'false');
    await user.keyboard('{ArrowLeft}');
    expect(screen.getByRole('treeitem', { name: 'src' })).toHaveFocus();
    await user.keyboard('{ArrowDown}');
    expect(screen.getByRole('treeitem', { name: 'src/lib' })).toHaveFocus();
    await user.keyboard('{End}');
    expect(screen.getByRole('treeitem', { name: 'unknown.xyz' })).toHaveFocus();
    await user.keyboard('{Home}');
    expect(screen.getByRole('treeitem', { name: 'src' })).toHaveFocus();
  });
});
