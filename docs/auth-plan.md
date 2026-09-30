# Authentication implementation plan

Technical plan for replacing the development identity bypass with real
authentication: email and password, plus OIDC (which makes Google SSO one
configuration entry).

Every API signature, SQL statement, and code sketch in this document was
compiled or executed against the versions in `Cargo.lock` before being written
down. Where an API surprised me, that is called out under
[Verified constraints](#verified-constraints) — do not skip it.

Scope is authentication. Authorization is called out where auth work exposes it
but is not part of this plan.

---

## Verified constraints

These were checked against the live toolchain, not recalled. Each one is a
place where the obvious code does not work.

### `argon2` is 0.6, and the salt argument is gone

`cargo add --dry-run argon2` resolves to **0.6.0**, not 0.5. In 0.6
`PasswordHasher::hash_password` takes **only the password**; `SaltString` no
longer exists. The 0.5 tutorials are wrong.

```rust
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

fn hash_password(password: &str) -> String {
    Argon2::default()
        .hash_password(password.as_bytes())
        .expect("hash")
        .to_string()
}

fn verify(stored: &str, candidate: &str) -> bool {
    let parsed = PasswordHash::new(stored).expect("parse the stored hash");
    Argon2::default()
        .verify_password(candidate.as_bytes(), &parsed)
        .is_ok()
}
```

Verified output of the default: `$argon2id$v=19$m=19456,t=2,p=1$…`, so the
default is already Argon2id at 19 MiB, 2 passes, 1 lane. Parameters are recorded
in the string, so raising them later does not invalidate existing hashes.

Explicit parameters, if the defaults ever need to change:

```rust
let params = argon2::Params::new(19 * 1024, 2, 1, Some(32)).expect("params");
let alg = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
let hash = alg.hash_password(b"pw").expect("hash").to_string();
```

### `rand` is 0.9 and `OsRng` is fallible

`rand`'s latest is 0.10 but the lock already carries 0.8.8, 0.9.5 and 0.10.3.
Pin `"0.9"` to reuse 0.9.5. In 0.9, `OsRng` implements `TryRngCore`, **not**
`RngCore`, so the 0.8 idiom `OsRng.fill_bytes(&mut b)` does not compile:

```rust
use rand::TryRngCore;
use rand::rngs::OsRng;

fn mint() -> String {
    let mut bytes = [0u8; 32];
    OsRng.try_fill_bytes(&mut bytes).expect("os randomness");
    format!("mz_{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}
```

`rand::rng().fill_bytes(&mut b)` also works but needs `use rand::RngCore;`.
Prefer `OsRng` for token material: it names the source.

### `sha2` must be pinned to 0.10

Latest is 0.11.0; the lock already has 0.10.9. Specify `sha2 = "0.10"` so the
build reuses it. `format!("{:x}", Sha256::digest(value.as_bytes()))` is correct
for 0.10 and is what the stores will use.

### `openidconnect` is 4.0.1

RustOIDC. Verified resolvable from crates.io.

### axum layer order: `.layer()` is outermost, so a gating layer on the outer router gates everything

This is the single most dangerous item. I built the obvious shape first and it
broke the health check:

```rust
Router::new()
    .route("/health", get(public))
    .route("/api/v1/auth/login", post(public))
    .merge(protected)
    .layer(middleware::from_fn_with_state(state, require_caller));  // WRONG
```

Actual result — `/health` and `/api/v1/auth/login` both returned 401, because
the outer layer is the outermost wrapper and ran for every route.

The correct shape gates only the merged subtree, and the outer layer never
rejects:

```rust
let protected = Router::new()
    .route("/api/v1/me", get(private))
    .route("/api/v1/thing", post(private))
    .layer(middleware::from_fn_with_state(state.clone(), require_csrf))
    .layer(middleware::from_fn_with_state(state.clone(), require_caller));

let app = Router::new()
    .route("/health", get(public))
    .route("/api/v1/auth/login", post(public))
    .merge(protected)
    .layer(middleware::from_fn(strip_identity_headers))
    .with_state(state);
```

Note the order of the two layers on `protected`: **the last `.layer()` is the
outermost.** `require_csrf` is added first, so `require_caller` runs first, so
an unauthenticated `POST` gets 401 rather than 403. That is the honest answer.

Verified behaviour of the correct shape:

| Request | Result |
| --- | --- |
| `GET /health`, no cookie | 200 |
| `POST /api/v1/auth/login`, no cookie | 200 |
| `GET /api/v1/me`, no cookie | 401 `{"error":{"code":"unauthenticated"}}` |
| `GET /api/v1/me`, wrong token | 401 |
| `GET /api/v1/me`, valid token | 200 with the session's user |
| `POST /api/v1/thing`, valid token, no CSRF | 403 |
| `POST /api/v1/thing`, valid token, wrong CSRF | 403 |
| `POST /api/v1/thing`, valid token, right CSRF | 200 |

### `headers: HeaderMap` in middleware is a clone

Stripping a header by mutating a `headers: HeaderMap` middleware parameter does
nothing to the request the handler sees. The parameter is a clone taken from the
request parts.

```rust
// WRONG: mutates a clone, the handler still sees the header
async fn strip(mut headers: HeaderMap, request: Request, next: Next) -> Response { … }

// RIGHT
async fn strip_identity_headers(mut request: Request, next: Next) -> Response {
    request.headers_mut().remove(HEADER_USER_ID);
    request.headers_mut().remove(HEADER_SERVICE);
    next.run(request).await
}
```

Verified: with `x-menzi-user-id: attacker` sent, a handler reading
`HeaderMap` reports the header absent.

`from_fn_with_state` handlers accept `State<T>` as an extractor, and
`Option<Extension<T>>` works when a value may not be present.

### No test databases, so the stores need in-memory implementations

There is no test container anywhere in the repo. Existing tests use
`sqlx::connect_lazy` pools and real `axum::serve` fakes on `127.0.0.1:0`. A lazy
pool cannot execute a query, so **any handler that touches the database cannot
be unit tested the way the repo tests today.**

`menzi-workspace` already solved this with a trait plus two implementations, and
auth must copy that shape or its tests will be integration tests that need a
database nobody runs:

```rust
#[async_trait]
pub trait WorkspaceStore: Send + Sync { … }

pub struct InMemoryWorkspaceStore { entries: Mutex<HashMap<WorkspaceKey, Workspace>> }

pub struct PostgresWorkspaceStore { pool: PgPool }
```

See W2.

### The caller is currently resolved twice per request

`WorkspaceProxy` owns a private `resolver` and builds its own
`PgPoolOptions::max_connections(2)` or `(4)`, separate from the pool core-api
was given. `forward_workspaces` resolves the caller itself. With a cookie, that
resolution is a database round trip, so a proxied request would do it twice
against two different pools.

W4 collapses this: the middleware resolves once, inserts `Caller`, and the proxy
reads the extension. The private resolver and the duplicate pool go away.

### `Config` is not where auth configuration belongs

`menzi_common::Config` is a flat `Deserialize` read by every service, and
`resolver_from_env` in `identity.rs` reads `std::env` directly. Auth config is
~14 variables and only core-api wants it, so `AuthConfig::from_env()` lives in
`crates/core-api/src/auth/config.rs`. This follows the existing precedent rather
than bloating a shared struct every service must parse.

---

## Architecture

```
browser
  │  cookie only; no token in JS
  ▼
core-api
  ├─ strip_identity_headers        outermost, never rejects
  ├─ public router                 /health, /api/v1/auth/*
  ├─ protected router              require_caller → require_csrf → handler
  └─ workspace proxy               reads Extension<Caller>, no second lookup
       │  x-menzi-user-id          (the only writer of this header)
       ▼
menzi-workspace :8096              trusts the header, loopback only
```

Browser-side session state is the server's answer and nothing else. No
`localStorage` token, no optimistic "we know who this is".

---

## Decisions

**D1 — httpOnly cookie, not a bearer token in the browser.** A token in
`localStorage` is readable by any script on the page, and this app renders model
output into the DOM. Cookie is `HttpOnly`, `SameSite=Lax`, `Secure` in
production, `Path=/`. CSRF closes by requiring `x-menzi-csrf` on every non-`GET`
request, matched against a second readable cookie. A cross-origin page cannot
set a custom header without a preflight, and no CORS headers are sent.

The `Authorization: Bearer` path stays for service calls and scripts, and
`menzi-workspace` keeps receiving identity headers rather than tokens.

`SameSite=Lax`, not `Strict`: the OIDC callback arrives as a cross-site `GET`
redirect from Google, and `Strict` would strip the cookie and loop forever.

**D2 — OIDC runs in core-api, not the browser.** Authorization code plus PKCE,
IdP tokens stay server-side, core-api issues our own cookie. One session
mechanism covers all three login methods, no IdP token reaches the browser, and
the frontend needs no OIDC library. Cost: a state store in Postgres and a token
exchange over `reqwest`, which is already a dependency.

**D3 — Google is an ordinary provider entry.** `issuer:
https://accounts.google.com`. No `Google` code path exists.

**D4 — Identities key on `(provider_id, subject)`, never on email.** An OIDC
login resolves to a user only through `auth_identities`. If there is no link:

| Situation | Outcome |
| --- | --- |
| no user with that email, `email_verified` true, `AUTO_PROVISION=1` | create the user, link the identity |
| no user with that email, unverified | refuse |
| a user with that email exists | refuse: "sign in with your password and link {provider} in Settings" |

Matching an OIDC login to an existing password account by email is account
takeover: any provider that lets a user set an arbitrary unverified email hands
them that account. v1 refuses. The upgrade path is an emailed one-time code, and
it is not in this plan.

**D5 — Argon2id for passwords, SHA-256 for tokens.** 32 random bytes,
base64url, prefixed `mz_`. Only the digest is stored, in a unique column, and
lookups are by digest. A database leak yields no live session.

**D6 — Registration mode is configuration.** `closed` (default), `open`, or
`invite`. `closed` answers 404 so the route is not probeable. A product decision,
not a technical one; production default is not chosen here.

**D7 — Login throttling is database backed.** Failures for the same lowercased
email inside a window lock it. `tower` limiters are process-local and wrong as
soon as there are two replicas.

**D8 — `MENZI_DEV_AUTH` becomes opt-in.** `dev.sh` currently defaults it to `1`,
which would mean the login page is never exercised in development. It flips to
`0`; the escape hatch stays.

---

## Data model

Two migrations. `007` also relaxes `users.org_id`, which is the blocker for
registration.

`006_session_tokens.sql` is already applied, so it must not be edited — sqlx
refuses to start with "migration 6 was previously applied but has been modified".
`007` renames and rewrites the table instead.

### `007_auth.sql`

```sql
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
```

`email` is stored lowercased by the write path; `users_email_lower` is what makes
that true. No `citext` extension dependency. No `CHECK (email = lower(email))`,
so the seed's existing mixed-case rows stay valid.

`csrf_hash` rather than the raw CSRF secret, for the same reason as the session
token: the readable cookie carries the value, the database does not.

`provider_id` on `auth_sessions` lets Settings say "signed in with Google"
instead of showing an unexplained row.

### The `org_id` problem, concretely

Registration inserts `(email, name, password_hash)` with no org. Consequences:

- `list_projects` joins `project_members`, so a new user gets an empty list. No
  change needed.
- `create_project` requires `org_id`, so the first project needs an org first.
- `list_orgs` currently returns **every org in the database**, which a new user
  must certainly not see.

So registration is followed by onboarding, not a redirect to a working page:

1. `create_org` sets `users.org_id` when it is null, so the first org is theirs.
2. `create_project` defaults `org_id` to the caller's when the body omits it, and
   400s when the caller has none.
3. `list_orgs` filters to the caller's org. There is no `org_members` table, so
   it filters on `orgs.id = <caller org>`. This also closes the existing leak.
4. `ProjectsPage` renders an empty state offering "Create an organisation".

`infra/local/seed.sql` inserts users with an explicit `org_id` and is unchanged.

---

## Backend

Domain primitives in `crates/auth`; HTTP in `core-api`.
`crates/auth/src/token.rs` is **deleted** — it models a plaintext, project-scoped
token, which is a different thing from a login session and is used nowhere.

```
crates/auth/src/
  lib.rs             role, permission, tenant unchanged
  password.rs        PasswordHash
  secret.rs          SessionSecret, digest
  record.rs          UserRecord, SessionRecord, NewSession, IdentityLink, OidcState

crates/core-api/src/auth/
  mod.rs             create_auth_router, request and response types
  config.rs          AuthConfig::from_env
  cookie.rs          Set-Cookie and Clear-Cookie, hand-written
  middleware.rs      require_caller, require_csrf, strip_identity_headers
  service.rs         AuthService
  accounts.rs        AccountStore trait, InMemoryAccountStore, PostgresAccountStore
  sessions.rs        SessionStore trait, InMemorySessionStore, PostgresSessionStore
  oidc_state.rs      OidcStateStore trait, InMemory, Postgres
  throttle.rs        ThrottleStore trait, InMemory, Postgres
  register.rs  login.rs  logout.rs  password.rs  devices.rs  providers.rs
  oidc.rs            OpenIdProvider trait, RustOidcProvider, FakeOidcProvider
  mailer.rs          Mailer trait, LogMailer, StubMailer
  clock.rs           Clock trait, SystemClock, FixedClock
```

### New dependencies

Workspace `Cargo.toml`: `argon2 = "0.6"`, `rand = "0.9"`, `sha2 = "0.10"`,
`openidconnect = "4"`. `base64 = "0.22"` is already there.

`crates/auth` gets `argon2`, `rand`, `sha2`, `base64`. `core-api` gets
`openidconnect` only; it never hashes a password itself, it calls
`menzi_auth::PasswordHash`.

### Store traits

Four traits, each with an in-memory and a Postgres implementation. This is what
makes W6 through W12 testable without a database.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserRecord {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub password_hash: Option<String>,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("duplicate: {0}")]
    Duplicate(String),
    #[error("not found")]
    NotFound,
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
}

