import { applyTheme, loadTheme } from '@canonical/react-components';
import { S } from '../strings/catalogue';

export type Theme = 'system' | 'light' | 'dark';

export const DARK_QUERY = '(prefers-color-scheme: dark)';

/**
 * The single control cycles through the three themes in this order, so the
 * label always describes what a click will switch to.
 */
export const THEME_CYCLE: readonly Theme[] = ['system', 'light', 'dark'];

/** The theme in effect, resolving anything unrecognised to the system theme. */
export function currentTheme(): Theme {
  const saved = loadTheme();
  return saved === 'light' || saved === 'dark' ? saved : 'system';
}

/** The theme that follows a click on the current theme. */
export function nextTheme(theme: Theme): Theme {
  const index = THEME_CYCLE.indexOf(theme);
  return THEME_CYCLE[(index + 1) % THEME_CYCLE.length];
}

/**
 * Applies the stored theme to the document. Vanilla reads the theme from the
 * wrapper classes, so both are toggled, and the stored value is the source of
 * truth for what a re-sync should restore.
 */
export function syncTheme(): Theme {
  const theme = currentTheme();
  document.body.classList.toggle('is-dark', theme === 'dark');
  document.body.classList.toggle('is-light', theme !== 'dark');
  applyTheme(theme);
  return theme;
}

/** Stores the theme and applies it. */
export function setTheme(theme: Theme): void {
  localStorage.setItem('theme', theme);
  syncTheme();
}

/** Re-applies the theme when the system theme changes, unless one is pinned. */
export function watchSystemTheme(onChange: () => void): () => void {
  if (typeof window === 'undefined' || !window.matchMedia) return () => undefined;

  const query = window.matchMedia(DARK_QUERY);
  const listener = () => {
    if (currentTheme() === 'system') onChange();
  };
  query.addEventListener('change', listener);

  return () => query.removeEventListener('change', listener);
}

/**
 * The theme states have no icon in the bundled set, so the switcher uses a
 * glyph. The two controls that render it read these rather than repeating them.
 */
const THEME_LABEL: Record<Theme, string> = {
  system: S.theme.system,
  light: S.theme.light,
  dark: S.theme.dark,
};

const THEME_GLYPH: Record<Theme, string> = {
  system: '◐',
  light: '☀',
  dark: '☾',
};

export function themeLabel(theme: Theme): string {
  return THEME_LABEL[theme];
}

export function themeGlyph(theme: Theme): string {
  return THEME_GLYPH[theme];
}
