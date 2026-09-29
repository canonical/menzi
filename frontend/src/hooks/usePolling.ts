import { useCallback, useEffect, useRef, useState } from 'react';

const POLL_MAX_INTERVAL_MS = 60_000;

interface PollingOptions {
  enabled?: boolean;
  intervalMs?: number;
  backoff?: boolean;
}

export function usePolling<T>(
  fn: () => Promise<T>,
  { enabled = true, intervalMs = 5000, backoff = false }: PollingOptions = {},
) {
  const [data, setData] = useState<T | undefined>();
  const [error, setError] = useState<unknown>(null);
  const [isPolling, setIsPolling] = useState(false);
  const fnRef = useRef(fn);
  const stopRef = useRef(false);

  useEffect(() => {
    fnRef.current = fn;
  }, [fn]);

  useEffect(() => {
    if (!enabled) return;

    let active = true;
    let delay = intervalMs;
    let timer: ReturnType<typeof setTimeout> | undefined;
    stopRef.current = false;

    const run = async () => {
      setIsPolling(true);
      try {
        const result = await fnRef.current();
        if (!active) return;
        setData(result);
        setError(null);
        delay = intervalMs;
      } catch (err) {
        if (!active) return;
        setError(err);
        if (backoff) delay = Math.min(delay * 2, POLL_MAX_INTERVAL_MS);
      } finally {
        if (active) setIsPolling(false);
      }
      if (active) timer = setTimeout(() => void run(), delay);
    };

    void run();

    return () => {
      active = false;
      stopRef.current = true;
      if (timer) clearTimeout(timer);
    };
  }, [enabled, intervalMs, backoff]);

  const stop = useCallback(() => {
    stopRef.current = true;
  }, []);

  return { data, error, isPolling, stop };
}