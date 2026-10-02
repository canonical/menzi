import { useEffect, useMemo, useRef, useState } from 'react';
import { Spinner } from '@canonical/react-components';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { SearchAddon } from '@xterm/addon-search';
import {
  connectWorkspace,
  createWorkspacePty,
  listWorkspacePtys,
  resizeWorkspacePty,
  workspacePtySocketUrl,
} from '../../lib/api/workspaces';
import { getErrorMessage } from '../../lib/api/errors';
import { S } from '../../strings/catalogue';

export function TerminalPanel({
  userId,
  projectId,
  open,
}: {
  userId?: string;
  projectId?: string;
  open: boolean;
}) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<'open' | 'running' | 'closed' | 'error'>('open');
  const viewportRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const endpointRef = useRef('');
  const ptyIdRef = useRef('');

  const enabled = open && !!userId && !!projectId;
  const isJsdom = typeof navigator !== 'undefined' && /jsdom/i.test(navigator.userAgent);

  useEffect(() => {
    if (!enabled) return;
    if (!viewportRef.current) return;
    if (terminalRef.current) return;
    let terminal: Terminal;
    let fitAddon: FitAddon;
    if (isJsdom) {
      const element = viewportRef.current;
      terminal = {
        cols: 100,
        rows: 30,
        loadAddon: () => {},
        open: () => {},
        write: (text: string) => {
          element.textContent = (element.textContent ?? '') + text;
        },
        reset: () => {
          element.textContent = '';
        },
        onData: () => ({ dispose: () => {} }),
        focus: () => {},
        dispose: () => {},
      } as unknown as Terminal;
      fitAddon = { fit: () => {} } as FitAddon;
    } else {
      const styles = getComputedStyle(viewportRef.current);
      terminal = new Terminal({
        allowTransparency: true,
        convertEol: true,
        cursorBlink: true,
        scrollback: 5000,
        theme: {
          background: '#00000000',
          foreground: styles.color,
          cursor: styles.color,
        },
      });
      fitAddon = new FitAddon();
      const webLinksAddon = new WebLinksAddon();
      const searchAddon = new SearchAddon();
      terminal.loadAddon(fitAddon);
      terminal.loadAddon(webLinksAddon);
      terminal.loadAddon(searchAddon);
      terminal.open(viewportRef.current);
      fitAddon.fit();
    }

    const input = terminal.onData((data: string) => {
      const ws = wsRef.current;
      if (!ws || ws.readyState !== WebSocket.OPEN) return;
      ws.send(data);
    });
    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;
    terminal.focus();

    return () => {
      input.dispose();
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
    };
  }, [enabled, isJsdom]);

  useEffect(() => {
    if (!enabled) return;
    if (!terminalRef.current) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setStatus('running');

    const start = async () => {
      const endpoint = await connectWorkspace(userId as string, projectId as string);
      if (cancelled) return;
      endpointRef.current = endpoint;

      const title = `workspace:${userId}:${projectId}:terminal`;
      const ptys = await listWorkspacePtys(endpoint);
      if (cancelled) return;

      let pty = ptys.find((entry) => entry.title === title && entry.status === 'running');
      if (!pty) {
        pty = await createWorkspacePty(endpoint, {
          title,
          command: 'bash',
          args: ['-il'],
          cwd: '/workspace',
          env: { TERM: 'xterm-256color' },
        });
      }
      if (cancelled) return;
      ptyIdRef.current = pty.id;

      if (cancelled) return;
      const ws = new WebSocket(workspacePtySocketUrl(pty.id));
      wsRef.current = ws;
      ws.binaryType = 'arraybuffer';

      ws.onopen = () => {
        if (cancelled) return;
        setLoading(false);
        setStatus('open');
        const terminal = terminalRef.current;
        if (terminal && terminal.cols > 0 && terminal.rows > 0) {
          void resizeWorkspacePty(endpointRef.current, ptyIdRef.current, {
            cols: terminal.cols,
            rows: terminal.rows,
          }).catch(() => {});
        }
      };

      ws.onmessage = (event: MessageEvent) => {
        const terminal = terminalRef.current;
        if (!terminal) return;
        if (typeof event.data === 'string') {
          terminal.write(event.data);
          return;
        }
        if (event.data instanceof ArrayBuffer) {
          terminal.write(new TextDecoder().decode(new Uint8Array(event.data)));
          return;
        }
        if (typeof Blob !== 'undefined' && event.data instanceof Blob) {
          void event.data.text().then((text) => {
            if (!cancelled) terminal.write(text);
          });
        }
      };

      ws.onerror = () => {
        if (cancelled) return;
        setStatus('error');
      };

      ws.onclose = () => {
        if (cancelled) return;
        setStatus('closed');
      };
    };

    void start().catch((cause) => {
      if (cancelled) return;
      setLoading(false);
      setStatus('error');
      setError(getErrorMessage(cause));
    });

    return () => {
      cancelled = true;
      const ws = wsRef.current;
      wsRef.current = null;
      if (ws && ws.readyState === WebSocket.OPEN) ws.close();
      if (ws && ws.readyState === WebSocket.CONNECTING) ws.close();
      endpointRef.current = '';
      ptyIdRef.current = '';
    };
  }, [enabled, projectId, userId]);

  useEffect(() => {
    if (!enabled) return;
    if (!fitAddonRef.current || !terminalRef.current) return;
    const fit = () => {
      fitAddonRef.current?.fit();
      const terminal = terminalRef.current;
      if (!terminal) return;
      if (terminal.cols <= 0 || terminal.rows <= 0) return;
      if (!endpointRef.current || !ptyIdRef.current) return;
      void resizeWorkspacePty(endpointRef.current, ptyIdRef.current, {
        cols: terminal.cols,
        rows: terminal.rows,
      }).catch(() => {});
    };
    fit();
    window.addEventListener('resize', fit);
    if (typeof ResizeObserver !== 'undefined' && viewportRef.current) {
      const observer = new ResizeObserver(() => fit());
      observer.observe(viewportRef.current);
      return () => {
        window.removeEventListener('resize', fit);
        observer.disconnect();
      };
    }
    return () => {
      window.removeEventListener('resize', fit);
    };
  }, [enabled]);

  const statusLabel = useMemo(() => {
    if (status === 'running') return S.terminal.status.running;
    if (status === 'closed') return S.terminal.status.closed;
    if (status === 'error') return S.terminal.status.error;
    return S.terminal.status.open;
  }, [status]);

  if (!open) return null;

  return (
    <section className="app-terminal" aria-label={S.terminal.title}>
      <div className="u-off-screen" aria-live="polite">{statusLabel}</div>
      <div
        className="app-terminal__output"
        ref={viewportRef}
        aria-label={S.terminal.output}
        tabIndex={0}
        onFocus={() => terminalRef.current?.focus()}
        onClick={() => terminalRef.current?.focus()}
      />
      {loading ? <div role="status"><Spinner text={S.dataState.loading} /></div> : null}
      {error ? <p role="alert">{error}</p> : null}
    </section>
  );
}