#[async_trait]
pub trait AccountStore: Send + Sync {
    async fn find_by_email(&self, email: &str) -> Result<Option<UserRecord>, StoreError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<UserRecord>, StoreError>;
    async fn create(&self, email: &str, name: &str, password_hash: Option<&str>, verified: bool)
        -> Result<UserRecord, StoreError>;
    async fn set_password_hash(&self, id: Uuid, hash: &str) -> Result<(), StoreError>;
    async fn find_by_identity(&self, provider_id: &str, subject: &str)
        -> Result<Option<UserRecord>, StoreError>;
    async fn link_identity(&self, provider_id: &str, subject: &str, user_id: Uuid,
                            email: Option<&str>) -> Result<(), StoreError>;
    async fn list_identities(&self, user_id: Uuid) -> Result<Vec<IdentityLink>, StoreError>;
    async fn unlink_identity(&self, user_id: Uuid, provider_id: &str) -> Result<bool, StoreError>;
}
```

The Postgres implementation maps sqlx error code `23505` to
`StoreError::Duplicate` so the handlers can answer 409 without inspecting
constraint names.

```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn create(&self, new: NewSession) -> Result<SessionRecord, StoreError>;
    async fn find_active(&self, token_hash: &str) -> Result<Option<SessionRecord>, StoreError>;
    async fn touch(&self, id: Uuid) -> Result<(), StoreError>;
    async fn revoke(&self, id: Uuid) -> Result<(), StoreError>;
    async fn revoke_all_for(&self, user_id: Uuid, keep: Option<Uuid>) -> Result<u64, StoreError>;
    async fn list_for(&self, user_id: Uuid) -> Result<Vec<SessionRecord>, StoreError>;
    async fn purge_expired(&self) -> Result<u64, StoreError>;
}

