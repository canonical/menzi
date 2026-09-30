import { useState } from 'react';
import { Button } from '@canonical/react-components';
import { currentTheme, nextTheme, setTheme, type Theme } from '../lib/theme';
import { S } from '../strings/catalogue';

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

/**
 * A single side navigation control that cycles the theme, so the switcher does
 * not have to own a row in the status bar at the bottom of every page.
 */
export function ThemeNavItem() {
  const [theme, setCurrent] = useState<Theme>(currentTheme);

  const cycle = () => {
    const next = nextTheme(theme);
    setTheme(next);
    setCurrent(next);
  };

  return (
    <Button
      appearance="link"
      className="p-side-navigation__link app-nav-button"
      onClick={cycle}
    >
      <span className="p-side-navigation__icon app-theme-nav__glyph" aria-hidden="true">
        {THEME_GLYPH[theme]}
      </span>
      <span className="p-side-navigation__label">{THEME_LABEL[theme]}</span>
    </Button>
  );
}
