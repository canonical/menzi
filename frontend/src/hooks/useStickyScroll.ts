import { RefObject, useCallback, useRef, useState } from 'react';

export interface StickyScroll<T extends HTMLElement> {
  /** Attach to the scrollable element. */
  ref: RefObject<T | null>;
  /** True while the element is scrolled to the bottom. */
  stuck: boolean;
  /** Wire to the element's `onScroll`. */
  handleScroll: () => void;
  /** Re-pin to the bottom, but only while the user has not scrolled away. */
  follow: () => void;
  /** Scroll to the bottom and stop following, used by the "Jump to latest" chip. */
  jumpToLatest: () => void;
}

const DEFAULT_THRESHOLD = 24;

/**
 * Keeps a scroll container pinned to the bottom while new content streams in,
 * and lets the reader scroll away without being dragged back down.
 *
 * `follow` is a no-op once the reader has scrolled up, so call it whenever the
 * content changes. `jumpToLatest` re-arms the behaviour and moves focus to the
 * container so keyboard users are not dropped at the top of the page when the
 * chip disappears.
 */
export function useStickyScroll<T extends HTMLElement>(
  threshold = DEFAULT_THRESHOLD,
): StickyScroll<T> {
  const ref = useRef<T | null>(null);
  const [stuck, setStuck] = useState(true);
  // Mirrors `stuck` so the scroll callbacks stay stable across renders.
  const stuckRef = useRef(true);

  const isAtBottom = useCallback(() => {
    const element = ref.current;
    if (!element) return true;
    return element.scrollHeight - element.scrollTop - element.clientHeight <= threshold;
  }, [threshold]);

  const handleScroll = useCallback(() => {
    const next = isAtBottom();
    if (next === stuckRef.current) return;
    stuckRef.current = next;
    setStuck(next);
  }, [isAtBottom]);

  const follow = useCallback(() => {
    const element = ref.current;
    if (!element || !stuckRef.current) return;
    element.scrollTop = element.scrollHeight;
  }, []);

  const jumpToLatest = useCallback(() => {
    const element = ref.current;
    stuckRef.current = true;
    setStuck(true);
    if (!element) return;
    element.scrollTop = element.scrollHeight;
    element.focus({ preventScroll: true });
  }, []);

  return { ref, stuck, handleScroll, follow, jumpToLatest };
}
