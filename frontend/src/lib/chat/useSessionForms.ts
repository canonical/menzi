import { useQuery, useQueryClient } from '@tanstack/react-query';
import { cancelForm, formsKey, getForm, listForms, replyToForm, type FormAnswer, type QuestionForm } from '../api/forms';

export function useSessionForms(sessionId: string) {
  const client = useQueryClient();
  const key = formsKey(sessionId);
  const query = useQuery({
    queryKey: key,
    queryFn: async () => {
      const pending = (await listForms(sessionId)).filter((form) => form.sessionID === sessionId);
      const previous = client.getQueryData<QuestionForm[]>(key) ?? [];
      const missing = previous.filter((form) => !pending.some((entry) => entry.id === form.id));
      const settled = await Promise.all(missing.map(async (form) => {
        if (form.state && form.state.status !== 'pending') return form;
        try {
          return await getForm(sessionId, form.id);
        } catch {
          return form;
        }
      }));
      const merged = [...settled, ...pending].filter((form) => form.sessionID === sessionId);
      const latest = client.getQueryData<QuestionForm[]>(key) ?? [];
      return merged.map((form) => {
        const existing = latest.find((entry) => entry.id === form.id);
        return existing?.state && existing.state.status !== 'pending' ? existing : form;
      });
    },
    enabled: !!sessionId,
    retry: false,
    refetchInterval: 5000,
  });
  const forms = query.data ?? [];
  const pending = forms.filter((form) => !form.state || form.state.status === 'pending');
  const submit = async (form: QuestionForm, answer: FormAnswer) => {
    await replyToForm(sessionId, form.id, answer);
    client.setQueryData<QuestionForm[]>(key, (previous) => (previous ?? []).map((entry) =>
      entry.id === form.id ? { ...entry, state: { status: 'answered', answer } } : entry));
    client.invalidateQueries({ queryKey: key });
  };
  const dismiss = async (form: QuestionForm) => {
    await cancelForm(sessionId, form.id);
    client.setQueryData<QuestionForm[]>(key, (previous) => (previous ?? []).map((entry) =>
      entry.id === form.id ? { ...entry, state: { status: 'cancelled' } } : entry));
    client.invalidateQueries({ queryKey: key });
  };
  return { forms, pending, submit, dismiss, error: query.error, refetch: query.refetch };
}
