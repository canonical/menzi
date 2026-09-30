CREATE UNIQUE INDEX idx_workspaces_one_per_user_project
    ON workspaces (project_id, user_id)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_workspaces_updated_at
    ON workspaces (updated_at)
    WHERE deleted_at IS NULL;

CREATE TABLE workspace_audit (
    id            BIGSERIAL PRIMARY KEY,
    workspace_id  UUID REFERENCES workspaces(id),
    user_id       UUID REFERENCES users(id),
    project_id    UUID NOT NULL REFERENCES projects(id),
    instance_name TEXT,
    action        TEXT NOT NULL,
    principal     TEXT NOT NULL,
    detail        TEXT NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_workspace_audit_workspace ON workspace_audit (workspace_id);
CREATE INDEX idx_workspace_audit_created ON workspace_audit (created_at DESC);

CREATE TABLE workspace_sessions (
    id                UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id      UUID NOT NULL REFERENCES workspaces(id),
    opencode_session  TEXT NOT NULL,
    title             TEXT,
    kind              TEXT NOT NULL DEFAULT 'interactive'
                      CHECK (kind IN ('interactive', 'autonomous')),
    principal         TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'active'
                      CHECK (status IN ('active', 'ended', 'failed')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at          TIMESTAMPTZ
);

ALTER TABLE workspace_sessions
    ADD COLUMN user_id UUID REFERENCES users(id),
    ADD COLUMN project_id UUID REFERENCES projects(id),
    ADD COLUMN endpoint TEXT,
    ADD COLUMN instance_name TEXT;

CREATE UNIQUE INDEX idx_workspace_sessions_opencode
    ON workspace_sessions (opencode_session);
CREATE INDEX idx_workspace_sessions_workspace
    ON workspace_sessions (workspace_id);
