-- Development seed data for the Menzi web UI.
-- Idempotent: safe to run repeatedly.

INSERT INTO orgs (id, name, slug)
VALUES ('11111111-1111-1111-1111-111111111111', 'Acme Corp', 'acme')
ON CONFLICT (id) DO NOTHING;

INSERT INTO orgs (id, name, slug)
VALUES ('22222222-2222-2222-2222-222222222222', 'Globex', 'globex')
ON CONFLICT (id) DO NOTHING;

INSERT INTO projects (id, org_id, name, slug, description)
VALUES (
  'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
  '11111111-1111-1111-1111-111111111111',
  'Storefront',
  'storefront',
  'Customer-facing web application and checkout service.'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO projects (id, org_id, name, slug, description)
VALUES (
  'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
  '11111111-1111-1111-1111-111111111111',
  'Billing Service',
  'billing-service',
  'Invoicing, dunning and payment reconciliation.'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO projects (id, org_id, name, slug, description)
VALUES (
  'cccccccc-cccc-cccc-cccc-cccccccccccc',
  '22222222-2222-2222-2222-222222222222',
  'Mobile App',
  'mobile-app',
  'iOS and Android client for the Globex platform.'
)
ON CONFLICT (id) DO NOTHING;

-- ada@acme.example signs in with the password "menzi-development-password".
-- The hash below is Argon2id of that exact string; the dollar signs are
-- doubled so psql does not read them as variables.
INSERT INTO users (id, org_id, email, name, password_hash, email_verified_at)
VALUES (
  'dddddddd-dddd-dddd-dddd-dddddddddddd',
  '11111111-1111-1111-1111-111111111111',
  'ada@acme.example',
  'Ada Lovelace',
  '@@argon2id@@v=19@@m=19456,t=2,p=1@@9/pbJcGCERy1X+w9fEMkOA@@8i/pfn4tTaPhc0CmqghSIvszfbSa/6gUNDvEd7lCxEU',
  now()
)
ON CONFLICT (id) DO NOTHING;
