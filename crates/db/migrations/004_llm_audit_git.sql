CREATE TABLE llm_credentials (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID REFERENCES orgs(id),
    project_id      UUID REFERENCES projects(id),
    user_id         UUID REFERENCES users(id),
    provider        TEXT NOT NULL,
    name            TEXT NOT NULL,
    encrypted_key   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE llm_usage (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    project_id      UUID,
    user_id         UUID,
    session_id      UUID,
    feature         TEXT NOT NULL,
    provider        TEXT NOT NULL,
    model           TEXT NOT NULL,
    input_tokens    INT NOT NULL,
    output_tokens   INT NOT NULL,
    cost_usd        NUMERIC(10,6) NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_usage_org ON llm_usage(org_id, created_at);
CREATE INDEX idx_llm_usage_project ON llm_usage(project_id, created_at);
CREATE INDEX idx_llm_usage_user ON llm_usage(user_id, created_at);

CREATE TABLE audit_logs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id          UUID NOT NULL,
    user_id         UUID,
    session_id      UUID,
    action          TEXT NOT NULL,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT,
    details         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_audit_org ON audit_logs(org_id, created_at);
CREATE INDEX idx_audit_user ON audit_logs(user_id, created_at);
CREATE INDEX idx_audit_session ON audit_logs(session_id, created_at);

CREATE TABLE git_repos (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    provider        TEXT NOT NULL CHECK (provider IN ('github', 'gitlab')),
    external_id     TEXT NOT NULL,
    name            TEXT NOT NULL,
    default_branch  TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, provider, external_id)
);

CREATE TABLE git_prs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repo_id         UUID NOT NULL REFERENCES git_repos(id),
    external_id     TEXT NOT NULL,
    number          INT NOT NULL,
    title           TEXT NOT NULL,
    branch          TEXT NOT NULL,
    head_sha        TEXT NOT NULL,
    status          TEXT NOT NULL,
    preview_id      UUID REFERENCES previews(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE notifications (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id),
    title           TEXT NOT NULL,
    body            TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    priority        TEXT NOT NULL DEFAULT 'normal',
    read            BOOLEAN NOT NULL DEFAULT FALSE,
    action_url      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE notification_preferences (
    user_id         UUID PRIMARY KEY REFERENCES users(id),
    web_push_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    email_enabled   BOOLEAN NOT NULL DEFAULT TRUE,
    chat_enabled    BOOLEAN NOT NULL DEFAULT FALSE,
    muted_types     JSONB NOT NULL DEFAULT '[]',
    quiet_hours_start TEXT,
    quiet_hours_end TEXT,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE web_push_subscriptions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id),
    endpoint        TEXT NOT NULL,
    p256dh          TEXT NOT NULL,
    auth            TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_type        TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'running', 'completed', 'failed', 'cancelled')),
    payload         JSONB NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at      TIMESTAMPTZ,
    completed_at    TIMESTAMPTZ,
    error           TEXT
);

CREATE TABLE templates (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID REFERENCES projects(id),
    org_id          UUID REFERENCES orgs(id),
    name            TEXT NOT NULL,
    description     TEXT,
    devenv_yaml     TEXT NOT NULL,
    is_public       BOOLEAN NOT NULL DEFAULT FALSE,
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
