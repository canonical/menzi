import { useState } from 'react';
import { Button } from '@canonical/react-components';
import {
  currentTheme,
  nextTheme,
  setTheme,
  themeGlyph,
  themeLabel,
  type Theme,
} from '../lib/theme';

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
        {themeGlyph(theme)}
      </span>
      <span className="p-side-navigation__label">{themeLabel(theme)}</span>
    </Button>
  );
}
