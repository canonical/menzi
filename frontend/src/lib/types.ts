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

export type WorkspaceStatus =
  | 'requested'
  | 'provisioning'
  | 'ready'
  | 'running'
  | 'idle'
  | 'archived'
  | 'deleted';

export interface Workspace {
  id: string;
  user_id: string;
  project_id: string;
  name: string;
  status: WorkspaceStatus;
  instance_name?: string | null;
  endpoint?: string | null;
  branch?: string | null;
  commit_sha?: string | null;
  last_error?: string | null;
  created_at: string;
  updated_at: string;
}

export interface WorkspaceSession {
  id: string;
  parentID?: string | null;
  title?: string | null;
  directory?: string | null;
  time?: { created?: number; updated?: number };
}

export interface PromptOutcome {
  session_id: string;
  message_id?: string | null;
  finish_reason?: string | null;
  error?: string | null;
}

export type WorkspaceTerminalStatus = 'open' | 'running' | 'closed' | 'error';

export interface WorkspaceTerminalChunk {
  seq: number;
  stream: string;
  text: string;
}

export interface WorkspaceTerminalSnapshot {
  id: string;
  status: WorkspaceTerminalStatus;
  cwd: string;
  cols: number;
  rows: number;
  last_seq: number;
  text: string;
}

export interface WorkspaceTerminalOutput {
  id: string;
  status: WorkspaceTerminalStatus;
  last_seq: number;
  chunks: WorkspaceTerminalChunk[];
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
  name: string;
  slug: string;
  description: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateProjectInput {
  name: string;
  slug: string;
  description?: string;
}

export interface ListProjectsInput {
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
  /** Epoch milliseconds. `end` is missing until the thought finishes. */
  time?: { start?: number; end?: number };
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
  /** Attached to user messages; carries the file changes the turn produced. */
  summary?: { diffs?: Record<string, unknown>[]; title?: string; body?: string };
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
