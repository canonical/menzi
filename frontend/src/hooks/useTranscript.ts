import { useCallback, useEffect, useRef, useState } from 'react';
import { listTranscript, transcriptStreamUrl } from '../lib/api/transcript';
import { getErrorMessage } from '../lib/api/errors';
import type { TunnelMessage } from '../lib/types';

export type TranscriptStatus = 'idle' | 'loading' | 'live' | 'reconnecting' | 'error';

const RECONNECT_DELAY_MS = 1000;

interface UseTranscriptOptions {
  enabled: boolean;
}

interface UseTranscriptResult {
  events: TunnelMessage[];
  status: TranscriptStatus;
  error: string | null;
  loaded: boolean;
  lastSequence: number;
  reconnect: () => void;
}

function isTunnelMessage(value: unknown): value is TunnelMessage {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<TunnelMessage>;
  return (
    typeof candidate.sequence === 'number' &&
    typeof candidate.session_id === 'string' &&
    typeof candidate.message_type === 'string'
  );
}

export function useTranscript(
  sessionId: string,
  { enabled }: UseTranscriptOptions,
): UseTranscriptResult {
  const [events, setEvents] = useState<TunnelMessage[]>([]);
  const [status, setStatus] = useState<TranscriptStatus>('idle');
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const lastSequence = useRef(-1);
  const source = useRef<EventSource | null>(null);
  const timer = useRef<number | null>(null);

  const reconnect = useCallback(() => {
    setAttempt((value) => value + 1);
  }, []);

  useEffect(() => {
    if (!enabled) {
      setStatus('idle');
      return;
    }

    let cancelled = false;
    setStatus('loading');
    setLoaded(false);

    const absorb = (incoming: TunnelMessage[]) => {
      const fresh = incoming.filter(
        (message) => message.sequence > lastSequence.current,
      );
      if (fresh.length === 0) return;
      lastSequence.current = fresh[fresh.length - 1].sequence;
      setEvents((current) => [...current, ...fresh]);
    };

    const open = () => {
      if (cancelled) return;
      const url = transcriptStreamUrl(sessionId, lastSequence.current);
      const stream = new EventSource(url);
      source.current = stream;

      stream.onopen = () => {
        if (cancelled) return;
        setStatus('live');
        setError(null);
      };

      stream.onmessage = (event) => {
        try {
          const parsed: unknown = JSON.parse(event.data);
          if (isTunnelMessage(parsed)) absorb([parsed]);
        } catch {
          setError('The session stream sent an event that could not be read.');
        }
      };

      stream.onerror = () => {
        if (cancelled) return;
        stream.close();
        source.current = null;
        setStatus('reconnecting');
        timer.current = window.setTimeout(open, RECONNECT_DELAY_MS);
      };
    };

    const load = async () => {
      try {
        const backlog = await listTranscript(sessionId);
        if (cancelled) return;
        absorb(backlog);
        setLoaded(true);
        open();
      } catch (caught) {
        if (cancelled) return;
        setLoaded(true);
        setStatus('error');
        setError(getErrorMessage(caught));
      }
    };

    void load();

    return () => {
      cancelled = true;
      if (timer.current !== null) {
        window.clearTimeout(timer.current);
        timer.current = null;
      }
      source.current?.close();
      source.current = null;
    };
  }, [sessionId, enabled, attempt]);

  return { events, status, error, loaded, lastSequence: lastSequence.current, reconnect };
}
