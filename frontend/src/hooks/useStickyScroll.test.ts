import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { useStickyScroll } from './useStickyScroll';

/** A scroll container stand-in: jsdom does no layout, so sizes are set by hand. */
function container(overrides: Partial<Record<'scrollHeight' | 'clientHeight', number>> = {}) {
  const element = document.createElement('div');
  element.tabIndex = -1;
  Object.defineProperties(element, {
    scrollHeight: { value: overrides.scrollHeight ?? 1000, configurable: true },
    clientHeight: { value: overrides.clientHeight ?? 400, configurable: true },
  });
  element.scrollTop = 0;
  document.body.appendChild(element);
  return element;
}

function setup(threshold?: number) {
  const hook = renderHook(() => useStickyScroll<HTMLDivElement>(threshold));
  const element = container();
  act(() => {
    hook.result.current.ref.current = element;
  });
  return { ...hook, element };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('useStickyScroll', () => {
  it('starts pinned to the bottom', () => {
    const { result } = setup();
    expect(result.current.stuck).toBe(true);
  });

  it('follows new content only while the reader has not scrolled away', () => {
    const { result, element } = setup();

    act(() => result.current.follow());
    expect(element.scrollTop).toBe(1000);

    act(() => {
      element.scrollTop = 100;
      result.current.handleScroll();
    });
    expect(result.current.stuck).toBe(false);

    act(() => result.current.follow());
    expect(element.scrollTop).toBe(100);
  });

  it('treats a reader within the threshold of the bottom as still stuck', () => {
    const { result, element } = setup(24);

    act(() => {
      element.scrollTop = 1000 - 400 - 20;
      result.current.handleScroll();
    });
    expect(result.current.stuck).toBe(true);
  });

  it('jumps back to the bottom and re-arms following', () => {
    const { result, element } = setup();

    act(() => {
      element.scrollTop = 0;
      result.current.handleScroll();
    });
    expect(result.current.stuck).toBe(false);

    act(() => result.current.jumpToLatest());
    expect(result.current.stuck).toBe(true);
    expect(element.scrollTop).toBe(1000);
    expect(document.activeElement).toBe(element);
  });
});
