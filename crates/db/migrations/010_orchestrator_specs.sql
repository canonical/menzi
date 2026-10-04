CREATE TABLE orchestrator_env_specs (
    name TEXT PRIMARY KEY,
    spec JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
