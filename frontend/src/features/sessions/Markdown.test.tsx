import { describe, expect, it } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { Markdown } from './Markdown';

describe('Markdown', () => {
  it('renders bold and italic', () => {
    const { container } = render(<Markdown text="a **bold** and an *italic* word" />);
    expect(container.querySelector('strong')?.textContent).toBe('bold');
    expect(container.querySelector('em')?.textContent).toBe('italic');
  });

  it('renders inline code', () => {
    const { container } = render(<Markdown text="run `cargo test` first" />);
    expect(container.querySelector('.app-md__code')?.textContent).toBe('cargo test');
  });

  it('renders a heading', () => {
    render(<Markdown text={'## What I found\n\nbody'} />);
    expect(screen.getByRole('heading', { level: 2, name: 'What I found' })).toBeInTheDocument();
  });

  it('renders an ordered list', () => {
    const { container } = render(<Markdown text={'1. read the file\n2. edit it'} />);
    expect(container.querySelectorAll('ol li')).toHaveLength(2);
  });

  it('renders a bullet list', () => {
    const { container } = render(<Markdown text={'- one\n- two'} />);
    expect(container.querySelectorAll('ul li')).toHaveLength(2);
  });

  it('renders a fenced code block', () => {
    const { container } = render(<Markdown text={'```rust\nfn main() {}\n```'} />);
    expect(container.querySelector('.app-md__pre')?.textContent).toContain('fn main()');
  });

  it('colours a fenced code block once the grammar loads', async () => {
    const { container } = render(<Markdown text={'```rust\nlet x = 1;\n```'} />);
    await waitFor(
      () => expect(container.querySelector('.app-md__pre .hljs-keyword')).not.toBeNull(),
      { timeout: 5_000 },
    );
  });

  it('renders a gfm table', () => {
    const { container } = render(
      <Markdown text={'| a | b |\n| - | - |\n| 1 | 2 |'} />,
    );
    expect(container.querySelectorAll('table td')).toHaveLength(2);
  });

  it('opens a link in a new tab without leaking the opener', () => {
    render(<Markdown text={'[docs](https://example.com/a)'} />);
    const link = screen.getByRole('link', { name: 'docs' });
    expect(link).toHaveAttribute('href', 'https://example.com/a');
    expect(link).toHaveAttribute('rel', expect.stringContaining('noopener'));
  });

  it('drops raw html rather than injecting it', () => {
    const { container } = render(
      <Markdown text={'before <img src=x onerror="alert(1)"> after'} />,
    );
    expect(container.querySelector('img')).toBeNull();
    expect(container.textContent).not.toContain('alert(1)');
  });

  it('renders a blockquote', () => {
    const { container } = render(<Markdown text={'> noted'} />);
    expect(container.querySelector('blockquote')?.textContent).toContain('noted');
  });
});