-- Development seed data for the Menzi web UI.
-- Idempotent: safe to run repeatedly.

INSERT INTO projects (id, name, slug, description)
VALUES (
  'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  'Storefront',
  'storefront',
  'Customer-facing web application and checkout service.'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO projects (id, name, slug, description)
VALUES (
  'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
  'Billing Service',
  'billing-service',
  'Invoicing, dunning and payment reconciliation.'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO projects (id, name, slug, description)
VALUES (
  'cccccccc-cccc-cccc-cccc-cccccccccccc',
  'Mobile App',
  'mobile-app',
  'iOS and Android client for the platform.'
)
ON CONFLICT (id) DO NOTHING;

-- ada@acme.example signs in with the password "menzi-development-password".
-- The hash below is Argon2id of that exact string; the dollar signs are
-- doubled so psql does not read them as variables.
INSERT INTO users (id, email, name, password_hash, email_verified_at)
VALUES (
  'dddddddd-dddd-dddd-dddd-dddddddddddd',
  'ada@acme.example',
  'Ada Lovelace',
  '@@argon2id@@v=19@@m=19456,t=2,p=1@@9/pbJcGCERy1X+w9fEMkOA@@8i/pfn4tTaPhc0CmqghSIvszfbSa/6gUNDvEd7lCxEU',
  now()
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO project_members (project_id, user_id, role)
VALUES
  ('aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'owner'),
  ('bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'owner'),
  ('cccccccc-cccc-cccc-cccc-cccccccccccc', 'dddddddd-dddd-dddd-dddd-dddddddddddd', 'owner')
ON CONFLICT (project_id, user_id) DO NOTHING;
