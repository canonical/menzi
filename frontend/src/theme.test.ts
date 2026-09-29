import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Listener = (event: MediaQueryListEvent) => void;

class FakeMatchMedia {
  static byQuery = new Map<string, FakeMatchMedia>();
  static query = '(prefers-color-scheme: dark)';
  listeners: Listener[] = [];
  matches: boolean;

  constructor(public media: string) {
    this.matches = media === FakeMatchMedia.query;
  }

  static for(query: string) {
    const existing = FakeMatchMedia.byQuery.get(query);
    if (existing) return existing;
    const created = new FakeMatchMedia(query);
    FakeMatchMedia.byQuery.set(query, created);
    return created;
  }

  addEventListener(_type: string, listener: Listener) {
    this.listeners.push(listener);
  }

  removeEventListener(_type: string, listener: Listener) {
    this.listeners = this.listeners.filter((item) => item !== listener);
  }

  emit(matches: boolean) {
    this.matches = matches;
    for (const listener of [...this.listeners]) {
      listener({ matches } as MediaQueryListEvent);
    }
  }
}

function loadMain() {
  FakeMatchMedia.byQuery.clear();
  document.body.className = '';
  document.documentElement.style.colorScheme = '';
  vi.resetModules();
  return import('./main');
}

beforeEach(() => {
  FakeMatchMedia.byQuery.clear();
  const factory = (query: string) => FakeMatchMedia.for(query);
  vi.stubGlobal('matchMedia', factory);
  Object.defineProperty(window, 'matchMedia', { writable: true, value: factory });
  document.body.innerHTML = '<div id="root"></div>';
});

afterEach(() => {
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe('theme bootstrap', () => {
  it('applies the light class when light is stored', async () => {
    localStorage.setItem('theme', 'light');
    await loadMain();
    expect(document.body.classList.contains('is-light')).toBe(true);
    expect(document.body.classList.contains('is-dark')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('light');
  });

  it('applies the dark class when dark is stored', async () => {
    localStorage.setItem('theme', 'dark');
    await loadMain();
    expect(document.body.classList.contains('is-dark')).toBe(true);
    expect(document.body.classList.contains('is-light')).toBe(false);
    expect(document.documentElement.style.colorScheme).toBe('dark');
  });

  it('follows the operating system when the theme is system', async () => {
    localStorage.setItem('theme', 'system');
    await loadMain();
    expect(document.body.classList.contains('is-dark')).toBe(true);
  });

  it('reacts to an operating system change only in system mode', async () => {
    localStorage.setItem('theme', 'system');
    await loadMain();
    const media = FakeMatchMedia.byQuery.get(FakeMatchMedia.query);
    expect(media).toBeDefined();

    media!.emit(false);
    expect(document.body.classList.contains('is-dark')).toBe(false);
    expect(document.body.classList.contains('is-light')).toBe(true);

    media!.emit(true);
    expect(document.body.classList.contains('is-dark')).toBe(true);
  });

  it('ignores operating system changes when a theme is pinned', async () => {
    localStorage.setItem('theme', 'light');
    await loadMain();
    const media = FakeMatchMedia.byQuery.get(FakeMatchMedia.query);
    media!.emit(true);
    expect(document.body.classList.contains('is-light')).toBe(true);
    expect(document.body.classList.contains('is-dark')).toBe(false);
  });
});
