import { http } from './client';
import { apiPaths } from '../routes';
import type { TunnelMessage } from '../types';

export async function listTranscript(
  sessionId: string,
  after?: number,
): Promise<TunnelMessage[]> {
  const path = apiPaths.tunnel.transcript(sessionId);
  const query = after === undefined ? '' : `?after=${after}`;
  return http.get<TunnelMessage[]>(`${path}${query}`);
}

export function transcriptStreamUrl(sessionId: string, after: number): string {
  return `${apiPaths.tunnel.transcript(sessionId)}?after=${after}&follow=true`;
}
