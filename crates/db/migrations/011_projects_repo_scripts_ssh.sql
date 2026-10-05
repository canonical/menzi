ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS repository_url TEXT;

CREATE TABLE IF NOT EXISTS project_development_scripts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    relative_path TEXT,
    body TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('imported', 'manual')),
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_project_dev_scripts_project
    ON project_development_scripts(project_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS user_ssh_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    public_key TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    private_key_encrypted TEXT NOT NULL,
    passphrase_encrypted TEXT,
    is_default BOOLEAN NOT NULL DEFAULT false,
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_user_ssh_keys_user
    ON user_ssh_keys(user_id, created_at DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ssh_keys_one_default
    ON user_ssh_keys(user_id)
    WHERE is_default = true;
