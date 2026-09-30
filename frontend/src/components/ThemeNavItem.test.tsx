import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ThemeNavItem } from './ThemeNavItem';
import { S } from '../strings/catalogue';

beforeEach(() => {
  document.body.className = '';
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockReturnValue({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('ThemeNavItem', () => {
  it('shows the stored theme', () => {
    localStorage.setItem('theme', 'dark');
    render(<ThemeNavItem />);
    expect(screen.getByRole('button', { name: S.theme.dark })).toBeInTheDocument();
  });

  it('cycles auto, light and dark, applying each one', async () => {
    const user = userEvent.setup();
    render(<ThemeNavItem />);

    await user.click(screen.getByRole('button', { name: S.theme.system }));
    expect(localStorage.getItem('theme')).toBe('light');
    expect(document.body.classList.contains('is-light')).toBe(true);
    expect(document.body.classList.contains('is-dark')).toBe(false);

    await user.click(screen.getByRole('button', { name: S.theme.light }));
    expect(localStorage.getItem('theme')).toBe('dark');
    expect(document.body.classList.contains('is-dark')).toBe(true);

    await user.click(screen.getByRole('button', { name: S.theme.dark }));
    expect(localStorage.getItem('theme')).toBe('system');
  });
});
