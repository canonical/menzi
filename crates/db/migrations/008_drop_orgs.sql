ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_org_id_slug_key;
ALTER TABLE projects DROP COLUMN IF EXISTS org_id;

CREATE UNIQUE INDEX IF NOT EXISTS projects_slug_key ON projects (slug);

ALTER TABLE users DROP COLUMN IF EXISTS org_id;

DROP INDEX IF EXISTS idx_llm_usage_org;
ALTER TABLE llm_usage DROP COLUMN IF EXISTS org_id;

DROP INDEX IF EXISTS idx_audit_org;
ALTER TABLE audit_logs DROP COLUMN IF EXISTS org_id;

ALTER TABLE llm_credentials DROP COLUMN IF EXISTS org_id;
ALTER TABLE templates DROP COLUMN IF EXISTS org_id;

DROP TABLE IF EXISTS orgs;
