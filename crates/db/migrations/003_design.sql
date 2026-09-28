CREATE TABLE design_revisions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    revision_number INT NOT NULL,
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, revision_number)
);

CREATE TABLE design_clauses (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id          UUID NOT NULL REFERENCES projects(id),
    clause_id           TEXT NOT NULL,
    version             INT NOT NULL,
    title               TEXT NOT NULL,
    status              TEXT NOT NULL DEFAULT 'ratified'
                        CHECK (status IN ('ratified', 'implementing', 'enforced', 'deprecated', 'superseded')),
    level               TEXT NOT NULL CHECK (level IN ('must', 'should')),
    visibility          TEXT NOT NULL DEFAULT 'project' CHECK (visibility IN ('project', 'private')),
    scope               JSONB NOT NULL DEFAULT '{}',
    approvers           JSONB NOT NULL DEFAULT '[]',
    statement           TEXT NOT NULL,
    rationale_ref       TEXT,
    verification        JSONB NOT NULL DEFAULT '{}',
    supersedes          TEXT,
    trial_until         TIMESTAMPTZ,
    revision_id         UUID NOT NULL REFERENCES design_revisions(id),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, clause_id, version)
);

CREATE TABLE design_amendments (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    amendment_id    TEXT NOT NULL,
    clause_id       TEXT,
    action          TEXT NOT NULL CHECK (action IN ('add', 'edit', 'deprecate', 'supersede')),
    status          TEXT NOT NULL DEFAULT 'draft'
                    CHECK (status IN ('draft', 'open', 'in_review', 'approved', 'ratified', 'rejected', 'withdrawn')),
    redline_diff    JSONB,
    rationale       TEXT,
    impact_report   JSONB,
    approvals       JSONB NOT NULL DEFAULT '[]',
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE design_waivers (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    clause_id       TEXT NOT NULL,
    paths           JSONB NOT NULL DEFAULT '[]',
    reason          TEXT NOT NULL,
    until           TIMESTAMPTZ NOT NULL,
    granted_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE design_baselines (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    name            TEXT NOT NULL,
    revision_id     UUID NOT NULL REFERENCES design_revisions(id),
    branch_patterns JSONB NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE design_conformance_reports (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    revision_id     UUID NOT NULL REFERENCES design_revisions(id),
    target_type     TEXT NOT NULL CHECK (target_type IN ('pr', 'commit', 'branch')),
    target_id       TEXT NOT NULL,
    findings        JSONB NOT NULL DEFAULT '[]',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE design_discussions (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES projects(id),
    title           TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'proposed', 'abandoned')),
    created_by      UUID NOT NULL REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
