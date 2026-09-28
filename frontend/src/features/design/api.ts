import { api } from '../../lib/api';
import type { DesignClause, DesignAmendment } from '../../lib/types';

export async function listClauses(projectId: string): Promise<DesignClause[]> {
  return api.get<DesignClause[]>(`/api/v1/projects/${projectId}/design/clauses`);
}

export async function getClause(projectId: string, clauseId: string): Promise<DesignClause> {
  return api.get<DesignClause>(`/api/v1/projects/${projectId}/design/clauses/${clauseId}`);
}

export async function listAmendments(projectId: string): Promise<DesignAmendment[]> {
  return api.get<DesignAmendment[]>(`/api/v1/projects/${projectId}/design/amendments`);
}

export async function proposeAmendment(projectId: string, clauseId: string | null, action: string, redlineDiff: unknown): Promise<DesignAmendment> {
  return api.post<DesignAmendment>(`/api/v1/projects/${projectId}/design/amendments`, {
    clause_id: clauseId,
    action,
    redline_diff: redlineDiff,
  });
}

export async function approveAmendment(projectId: string, amendmentId: string): Promise<void> {
  await api.post(`/api/v1/projects/${projectId}/design/amendments/${amendmentId}/approve`, {});
}

export async function rejectAmendment(projectId: string, amendmentId: string): Promise<void> {
  await api.post(`/api/v1/projects/${projectId}/design/amendments/${amendmentId}/reject`, {});
}
