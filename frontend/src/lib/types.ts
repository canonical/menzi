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

export interface Project {
  id: string;
  name: string;
  slug: string;
  description?: string;
  role: string;
}

export interface Workspace {
  id: string;
  project_id: string;
  user_id: string;
  name: string;
  status: string;
  branch?: string;
  commit_sha?: string;
  created_at: string;
}

export interface Environment {
  id: string;
  workspace_id: string;
  name: string;
  status: string;
  components: EnvironmentComponent[];
  exposures: Exposure[];
}

export interface EnvironmentComponent {
  name: string;
  status: string;
  health: string;
  uptime_secs: number;
}

export interface Exposure {
  name: string;
  url: string;
  as_type: string;
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

export interface DesignClause {
  id: string;
  clause_id: string;
  title: string;
  status: string;
  level: string;
  scope: string[];
  version: number;
}

export interface DesignAmendment {
  id: string;
  amendment_id: string;
  clause_id?: string;
  action: string;
  status: string;
  created_by: string;
}

export interface Session {
  id: string;
  project_id: string;
  name: string;
  status: string;
  branch?: string;
  created_at: string;
}

export interface Message {
  id: string;
  role: string;
  content: string;
  timestamp: string;
}

export interface Notification {
  id: string;
  title: string;
  body: string;
  type: string;
  priority: string;
  read: boolean;
  created_at: string;
  action_url?: string;
}