#[async_trait]
pub trait OidcStateStore: Send + Sync {
    async fn put(&self, state: &OidcState) -> Result<(), StoreError>;
    /// Delete and return in one statement. A replayed state deletes zero rows
    /// and yields `None`, which is what makes the state single use.
    async fn take(&self, state: &str) -> Result<Option<OidcState>, StoreError>;
}

#[async_trait]
pub trait ThrottleStore: Send + Sync {
    async fn failures_since(&self, email: &str, since: DateTime<Utc>) -> Result<i64, StoreError>;
    async fn record(&self, email: &str, succeeded: bool) -> Result<(), StoreError>;
}
```

### The SQL

```sql
-- AccountStore::create  ($1 is already lower(trim($1)))
INSERT INTO users (email, name, password_hash, email_verified_at, password_changed_at)
VALUES ($1, $2, $3, CASE WHEN $4 THEN now() END, now())
RETURNING id, org_id, email, name, avatar_url, password_hash, email_verified_at, created_at;

-- AccountStore::find_by_email
SELECT id, org_id, email, name, avatar_url, password_hash, email_verified_at, created_at
FROM users WHERE lower(email) = lower($1);

-- AccountStore::find_by_identity
SELECT u.id, u.org_id, u.email, u.name, u.avatar_url, u.password_hash,
       u.email_verified_at, u.created_at
FROM users u
JOIN auth_identities i ON i.user_id = u.id
WHERE i.provider_id = $1 AND i.subject = $2;

-- AccountStore::link_identity
INSERT INTO auth_identities (provider_id, subject, user_id, email_at_link)
VALUES ($1, $2, $3, $4) ON CONFLICT (provider_id, subject) DO NOTHING;

-- SessionStore::create
INSERT INTO auth_sessions (user_id, token_hash, csrf_hash, provider_id, expires_at, user_agent, ip)
VALUES ($1, $2, $3, $4, $5, $6, $7)
RETURNING id, user_id, provider_id, created_at, expires_at, last_seen_at, user_agent, ip;

