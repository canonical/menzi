import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useSessionForms } from './useSessionForms';
import { formsKey, type QuestionForm } from '../api/forms';
import type { ReactNode } from 'react';

const form: QuestionForm = { id: 'frm_1', sessionID: 'ses_1', title: 'Database', fields: [{ key: 'db', type: 'string' }] };
afterEach(() => vi.unstubAllGlobals());

function setup() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return { client, ...renderHook(() => useSessionForms('ses_1'), { wrapper }) };
}

describe('useSessionForms', () => {
  it('filters other sessions and retains answered forms after submission', async () => {
    let answered = false;
    vi.stubGlobal('fetch', vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
      if (init?.method === 'POST') { answered = true; return Promise.resolve(new Response(null, { status: 204 })); }
      return Promise.resolve(new Response(JSON.stringify({ data: answered ? [] : [form, { ...form, id: 'frm_other', sessionID: 'ses_other' }] })));
    }));
    const { result, client } = setup();
    await waitFor(() => expect(result.current.pending).toHaveLength(1));
    await result.current.submit(form, { db: 'pg' });
    await waitFor(() => expect(result.current.pending).toHaveLength(0));
    expect(client.getQueryData<QuestionForm[]>(formsKey('ses_1'))?.[0].state).toEqual({ status: 'answered', answer: { db: 'pg' } });
    await result.current.refetch();
    expect(result.current.forms).toHaveLength(1);
  });

  it('reconciles an answer submitted from another client', async () => {
    let settled = false;
    vi.stubGlobal('fetch', vi.fn((input: RequestInfo | URL) => {
      const detail = String(input).endsWith('/frm_1');
      return Promise.resolve(new Response(JSON.stringify({ data: detail ? { ...form, state: { status: 'answered', answer: { db: 'sqlite' } } } : settled ? [] : [form] })));
    }));
    const { result } = setup();
    await waitFor(() => expect(result.current.pending).toHaveLength(1));
    settled = true;
    await result.current.refetch();
    await waitFor(() => expect(result.current.pending).toHaveLength(0));
    expect(result.current.forms[0].state?.status).toBe('answered');
  });

  it('keeps forms with camel case session id alias', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({
      data: [
        { id: 'frm_1', sessionId: 'ses_1', title: 'Database', fields: [{ key: 'db', type: 'string' }] },
        { id: 'frm_2', sessionId: 'ses_other', title: 'Other', fields: [{ key: 'db', type: 'string' }] },
      ],
    }))));
    const { result } = setup();
    await waitFor(() => expect(result.current.forms).toHaveLength(1));
    expect(result.current.forms[0].id).toBe('frm_1');
  });
});
