import { http } from './client';
import { ApiError } from './errors';
import { apiPaths } from '../routes';
import type {
  EnvExecResult,
  EnvLogs,
  EnvResponse,
  EnvSpecList,
  EnvironmentState,
  LaunchResult,
  RelaunchDiff,
} from '../types';

function unwrap<T>(response: EnvResponse<T>, status: number): T {
  if (!response.success || response.data === null) {
    throw new ApiError(status, response.message);
  }
  return response.data;
}

export async function listEnvSpecs(): Promise<string[]> {
  const response = await http.get<EnvResponse<EnvSpecList>>(apiPaths.env.specs());
  if (!response.success || response.data === null) {
    throw new ApiError(200, response.message);
  }
  return response.data.specs;
}

export async function getEnvStatus(
  sessionId: string,
  environmentName: string,
): Promise<EnvironmentState> {
  const response = await http.getWithBody<EnvResponse<EnvironmentState>>(apiPaths.env.status(), {
    session_id: sessionId,
    environment_name: environmentName,
  });
  return unwrap(response, 200);
}

export async function launchEnv(
  sessionId: string,
  environmentName: string,
): Promise<LaunchResult> {
  const response = await http.post<EnvResponse<LaunchResult>>(apiPaths.env.launch(), {
    session_id: sessionId,
    environment_name: environmentName,
  });
  return unwrap(response, 200);
}

export async function relaunchEnv(
  sessionId: string,
  environmentName: string,
): Promise<RelaunchDiff> {
  const response = await http.post<EnvResponse<RelaunchDiff>>(apiPaths.env.relaunch(), {
    session_id: sessionId,
    environment_name: environmentName,
    reset_data: false,
  });
  return unwrap(response, 200);
}

export async function getEnvLogs(
  sessionId: string,
  environmentName: string,
  component: string,
  tail?: number,
): Promise<EnvLogs> {
  const response = await http.post<EnvResponse<EnvLogs>>(apiPaths.env.logs(), {
    session_id: sessionId,
    environment_name: environmentName,
    component,
    tail: tail ?? 200,
  });
  return unwrap(response, 200);
}

export async function execInEnv(
  sessionId: string,
  environmentName: string,
  component: string,
  command: string[],
): Promise<EnvExecResult> {
  const response = await http.post<EnvResponse<EnvExecResult>>(apiPaths.env.exec(), {
    session_id: sessionId,
    environment_name: environmentName,
    component,
    command,
  });
  return unwrap(response, 200);
}
