import { http } from './client';
import { apiPaths } from '../routes';
import type { Org } from '../types';

export async function listOrgs(): Promise<Org[]> {
  return http.get<Org[]>(apiPaths.orgs.list());
}

export async function getOrg(orgId: string): Promise<Org> {
  return http.get<Org>(apiPaths.orgs.detail(orgId));
}