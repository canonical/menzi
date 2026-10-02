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
const FORM_BASES = ['/session', '/api/session'] as const;

function asRecord(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function firstString(source: Record<string, unknown>, keys: string[]): string {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'string' && value.trim().length > 0) return value.trim();
  }
  return '';
}

function unwrapData(value: unknown): unknown {
  const record = asRecord(value);
  return Object.prototype.hasOwnProperty.call(record, 'data') ? record.data : value;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.filter((entry): entry is string => typeof entry === 'string');
}

function asOptions(value: unknown): FormOption[] {
  if (!Array.isArray(value)) return [];
  return value
    .map((entry) => {
      const record = asRecord(entry);
      const optionValue = firstString(record, ['value']);
      const label = firstString(record, ['label']);
      if (!optionValue || !label) return null;
      const description = firstString(record, ['description']);
      return description ? { value: optionValue, label, description } : { value: optionValue, label };
    })
    .filter((entry): entry is FormOption => entry !== null);
}

function asWhen(value: unknown): QuestionField['when'] {
  if (!Array.isArray(value)) return undefined;
  const entries = value
    .map((entry) => {
      const record = asRecord(entry);
      const key = firstString(record, ['key']);
      const op = record.op === 'neq' ? 'neq' : record.op === 'eq' ? 'eq' : null;
      const candidate = record.value;
      const validValue =
        typeof candidate === 'string'
        || typeof candidate === 'number'
        || typeof candidate === 'boolean'
        || Array.isArray(candidate);
      if (!key || !op || !validValue) return null;
      return { key, op, value: candidate as string | number | boolean | string[] };
    })
    .filter((entry): entry is NonNullable<QuestionField['when']>[number] => entry !== null);
  return entries.length > 0 ? entries : undefined;
}

function asField(value: unknown): QuestionField | null {
  const record = asRecord(value);
  const key = firstString(record, ['key']);
  if (!key) return null;
  const rawType = firstString(record, ['type']);
  const type: QuestionField['type'] = rawType === 'multiselect' ? 'multiselect' : 'string';
  const title = firstString(record, ['title']);
  const description = firstString(record, ['description']);
  const placeholder = firstString(record, ['placeholder']);
  const options = asOptions(record.options);
  const when = asWhen(record.when);
  const field: QuestionField = {
    key,
    type,
    ...(title ? { title } : {}),
    ...(description ? { description } : {}),
    ...(options.length > 0 ? { options } : {}),
    ...(typeof record.custom === 'boolean' ? { custom: record.custom } : {}),
    ...(typeof record.required === 'boolean' ? { required: record.required } : {}),
    ...(typeof record.hidden === 'boolean' ? { hidden: record.hidden } : {}),
    ...(when ? { when } : {}),
    ...(typeof record.minItems === 'number' ? { minItems: record.minItems } : {}),
    ...(typeof record.maxItems === 'number' ? { maxItems: record.maxItems } : {}),
    ...(typeof record.minLength === 'number' ? { minLength: record.minLength } : {}),
    ...(typeof record.maxLength === 'number' ? { maxLength: record.maxLength } : {}),
    ...(placeholder ? { placeholder } : {}),
  };
  return field;
}

function asState(value: unknown): QuestionForm['state'] {
  const record = asRecord(value);
  const status = firstString(record, ['status']);
  if (status === 'pending') return { status: 'pending' };
  if (status === 'cancelled') {
    const message = firstString(record, ['message']);
    return message ? { status: 'cancelled', message } : { status: 'cancelled' };
  }
  if (status === 'answered') {
    const answerSource = asRecord(record.answer);
    const answer: FormAnswer = {};
    for (const [key, raw] of Object.entries(answerSource)) {
      if (typeof raw === 'string' || typeof raw === 'number' || typeof raw === 'boolean') {
        answer[key] = raw;
      } else {
        const values = asStringArray(raw);
        if (values.length > 0) answer[key] = values;
      }
    }
    return { status: 'answered', answer };
  }
  return undefined;
}

function normaliseForm(value: unknown): QuestionForm | null {
  const record = asRecord(value);
  const id = firstString(record, ['id']);
  const sessionID = firstString(record, ['sessionID', 'sessionId', 'session_id']);
  const title = firstString(record, ['title', 'question', 'header', 'text']);
  if (!id || !sessionID || !title) return null;
  const fields = Array.isArray(record.fields)
    ? record.fields.map(asField).filter((entry): entry is QuestionField => entry !== null)
    : [];
  const metadata = asRecord(record.metadata);
  const state = asState(record.state);
  return {
    id,
    sessionID,
    title,
    fields,
    ...(Object.keys(metadata).length > 0 ? { metadata } : {}),
    ...(state ? { state } : {}),
  };
}

export async function listForms(sessionId: string): Promise<QuestionForm[]> {
  let failure: unknown = null;
  for (const base of FORM_BASES) {
    try {
      const response = await http.get<unknown>(`${base}/${encodeURIComponent(sessionId)}/form`);
      const forms = unwrapData(response);
      if (!Array.isArray(forms)) return [];
      return forms
        .map(normaliseForm)
        .filter((entry): entry is QuestionForm => entry !== null);
    } catch (error) {
      failure = error;
    }
  }
  throw failure;
}

export async function getForm(sessionId: string, formId: string): Promise<QuestionForm> {
  let failure: unknown = null;
  for (const base of FORM_BASES) {
    try {
      const response = await http.get<unknown>(`${base}/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}`);
      const form = normaliseForm(unwrapData(response));
      if (!form) throw new Error('Invalid form response');
      return form;
    } catch (error) {
      failure = error;
    }
  }
  throw failure;
}

export async function cancelForm(sessionId: string, formId: string): Promise<void> {
  let failure: unknown = null;
  for (const base of FORM_BASES) {
    try {
      await http.delete(`${base}/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}`);
      return;
    } catch (error) {
      failure = error;
    }
  }
  throw failure;
}

export async function replyToForm(sessionId: string, formId: string, answer: FormAnswer): Promise<void> {
  let failure: unknown = null;
  for (const base of FORM_BASES) {
    try {
      await http.post(`${base}/${encodeURIComponent(sessionId)}/form/${encodeURIComponent(formId)}/reply`, { answer });
      return;
    } catch (error) {
      failure = error;
    }
  }
  throw failure;
}