-- SessionStore::find_active
SELECT id, user_id, provider_id, created_at, expires_at, last_seen_at, user_agent, ip
FROM auth_sessions
WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now();

-- OidcStateStore::take
DELETE FROM auth_oidc_states
WHERE state = $1 AND expires_at > now()
RETURNING state, provider_id, nonce, code_verifier, redirect_to;

-- ThrottleStore::failures_since
SELECT count(*) FROM auth_login_attempts
WHERE lower(email) = lower($1) AND succeeded = false AND created_at > $2;
```

`ON CONFLICT (provider_id, subject) DO NOTHING` is safe here because
`link_identity` is only ever called after `find_by_identity` returned `None` for
that pair.

`ON CONFLICT` is *not* used for `users`: the conflict is on the
`users_email_lower` expression index, and naming the expression index as the
arbiter requires writing the index predicate out. Catching `23505` and mapping
to `StoreError::Duplicate` is less fragile, and the same lesson already bit
`PostgresWorkspaceStore::save`.

### `crates/auth` primitives

```rust
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn hash(plaintext: &str) -> crate::Result<Self> { … }
    pub fn verify(&self, plaintext: &str) -> bool { … }
    pub fn needs_rehash(&self) -> bool { … }
}

pub struct SessionSecret(String);

impl SessionSecret {
    pub fn mint() -> Self { /* 32 OsRng bytes, URL_SAFE_NO_PAD, "mz_" prefix */ }
    pub fn expose(&self) -> &str { &self.0 }
    pub fn digest(&self) -> String { /* format!("{:x}", Sha256::digest(...)) */ }
}
```

`verify` returns `false` on a malformed stored hash rather than erroring: a
corrupt row should deny access, not 500.

### `AuthService`

One struct the handlers call. It owns the stores, the clock, the mailer, and the
provider list, so a test drives the whole auth surface with in-memory stores, a
`FixedClock`, a `StubMailer`, and a `FakeOidcProvider`.

```rust
pub struct AuthService {
    accounts: Arc<dyn AccountStore>,
    sessions: Arc<dyn SessionStore>,
    states: Arc<dyn OidcStateStore>,
    throttle: Arc<dyn ThrottleStore>,
    clock: Arc<dyn Clock>,
    mailer: Arc<dyn Mailer>,
    providers: Vec<Arc<dyn OpenIdProvider>>,
    config: AuthConfig,
}

impl AuthService {
    pub async fn register(&self, req: RegisterRequest, meta: RequestMeta) -> Result<AuthOutcome, AuthError>;
    pub async fn login(&self, req: LoginRequest, meta: RequestMeta) -> Result<AuthOutcome, AuthError>;
    pub async fn logout(&self, token: Option<&str>) -> Result<(), AuthError>;
    pub async fn start_oidc(&self, provider_id: &str, redirect_to: Option<&str>) -> Result<String, AuthError>;
    pub async fn finish_oidc(&self, provider_id: &str, code: &str, state: &str, meta: RequestMeta)
        -> Result<OidcOutcome, AuthError>;
    pub async fn change_password(&self, caller: Uuid, current: &str, next: &str) -> Result<(), AuthError>;
    pub async fn request_reset(&self, email: &str) -> Result<(), AuthError>;
    pub async fn complete_reset(&self, token: &str, new_password: &str) -> Result<(), AuthError>;
    pub async fn devices(&self, caller: Uuid) -> Result<Vec<SessionRecord>, AuthError>;
    pub async fn revoke_device(&self, caller: Uuid, id: Uuid) -> Result<(), AuthError>;
}

pub struct AuthOutcome {
    pub user: UserRecord,
    pub session_token: SessionSecret,
    pub csrf_token: SessionSecret,
    pub expires_at: DateTime<Utc>,
    pub provider_id: String,
}

