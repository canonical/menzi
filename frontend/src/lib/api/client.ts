import { ApiError } from './errors';

const BASE_URL = import.meta.env.VITE_API_URL || '';

export const UNAUTHORIZED_EVENT = 'menzi:unauthorized';

const CSRF_COOKIE = 'menzi_csrf';
const CSRF_HEADER = 'x-menzi-csrf';

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

const DEV_SESSION_CREDENTIAL = 'menzi-dev-session';

interface ApiErrorBody {
  error?: { message?: string; code?: string } | string;
  message?: string;
  code?: string;
}

export function readCookie(name: string): string | null {
  if (typeof document === 'undefined') return null;
  for (const part of document.cookie.split(';')) {
    const [key, ...rest] = part.trim().split('=');
    if (key === name) return rest.join('=');
  }
  return null;
}

async function parseError(response: Response): Promise<ApiError> {
  let message = response.statusText;
  let code: string | undefined;
  const text = await response.text();
  if (text) {
    try {
      const body = JSON.parse(text) as ApiErrorBody;
      const nested = typeof body.error === 'object' ? body.error : undefined;
      const flat = typeof body.error === 'string' ? body.error : undefined;
      message = nested?.message ?? flat ?? body.message ?? message;
      code = nested?.code ?? body.code;
    } catch {
      message = text;
    }
  }
  return new ApiError(response.status, message, code);
}

function sessionCredential(): string {
  if (typeof document === 'undefined') return DEV_SESSION_CREDENTIAL;
  return readCookie(CSRF_COOKIE) ?? DEV_SESSION_CREDENTIAL;
}

const API = 'api';
const SESSION = 'session';
const PROXY_PREFIXES = [`/${SESSION}/`, `/${API}/${SESSION}/`];

function goesToSessionProxy(path: string): boolean {
  return PROXY_PREFIXES.some((prefix) => path.startsWith(prefix) && path.length > prefix.length);
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const method = options.method ?? 'GET';
  const headers = new Headers(options.headers);
  headers.set('Content-Type', 'application/json');

  if (!SAFE_METHODS.has(method.toUpperCase())) {
    const csrf = readCookie(CSRF_COOKIE);
    if (csrf) headers.set(CSRF_HEADER, csrf);
  }

  if (goesToSessionProxy(path)) {
    headers.set('Authorization', `Bearer ${sessionCredential()}`);
  }

  const response = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
    credentials: 'include',
  });

  if (response.status === 401 && typeof window !== 'undefined') {
    window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
  }

  if (!response.ok) {
    throw await parseError(response);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json();
}

export const http = {
  get: <T>(path: string) => request<T>(path),
  getWithBody: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'GET', body: JSON.stringify(body) }),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, {
      method: 'POST',
      body: body === undefined ? undefined : JSON.stringify(body),
    }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
};
