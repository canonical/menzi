ALTER TABLE users ALTER COLUMN org_id DROP NOT NULL;

ALTER TABLE users ADD COLUMN password_hash TEXT;
ALTER TABLE users ADD COLUMN email_verified_at TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN password_changed_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE UNIQUE INDEX users_email_lower ON users (lower(email));

DROP INDEX IF EXISTS idx_session_tokens_token;
DROP INDEX IF EXISTS idx_session_tokens_user;

ALTER TABLE session_tokens RENAME TO auth_sessions;
ALTER TABLE auth_sessions DROP COLUMN token;

ALTER TABLE auth_sessions ADD COLUMN token_hash TEXT NOT NULL;
ALTER TABLE auth_sessions ADD COLUMN csrf_hash TEXT NOT NULL;
ALTER TABLE auth_sessions ADD COLUMN provider_id TEXT NOT NULL DEFAULT 'password';
ALTER TABLE auth_sessions ADD COLUMN last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now();
ALTER TABLE auth_sessions ADD COLUMN user_agent TEXT;
ALTER TABLE auth_sessions ADD COLUMN ip TEXT;

CREATE UNIQUE INDEX idx_auth_sessions_token_hash ON auth_sessions (token_hash);
CREATE INDEX idx_auth_sessions_user_active
    ON auth_sessions (user_id) WHERE revoked_at IS NULL;

CREATE TABLE auth_identities (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id   TEXT NOT NULL,
    subject       TEXT NOT NULL,
    email_at_link TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider_id, subject)
);

CREATE INDEX idx_auth_identities_user ON auth_identities (user_id);

CREATE TABLE auth_oidc_states (
    state         TEXT PRIMARY KEY,
    provider_id   TEXT NOT NULL,
    nonce         TEXT NOT NULL,
    code_verifier TEXT NOT NULL,
    redirect_to   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at    TIMESTAMPTZ NOT NULL
);

CREATE TABLE auth_login_attempts (
    id         BIGSERIAL PRIMARY KEY,
    email      TEXT NOT NULL,
    succeeded  BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_auth_login_attempts_email
    ON auth_login_attempts (lower(email), created_at DESC);

CREATE TABLE auth_password_resets (
    token_hash TEXT PRIMARY KEY,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);
