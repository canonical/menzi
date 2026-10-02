import { afterEach, describe, expect, it, vi } from 'vitest';
import { getForm, listForms, replyToForm } from './forms';

afterEach(() => vi.unstubAllGlobals());

describe('forms API', () => {
  it('lists session-scoped forms using the V2 envelope', async () => {
    const form = { id: 'frm_1', sessionID: 'ses_1', title: 'Database', fields: [] };
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ data: [form] })));
    vi.stubGlobal('fetch', fetchMock);
    expect(await listForms('ses_1')).toEqual([form]);
    expect(fetchMock.mock.calls[0][0]).toBe('/api/session/ses_1/form');
    expect(fetchMock.mock.calls[0][1].headers.get('Authorization')).toMatch(/^Bearer /);
  });

  it('reads settled forms', async () => {
    const form = { id: 'frm_1', state: { status: 'answered', answer: { db: 'pg' } } };
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ data: form }))));
    expect(await getForm('ses_1', 'frm_1')).toEqual(form);
  });

  it('replies with keyed answers and handles no content', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal('fetch', fetchMock);
    await replyToForm('ses_1', 'frm_1', { db: 'pg', auth: ['email', 'github'] });
    expect(fetchMock.mock.calls[0][0]).toBe('/api/session/ses_1/form/frm_1/reply');
    expect(JSON.parse(fetchMock.mock.calls[0][1].body)).toEqual({ answer: { db: 'pg', auth: ['email', 'github'] } });
  });
});
