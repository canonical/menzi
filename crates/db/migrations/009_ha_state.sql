CREATE TABLE preview_registry (
    id TEXT PRIMARY KEY,
    project_id UUID NOT NULL,
    commit_sha TEXT,
    branch TEXT,
    status TEXT NOT NULL,
    mode TEXT NOT NULL,
    url TEXT NOT NULL,
    source_instance TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_preview_registry_project_created
    ON preview_registry (project_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE TABLE llm_gateway_policies (
    scope_key TEXT PRIMARY KEY,
    allowed_models JSONB NOT NULL DEFAULT '[]',
    denied_models JSONB NOT NULL DEFAULT '[]',
    data_classification TEXT NOT NULL DEFAULT 'internal',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE llm_gateway_budgets (
    scope_key TEXT PRIMARY KEY,
    project_id UUID,
    user_id UUID,
    session_id UUID,
    feature TEXT NOT NULL,
    spent_usd NUMERIC(12,6) NOT NULL DEFAULT 0,
    budget_usd NUMERIC(12,6) NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE llm_gateway_usage (
    id BIGSERIAL PRIMARY KEY,
    scope_key TEXT NOT NULL,
    project_id UUID,
    user_id UUID,
    session_id UUID,
    feature TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INT NOT NULL,
    output_tokens INT NOT NULL,
    cost_usd NUMERIC(12,6) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_llm_gateway_usage_scope_created
    ON llm_gateway_usage (scope_key, created_at DESC);
