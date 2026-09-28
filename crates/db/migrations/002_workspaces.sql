CREATE TABLE workspaces (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id          UUID NOT NULL REFERENCES projects(id),
    user_id             UUID NOT NULL REFERENCES users(id),
    name                TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'requested'
                        CHECK (status IN ('requested', 'provisioning', 'ready', 'running', 'idle', 'archived', 'deleted')),
    lxd_instance_name   TEXT,
    lxd_project         TEXT,
    template_id         UUID,
    branch              TEXT,
    commit_sha          TEXT,
    metadata            JSONB NOT NULL DEFAULT '{}',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at          TIMESTAMPTZ
);

CREATE INDEX idx_workspaces_project ON workspaces(project_id);
CREATE INDEX idx_workspaces_user ON workspaces(user_id);
CREATE INDEX idx_workspaces_status ON workspaces(status) WHERE deleted_at IS NULL;

CREATE TABLE environments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id    UUID NOT NULL REFERENCES workspaces(id),
    name            TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'stopped'
                    CHECK (status IN ('stopped', 'building', 'starting', 'ready', 'degraded', 'failed', 'tearing_down')),
    devenv_yaml     TEXT,
    components      JSONB NOT NULL DEFAULT '[]',
    exposures       JSONB NOT NULL DEFAULT '[]',
    last_launch_at   TIMESTAMPTZ,
    last_launch_result JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE previews (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    workspace_id    UUID REFERENCES workspaces(id),
    created_by      UUID NOT NULL REFERENCES users(id),
    commit_sha      TEXT,
    branch          TEXT,
    status          TEXT NOT NULL DEFAULT 'building'
                    CHECK (status IN ('building', 'ready', 'failed', 'torn_down')),
    mode            TEXT NOT NULL DEFAULT 'pinned' CHECK (mode IN ('pinned', 'live')),
    share_settings  JSONB NOT NULL DEFAULT '{}',
    expires_at      TIMESTAMPTZ,
    lxd_instance_name TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
