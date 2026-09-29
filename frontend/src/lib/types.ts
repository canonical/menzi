export interface Org {
  id: string;
  name: string;
  slug: string;
}

export interface User {
  id: string;
  email: string;
  name: string;
  avatar_url?: string;
}

export interface Preview {
  id: string;
  project_id: string;
  commit_sha?: string;
  branch?: string;
  status: string;
  mode: string;
  url: string;
  created_at: string;
}

export interface EnvResponse<T> {
  success: boolean;
  message: string;
  data: T | null;
}

export interface ComponentState {
  name: string;
  status: string;
  health: string;
}

export interface ExposureState {
  name: string;
  url: string;
  as_type: string;
}

export interface EnvironmentState {
  name: string;
  status: string;
  components: ComponentState[];
  exposures: ExposureState[];
}

export interface ComponentResult {
  name: string;
  success: boolean;
  message: string;
}

export interface LaunchResult {
  success: boolean;
  components: ComponentResult[];
  exposures: ExposureState[];
}

export interface RelaunchDiff {
  changed_inputs: string[];
  components_to_rebuild: string[];
  components_to_restart: string[];
  components_to_skip: string[];
}

export interface EnvExecResult {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export interface EnvLogs {
  component: string;
  lines: string[];
  truncated: boolean;
}

export interface Project {
  id: string;
  org_id: string;
  name: string;
  slug: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProjectInput {
  orgId: string;
  name: string;
  slug: string;
  description?: string;
}

export interface ListProjectsInput {
  orgId?: string;
  search?: string;
}

export interface OpencodeSession {
  id: string;
  projectID?: string;
  cost?: number;
  time?: { created?: number; updated?: number };
  location?: { directory?: string };
}

export interface OpencodeAgent {
  id: string;
  name: string;
  description?: string;
  mode?: string;
}

export interface OpencodeModel {
  id: string;
  name?: string;
  providerID?: string;
  status?: string;
  enabled?: boolean;
  capabilities?: { tools?: boolean; input?: string[]; output?: string[] };
}

export type ToolState =
  | { status: 'pending'; input: Record<string, unknown> }
  | { status: 'running'; input: Record<string, unknown>; title?: string }
  | {
      status: 'completed';
      input: Record<string, unknown>;
      output: string;
      title?: string;
      metadata?: Record<string, unknown>;
      time?: { start?: number; end?: number };
    }
  | {
      status: 'error';
      input: Record<string, unknown>;
      error: string;
      title?: string;
      metadata?: Record<string, unknown>;
      time?: { start?: number; end?: number };
    };

export interface TextPart {
  type: 'text';
  text: string;
}

export interface ReasoningPart {
  type: 'reasoning';
  text: string;
}

export interface ToolPart {
  type: 'tool';
  id: string;
  sessionID?: string;
  messageID?: string;
  callID?: string;
  tool: string;
  state?: ToolState;
}

export interface StepPart {
  type: 'step-start' | 'step-finish';
  id: string;
}

export interface CompactionPart {
  type: 'compaction';
  status?: string;
  error?: string;
}

export type MessagePart = TextPart | ToolPart | ReasoningPart | StepPart | CompactionPart;

export interface MessageInfo {
  id: string;
  sessionID: string;
  role: 'user' | 'assistant' | 'system' | string;
  agent?: string;
  model?: { providerID?: string; modelID?: string; id?: string };
  finish?: string;
  error?: { name?: string; data?: { message?: string } | string; message?: string };
  cost?: number;
  tokens?: Record<string, number>;
  time?: { created?: number; completed?: number };
}

export interface OpencodeMessage {
  info: MessageInfo;
  parts: MessagePart[];
}

export interface OpencodeVcsFile {
  file: string;
  status?: string;
  additions?: number;
  deletions?: number;
}

export interface TunnelMessage {
  message_type: 'request' | 'response' | 'event' | 'heartbeat' | 'error';
  session_id: string;
  payload: Record<string, unknown>;
  sequence: number;
}

export interface EnvSpecList {
  specs: string[];
}