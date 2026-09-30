/**
 * A full navigation is used rather than the router because the session cookie
 * is set by the response, and the router's client-side cache would still hold
 * the previous "anonymous" answer.
 */
export function navigateTo(path: string): void {
  window.location.assign(path);
}
