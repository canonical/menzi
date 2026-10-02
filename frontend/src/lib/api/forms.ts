import { http } from './client';

export interface FormOption {
  value: string;
  label: string;
  description?: string;
}

export type FormAnswer = Record<string, string | string[] | number | boolean>;

export interface QuestionField {
  key: string;
  title?: string;
  description?: string;
  type: 'string' | 'multiselect';
  options?: FormOption[];
  custom?: boolean;
  required?: boolean;
  hidden?: boolean;
  when?: { key: string; op: 'eq' | 'neq'; value: string | number | boolean | string[] }[];
  minItems?: number;
  maxItems?: number;
  minLength?: number;
  maxLength?: number;
  placeholder?: string;
}

export interface QuestionForm {
  id: string;
  sessionID: string;
  title: string;
  fields: QuestionField[];
  metadata?: Record<string, unknown>;
  state?: { status: 'pending' } | { status: 'answered'; answer: FormAnswer } | { status: 'cancelled'; message?: string };
}

export const formsKey = (sessionId: string) => ['opencode', 'forms', sessionId] as const;

export async function listForms(sessionId: string): Promise<QuestionForm[]> {
  const response = await http.get<{ data: QuestionForm[] }>(`/api/session/${encodeURIComponent(sessionId)}/form`);
  return Array.isArray(response.data) ? response.data : [];
}

export async function getForm(sessionId: string, formId: string): Promise<QuestionForm> {
  const response = await http.get<{ data: QuestionForm }>(`/api/session/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}`);
  return response.data;
}

export async function cancelForm(sessionId: string, formId: string): Promise<void> {
  await http.delete(`/api/session/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}`);
}

export async function replyToForm(sessionId: string, formId: string, answer: FormAnswer): Promise<void> {
  await http.post(`/api/session/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}/reply`, { answer });
}