pub struct OidcOutcome {
    pub target: String,
    pub outcome: AuthOutcome,
}
```

`AuthOutcome` returns the secrets; only `cookie.rs` turns them into headers. The
service never touches `HeaderMap`, so it is fully testable.

### Middleware

`strip_identity_headers` is applied to the **whole** router and never rejects.
It is what makes D4-style trust safe: core-api is the only writer of
`x-menzi-user-id`, and `menzi-workspace` can trust it.

`require_caller` runs on the protected subtree:

```rust
pub async fn require_caller(
    State(resolver): State<Arc<dyn CallerResolver>>,
    mut request: Request,
    next: Next,
) -> Response {
    match resolver.resolve(request.headers()).await {
        Ok(caller) => {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        Err(IdentityError::Missing) => json_error(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "sign in to continue",
        ),
        Err(IdentityError::Invalid) => json_error(
            StatusCode::FORBIDDEN,
            "invalid_credentials",
            "the credentials were refused",
        ),
    }
}
```

The error body is nested (`{"error": {"code", "message"}}`) so the frontend can
branch on `code`. The existing `whoami` returns a flat string, which
`client.ts` already tolerates; both shapes are handled.

`require_csrf` runs **inside** `require_caller`, and skips `GET` and `HEAD`:

```rust
pub async fn require_csrf(request: Request, next: Next) -> Response {
    if matches!(request.method(), &Method::GET | &Method::HEAD) {
        return next.run(request).await;
    }
    let sent = request.headers().get(HEADER_CSRF).and_then(|v| v.to_str().ok());
    match (sent, cookie_value(request.headers(), CSRF_COOKIE)) {
        (Some(a), Some(b)) if a == b => next.run(request).await,
        _ => json_error(StatusCode::FORBIDDEN, "csrf", "the request could not be attributed"),
    }
}
```

The CSRF cookie is readable by design, so the comparison is a plain `==` and not
a constant-time compare. It is not a secret; the token is.

### Cookies

`Set-Cookie` is hand-written rather than pulling `axum-extra` for a feature
gated string:

```rust
pub fn session_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    format!(
        "{name}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    )
}

pub fn csrf_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> String {
    format!(
        "{name}={value}; Path=/; Max-Age={max_age}; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    )
}
```

`HttpOnly` on the session and deliberately absent on the CSRF cookie.
`Secure` is on whenever `MENZI_PUBLIC_BASE_URL` is `https`. `SameSite=Strict` is
never used, for the reason in D1.

### OIDC

`start`:

1. Look up the provider, 404 if unconfigured.
2. Mint `state`, `nonce`, and a PKCE `code_verifier`; the challenge is its S256
   digest via `sha2`.
3. `OidcStateStore::put` with a ten minute expiry.
4. 302 to the authorisation endpoint.

`callback`:

1. `OidcStateStore::take(state)`. `None` means unknown, expired, or replayed;
   all three are 400.
2. Check the state's `provider_id` matches the path, so a callback to `/google/`
   cannot consume a state minted for `okta`.
3. Exchange the code with `code_verifier`, `client_id`, `client_secret`,
   `redirect_uri`, `grant_type=authorization_code`.
4. Verify the id token: signature against discovery `jwks_uri`, then `iss`,
   `aud`, `exp`, and the state's `nonce`.
5. Resolve `(provider_id, sub)` per D4.
6. Mint a session, set the cookie, 302 to `redirect_to` or `/projects`.

Step 4 is where this goes wrong, and it is why the plan uses RustOIDC rather than
hand-rolling. `openidconnect` does discovery, the exchange, JWKS fetching, and
signature verification.

```rust
#[async_trait]
pub trait OpenIdProvider: Send + Sync {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    async fn authorization_url(&self, challenge: &str, state: &str, nonce: &str)
        -> Result<Url, AuthError>;
    async fn exchange(&self, code: &str, verifier: &str) -> Result<IdTokenClaims, AuthError>;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdTokenClaims {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub name: Option<String>,
    pub picture: Option<String>,
}
```

`FakeOidcProvider` in tests returns fixed claims and records the challenge,
state, nonce, and verifier it was handed, so the tests assert on what the
outbound request actually contained rather than on a string the test built.

`redirect_to` is validated against a same-origin allowlist before it is stored
and again before the final 302. A missing or disallowed value becomes
`/projects`. Without this the callback is an open redirect.

### Route table

Public:

```
GET    /health
GET    /api/v1/auth/providers
POST   /api/v1/auth/register
POST   /api/v1/auth/login
POST   /api/v1/auth/logout
POST   /api/v1/auth/password/forgot
POST   /api/v1/auth/password/reset
GET    /api/v1/auth/oidc/{provider}/start
GET    /api/v1/auth/oidc/{provider}/callback
```

`logout` is public on purpose: it must clear the cookie even when the session
has already expired, otherwise a user with a stale cookie cannot sign out.

Authenticated:

```
GET    /api/v1/me
GET    /api/v1/auth/session
GET    /api/v1/auth/sessions
DELETE /api/v1/auth/sessions/{id}
POST   /api/v1/auth/password/change
```

plus everything already in `create_router`.

`GET /api/v1/me` becomes the real user. Today it returns `{id, kind}`, which
carries no email or name, which is why `auth.ts` has a `setUser` that fills them
with empty strings.

```json
{
  "id": "uuid",
  "kind": "user",
  "email": "person@example.com",
  "name": "Person",
  "avatar_url": null,
  "org_id": "uuid",
  "role": "owner"
}
```

`role` is the caller's role in that org, read from `project_members`. There is no
org-level role, so with more than one project it is the highest role held.
Advisory only until authorization lands; the frontend does not gate on it.

`GET /api/v1/auth/providers` is what the login page renders. It never returns a
client id or secret.

```json
{ "registration": "open", "password": true, "oidc": [{ "id": "google", "label": "Google" }] }
```

### Semantics worth stating

**register** `{email, name, password}`
- 404 when mode is `closed`
- 409 when the email is taken. This enumerates accounts; see open questions.
- 400 when the password fails the rules: at least
  `MENZI_PASSWORD_MIN_LENGTH` (default 12), and not equal to the email local
  part
- 201 with `Set-Cookie`. The user lands signed in rather than on a login form.

**login** `{email, password}`
- 400 missing field, 401 wrong password, 423 locked
- Argon2 runs even for an unknown email, against a fixed dummy hash computed at
  startup, so response time does not reveal whether an account exists

**forgot** `{email}` — always 202, no enumeration. Writes a reset row and calls
the mailer. `LogMailer` writes the link to the log; there is no SMTP yet.

**password change** — on success, `revoke_all_for(caller, Some(current))` so
changing a password signs out other devices but not the one doing it.

### Configuration

```
MENZI_AUTH_MODE=token|dev              # default token
MENZI_DEV_USER_ID=<uuid>               # MENZI_AUTH_MODE=dev only
MENZI_PUBLIC_BASE_URL=https://menzi.example.com
MENZI_SESSION_COOKIE=menzi_session
MENZI_CSRF_COOKIE=menzi_csrf
HEADER_CSRF=x-menzi-csrf
MENZI_SESSION_TTL_HOURS=720
MENZI_SESSION_IDLE_HOURS=168
MENZI_REGISTRATION_MODE=closed|invite|open
MENZI_PASSWORD_MIN_LENGTH=12
MENZI_LOGIN_MAX_ATTEMPTS=10
MENZI_LOGIN_LOCKOUT_MINUTES=15
MENZI_OIDC_AUTO_PROVISION=0
MENZI_OIDC_PROVIDERS_FILE=/etc/menzi/oidc.json
MENZI_OIDC_PROVIDERS=<inline json>
MENZI_MAILER=log|stub|smtp
```

`MENZI_PUBLIC_BASE_URL` builds the OIDC `redirect_uri` and is validated at
startup, because a wrong value produces a confusing provider-side error rather
than a local one.

`invite` is declared but behaves as `closed` until the invite table exists. A
configurable-but-unimplemented mode is worse than an absent one.

Provider file:

```json
{
  "providers": [
    {
      "id": "google",
      "label": "Google",
      "issuer": "https://accounts.google.com",
      "client_id": "…",
      "client_secret": "…",
      "scopes": ["openid", "email", "profile"]
    }
  ]
}
```

---

## Frontend

### Files

```
frontend/src/lib/api/auth.ts            register, login, logout, providers, startOidc, devices
frontend/src/lib/api/auth.test.ts
frontend/src/lib/api/client.ts          credentials, CSRF, no localStorage token
frontend/src/lib/api/whoami.ts          the real user
frontend/src/stores/auth.ts             user and status, nothing persisted
frontend/src/stores/useSession.ts       replaces useCurrentUser
frontend/src/features/auth/AuthLayout.tsx
frontend/src/features/auth/LoginPage.tsx
frontend/src/features/auth/RegisterPage.tsx
frontend/src/features/auth/ForgotPasswordPage.tsx
frontend/src/features/auth/ResetPasswordPage.tsx
frontend/src/features/auth/ProviderButtons.tsx
frontend/src/features/auth/AuthCallbackPage.tsx
frontend/src/components/RequireAuth.tsx
frontend/src/features/settings/AccountSection.tsx
```

`auth.ts` must live in `src/lib`. `frontend/src/lib/backend-contract.test.ts`
scans `src/lib` for string literals starting with `/api` and fails if the
backend does not register them; an auth client in `src/features` would be
invisible to that check and the auth routes would drift out of the committed
inventory silently.

### `client.ts`

Three changes:

```ts
function readCookie(name: string): string | null {
  for (const part of document.cookie.split(';')) {
    const [key, ...rest] = part.trim().split('=');
    if (key === name) return rest.join('=');
  }
  return null;
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const method = options.method ?? 'GET';
  const headers = new Headers(options.headers);
  headers.set('Content-Type', 'application/json');
  if (method !== 'GET' && method !== 'HEAD') {
    const csrf = readCookie('menzi_csrf');
    if (csrf) headers.set('x-menzi-csrf', csrf);
  }

  const response = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
    credentials: 'include',
  });

  if (response.status === 401) {
    window.dispatchEvent(new Event(UNAUTHORIZED_EVENT));
  }
  …
}
```

Three removals/additions that matter:

- **`credentials: 'include'`.** The Vite dev server is on `:5173` and the API on
  `:8080`. Same-*site* is computed from scheme plus registrable domain and
  ignores the port, so `SameSite=Lax` still holds, but `fetch` will not send or
  store the cookie without this.
- **Delete `localStorage.getItem('menzi_token')`** and the `Authorization` set.
  Leaving a second, weaker credential in the browser invites it being used
  again.
- **The 401 handler stops navigating.** It records that the session ended;
  `RequireAuth` does the navigation so the user keeps their page.

### Session state

```ts
export type SessionStatus = 'loading' | 'authenticated' | 'anonymous';

export function useSession() {
  const query = useQuery({
    queryKey: SESSION_KEY,
    queryFn: getSession,
    retry: false,
    staleTime: 60_000,
  });

  const status: SessionStatus = query.isPending
    ? 'loading'
    : query.data?.user
      ? 'authenticated'
      : 'anonymous';

  return { status, user: query.data?.user ?? null, error: query.error };
}
```

Nothing reads storage to decide whether anyone is signed in. Deciding from
`localStorage` would put the browser back in charge of an identity it cannot
prove, which is exactly what the workspace work removed.

`loading` is a real state, not an optimisation. On a hard refresh the session
request is in flight, and rendering the login page first flashes the user out of
the app and back in.

### Routing

```
/login            public, redirects to the intended page if already signed in
/register         public, same
/forgot-password  public
/reset-password   public
/auth/callback    public, spinner, then routes on

everything else   RequireAuth
```

```tsx
export function RequireAuth({ children }: { children: ReactNode }) {
  const { status } = useSession();
  const location = useLocation();

  if (status === 'loading') return <FullPageSpinner />;
  if (status === 'anonymous') {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />;
  }
  return <>{children}</>;
}
```

`AppShell` wraps `RequireAuth` around `Layout`. `useLocation` is read inside the
component, which is already under the `BrowserRouter`; an inline check in
`App.tsx` would be outside the route context in the current tree.

`/login` and `/register` share `AuthLayout`: centred card, product name, primary
form, provider buttons under an "or" rule. Vanilla has no auth layout, so the
card is a `p-strip` with `u-width--narrow`, not a custom component.

`AuthCallbackPage` exists because the callback needs a route to land on before
the server's 302, and because a refused callback should land somewhere with an
explanation rather than a blank redirect.

### Login page

- Email and password, `type="email"`, `autocomplete="current-password"`.
- Submit disabled while pending, label "Signing in".
- A failed login shows the server's message under the form, not a toast, because
  the user needs to correct the field.
- Provider buttons render from `GET /api/v1/auth/providers`. If none is
  configured the section is not rendered. No Google logo is hard-coded; a
  `p-button` with the server's label.
- "Create an account" only when `registration === "open"`.
- "Forgot your password" below.

### Register page

- Name, email, password, confirm.
- Confirm is compared in the browser only; the server never sees it.
- The strength hint is a length counter, not a rule list, so it cannot drift from
  the server's rules.

### Account section in Settings

- Profile: name, email (read only once verified), avatar.
- Password: change form, disabled with a reason when the account has no
  `password_hash` because it only ever signed in with a provider.
- Linked identities: each row, and a "Link" button per configured provider that
  is not yet linked. Unlink is disabled for the last remaining credential.
- Active sessions: device, last seen, provider, revoke.
- Sign out of this browser, and sign out of every browser.

Linking a provider is where D4's upgrade path lands and needs the emailed
one-time code. Not in this plan.

### `dev.sh` and `setup.sh`

`MENZI_DEV_AUTH` flips to `0`. The seeded user has no password, so the first run
either registers a new user or passes
`MENZI_DEV_AUTH=1 MENZI_DEV_USER_ID=<uuid>` to skip the login page. `dev.sh`
prints the mode it started in, because a silent bypass is what makes a login page
look broken in front of a demo.

The Vite proxy is unchanged; `/api` already forwards to core-api.

---

## Test plan

No test containers exist, so the suite is unit tests plus `tower::ServiceExt::oneshot`
against routers built from in-memory stores, matching how the repo tests today.

`crates/auth`:

- `PasswordHash` verifies its own output and rejects a wrong password
- the same password hashes differently each time (salt is real)
- an explicit-params hash still verifies
- a malformed stored hash denies rather than erroring
- `SessionSecret::mint` is 32 bytes of entropy, `mz_` prefixed, URL-safe
- `digest` is stable and differs for differing input

`core-api`, middleware and routing:

- `GET /health` with no cookie is 200 (regression guard for the layer order)
- `POST /api/v1/auth/login` with no cookie is not 401
- `GET /api/v1/me` with no cookie is 401
- a valid cookie reaches the handler as `Extension<Caller>`
- a client-supplied `x-menzi-user-id` is absent from the handler's `HeaderMap`
- `POST` on a protected route with no CSRF header is 403
- `POST` with a mismatched CSRF header is 403
- `POST` with a matching CSRF header succeeds
- `GET` needs no CSRF header

`core-api`, auth service:

- register creates a user with `org_id = None`, sets a cookie, and the cookie
  verifies
- register stores a hash that `PasswordHash::verify` accepts, and never stores
  the plaintext
- register lowercases the email
- register answers 404 when the mode is `closed`
- register answers 409 for `Bob@x.com` after `bob@x.com` exists
- register rejects a short password with 400
- register rejects a password equal to the email local part
- login with the right password sets a cookie that resolves the user
- login with the wrong password is 401 and creates no session
- login against an unknown email runs Argon2 and still takes the same time
  (asserted by counting `StubHasher` calls, not by wall clock)
- a locked email is 423
- a revoked session is 401
- logout clears the cookie and revokes the row
- change password revokes other sessions and keeps the current one

`core-api`, OIDC:

- start redirects to the authorisation endpoint carrying `code_challenge` and
  `code_challenge_method=S256`
- start stores a state row with a future expiry
- callback exchanges the code and issues a session for a new user when
  auto-provision is on
- callback refuses an unverified email
- callback refuses an email that already has a password account, with the
  message from D4
- a replayed state is refused because `take` deleted zero rows
- an expired state is refused
- a state minted for one provider is refused by another provider's callback
- a wrong `nonce` is refused
- a `redirect_to` outside the allowlist falls back to `/projects`
- an unknown provider id is 404
- `GET /api/v1/auth/providers` never contains a client secret

`core-api`, orgs:

- `list_orgs` returns only the caller's org

Frontend, with `testing-library` and the existing `QueryClient` harness:

- `RequireAuth` shows a spinner while loading, redirects to `/login` when
  anonymous, and renders children when authenticated
- a signed-in user visiting `/login` is redirected away
- the login redirect returns the user to the page they asked for
- a failed login shows the server's message in the form, not a toast
- provider buttons render only when the server reports a provider
- the register button is absent when registration is closed
- `client.ts` attaches the CSRF header on `POST` and not on `GET`
- `client.ts` sends `credentials: 'include'`
- a 401 dispatches `UNAUTHORIZED_EVENT` and writes no storage
- a 401 on a deep link returns the user to that page after signing in

---

## Work packages

Each is independently committable and leaves the tree building. Listed in
dependency order.

| # | Package | Commit |
| --- | --- | --- |
| W1 | `007_auth.sql`; drop `crates/auth/src/token.rs` | `Relax user org requirement and add the auth tables` |
| W2 | `crates/auth`: `password.rs`, `secret.rs`, `record.rs`; the four store traits with in-memory impls | `Add password hashing, token minting and auth store traits` |
| W3 | `core-api/auth/{config,cookie,clock,mailer}.rs` | `Add auth configuration, cookies and the clock` |
| W4 | strip middleware; collapse the proxy's duplicate resolver; `TokenCallerResolver` reads the cookie; wired into `create_router` | `Resolve the caller once and gate protected routes` |
| W5 | Postgres implementations of the four stores | `Implement the auth stores against postgres` |
| W6 | `AuthService` + register + login + logout | `Add register, login and logout` |
| W7 | `require_csrf` + CSRF cookie | `Add csrf protection for state changing requests` |
| W8 | `GET /api/v1/me` returns the real user | `Return the real user from the me endpoint` |
| W9 | throttle | `Throttle repeated login failures` |
| W10 | `providers` endpoint | `Advertise the enabled login providers` |
| W11 | OIDC config, `OpenIdProvider`, `RustOidcProvider`, `FakeOidcProvider` | `Add oidc provider discovery` |
| W12 | `oidc_state` store + start + callback | `Add the oidc start and callback endpoints` |
| W13 | D4 identity resolution | `Resolve an oidc identity to a user` |
| W14 | password change, forgot, reset | `Add password change and reset` |
| W15 | device list and revoke | `Add session listing and revocation` |
| W16 | `list_orgs` filter, `create_org` sets `org_id`, `create_project` defaults it | `Scope org and project creation to the caller` |
| W17 | `client.ts`; delete the localStorage token | `Stop reading a token from local storage` |
| F1 | `lib/api/auth.ts` + tests | `Add an auth api client` |
| F2 | `stores/auth.ts` + `useSession` | `Track the session in the auth store` |
| F3 | `RequireAuth` + `App.tsx` routing | `Protect routes behind a session guard` |
| F4 | `AuthLayout`, `LoginPage`, `ProviderButtons` | `Add the login page` |
| F5 | `RegisterPage` | `Add the register page` |
| F6 | `ForgotPasswordPage`, `ResetPasswordPage`, `AuthCallbackPage` | `Add password reset and the callback page` |
| F7 | `AccountSection` in Settings | `Add the account section to settings` |
| F8 | empty state for a user with no org | `Offer a first organisation to a new user` |
| I1 | `dev.sh` stops defaulting `MENZI_DEV_AUTH` on; seed gains a dev password | `Stop defaulting development auth on` |
| I2 | inventory regeneration wherever routes changed | `Regenerate the backend route inventory` |
| D1 | this document, updated to implemented status | `Document authentication` |

`I2` is not optional and is not free-standing: every package that adds a route
must regenerate the inventory in the same commit or
`frontend/src/lib/backend-contract.test.ts` fails.

## Verification

Per work package:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test -p menzi-auth -p menzi-core-api
```

At the end of each of W4, W6, W7, W8:

```bash
node scripts/backend-inventory.mjs
cd frontend && npx tsc --noEmit && npx eslint . && npx vitest run
```

Full gate:

```bash
cargo fmt --all --check
cargo test --workspace
cd frontend && npm run lint && npm run build && npm run test
```

## Verified against a running stack

Run against a real Postgres with migration 007 applied, using a real Argon2id
hash and a real httpOnly cookie jar.

| Check | Result |
| --- | --- |
| `GET /health` with no cookie | 200 |
| `GET /api/v1/auth/providers` with no cookie | 200, `{"registration":"open","password":true,"oidc":[]}` |
| `GET /api/v1/me` with no cookie | 401 |
| `POST /api/v1/auth/login` with the right password | 200, sets `menzi_session` (HttpOnly) and `menzi_csrf` (readable) |
| `GET /api/v1/me` with the cookie | 200, the real user including `email`, `org_id` and `role` |
| `GET /api/v1/me` with a spoofed `x-menzi-user-id` | 200, still the cookie's user. The header is stripped, not trusted. |
| `GET /api/v1/orgs` | only the caller's org |
| `GET /api/v1/orgs/{someone-elses}` | 404 |
| `GET /api/v1/projects` | only projects the caller is a member of |
| `GET /api/v1/auth/sessions` | the caller's devices, exactly one marked current |
| `POST /api/v1/projects` with no CSRF header | 403 |
| `POST /api/v1/projects` with the CSRF header | 201, and the new project is then listed |
| A newly registered user, `GET /api/v1/orgs` | `[]` |
| A newly registered user, `GET /api/v1/projects` | `[]` |
| A newly registered user creating an org | 201, and it is then their only org |
| A newly registered user creating a project with no `org_id` | 201, in the org they just created |
| `POST /api/v1/auth/logout` | 200, and the session stops resolving |
| Wrong password | 401 |
| Password under the minimum | 400 |
| Duplicate email | 409 |
| Unknown email on `forgot` | 202, and no mail is sent |
| OIDC start with no provider configured | 404 |

`role` was `owner` for the seeded user, read from `project_members` and advisory
until authorization lands.

A second user was registered to prove the scoping: it saw no orgs and no
projects, and after creating both saw only its own. That is the `list_orgs` leak
the plan called out, now closed. Both rows were removed afterwards.

## Gotchas

- **The outer `.layer()` is outermost.** A gating layer on the outer router
  breaks `/health` and `/login`. See
  [Verified constraints](#verified-constraints).
- **`headers: HeaderMap` in middleware is a clone.** Strip through
  `request.headers_mut()`, or nothing is stripped and the identity header
  reaches the handler.
- **`argon2` 0.6 dropped the salt argument.** The 0.5 examples do not compile.
- **`OsRng` in `rand` 0.9 needs `try_fill_bytes`,** not `fill_bytes`.
- **Pin `sha2` to `0.10`.** Latest is 0.11 and would duplicate a locked version.
- **Do not edit `006_session_tokens.sql`.** It is applied; sqlx refuses to start
  on a modified migration. `007` renames and rewrites the table instead.
- **`SameSite=Strict` breaks the OIDC callback** into a redirect loop, because
  `Strict` strips the cookie on the cross-site navigation back from the provider.
- **Two ports in development.** `credentials: 'include'` is required even though
  the host matches, because the port does not.
- **`list_orgs` has no membership filter today.** Fix it in W16, in the same
  change, not later. It is the one route that becomes a real vulnerability the
  moment identity is real.
- **Do not bind the workspace service off loopback.** It trusts
  `x-menzi-user-id` and nothing else; anything that can reach port 8096 can forge
  an identity.
- **A lazy pool cannot run auth handlers.** Every store needs an in-memory
  implementation or the test suite needs a database nobody runs.
- **The route inventory test fails on any new route.** Regenerate in the same
  commit.

## What was implemented, and what is still open

Implemented as described. The remaining gaps, honestly:

1. **Registration mode in production.** `closed` is the code's default;
   `dev.sh` sets `open` because a login page nobody can reach is untestable.
   Whether production is `open` or invite-only is a product call.
2. **Register answers 409 on a taken email.** It enumerates accounts. The
   alternatives are 202 plus a "check your email" flow, which needs a real
   mailer, or always-202, which is confusing for a normal signup.
3. **Session lifetime.** 30 days with a 7 day idle timeout, as a guess.
   Shorter sessions with refresh need a refresh-token table and a rotation
   policy, which this does not cover.
4. **Linking an OIDC identity to an existing password account.** Refused, per
   D4, with a message pointing at Settings. The Settings flow and the emailed
   one-time code are not built, so that message currently sends the user
   somewhere that cannot help yet. The refusal is the safe half of the answer.
5. **Password reset is a stub.** `forgot` always answers 202 and never
   enumerates, which is correct, but `LogMailer` writes the link to the log and
   `complete_reset` only validates its arguments. No password can actually be
   reset by email yet.
6. **Authorization is not enforced.** `Role` and `Permission` exist in
   `menzi-auth` and `/api/v1/me` reports a role, but nothing gates on it. Org
   and project reads are scoped to membership, which is the floor; per-role
   permissions on writes are a separate change.
7. **OIDC is exercised only against a fake provider.** The discovery, PKCE
   challenge, state single-use, nonce check and D4 rules are all covered by
   tests, and `RustOidcProvider` compiles against RustOIDC, but no real
   provider has completed a flow. `MENZI_OIDC_PROVIDERS` with a Google entry is
   the configuration; it has not been run against Google.
8. **`invite` mode behaves as `closed`,** because the invite table does not
   exist. A configurable-but-unimplemented mode is worse than an absent one.
