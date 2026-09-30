# Workspaces

A workspace is **one container, holding one opencode, for one user working on
one project**. That unit is the reason a person opening a session and an
autonomous run triggered by something else can share the same code path: neither
has to know the other exists, because isolation already happened.

One pre-created container (`menzi-supervisor`) runs a single shared opencode.
That works until an agent is editing files for one person and something else
wants to work unattended — then there is nowhere to put the second job, and no
boundary to stop them colliding. A container per user per project is that
boundary.

Everything here is verified against real LXD and a real opencode; see
[What is verified](#what-is-verified).

## The interface

`menzi_workspace::WorkspaceManager` is the whole surface, in five operations.

| # | Operation | What it does |
|---|-----------|--------------|
| 1 | `ensure_workspace(principal, spec)` | create the container for a project, or return the one that exists |
| 2 | `connect(key)` | reach the opencode inside that workspace, verified answering |
| 3 | `open_session(key, title)` / `prompt(key, session_id, text)` | converse in it |
| 4 | `terminal(key, request)` | run a command inside that container |
| 5 | `destroy_workspace(principal, key)` | stop and delete the container, keep the record |

```rust
let key = WorkspaceKey::new(user_id, project_id);

// a person opening a session
let workspace = manager.ensure_workspace(&user_id.to_string(), spec).await?;
let session = manager.open_session(&key, Some("refactor the client")).await?;
let outcome = manager.prompt(&key, &session.id, "move the retry policy").await?;

// something else, unattended
let key = WorkspaceKey::new(service_user, project_id);
manager.ensure_workspace("autonomous", spec).await?;
let session = manager.open_session(&key, Some("nightly")).await?;
let outcome = manager.prompt(&key, &session.id, "review the open diff").await?;
```

The difference between the two callers is only who supplies the prompt and
whether they wait. Both go through the same five calls.

## Why `ensure` and not `create`

There is one workspace per user per project, and that pair is the natural key.
`ensure_workspace` is idempotent on it, which matters because two tabs, a retry,
or a second replica must not each provision a container:

- the instance name is **derived** from the pair, `wsp-{user8}-{project8}`, not
  generated. The same call always addresses the same container, and a container
  that leaks is traceable to the user and project that owns it.
- a per-key lock serialises concurrent calls, so a race provisions once.
- a partial unique index on `(project_id, user_id) WHERE deleted_at IS NULL`
  makes it a database guarantee across processes, not just an in-process one.
- a failed provision leaves the record in `requested` with the reason in
  `last_error`, so the next call retries instead of inheriting a half-built
  workspace, and the failure is readable afterwards.
- an existing container is **adopted**, not re-copied, so a retry after a crash
  or a lost response does not fail on "name already in use".

## The three seams

The manager knows the five operations and nothing about how they are carried
out, which is what keeps the two scenarios from forking.

| Trait | Owns | Implementations | Why it is a trait |
|-------|------|-----------------|-------------------|
| `WorkspaceDriver` | the container: provision, start, stop, destroy, exec, resolve address | `LxdWorkspaceDriver` over `menzi_lxd::LxdClient` | the container backend can change without touching conversation code |
| `OpencodeGateway` | the conversation: health, session, prompt, interrupt, list | `HttpOpencodeGateway` | the opencode transport is separable from the container |
| `WorkspaceStore` | the record | `PostgresWorkspaceStore`, `InMemoryWorkspaceStore` | one per unit test, one in production |

`WorkspaceStatus` mirrors the `workspaces.status` check constraint
(`requested`, `provisioning`, `ready`, `running`, `idle`, `archived`,
`deleted`) and `can_transition_to` guards the state machine, so a deleted
workspace cannot be resurrected and `requested` cannot jump straight to
`running`.

## The record

`crates/db/migrations/002_workspaces.sql` has the table;
`005_workspaces_lifecycle.sql` adds what the manager needs:

- `idx_workspaces_one_per_user_project` — the uniqueness guarantee above.
- `workspace_audit` — who provisioned and destroyed which workspace, when. The
  binary writes to it through `WorkspaceAudit`; the manager only calls
  `record`.
- `workspace_sessions` — which opencode session ran in which workspace, under
  which principal, `interactive` or `autonomous`.

`PostgresWorkspaceStore` maps the record onto that table, keeping the endpoint
and the last error in `metadata` rather than adding columns. `save` is an
`UPDATE` first and an `INSERT` second: Postgres cannot route an `ON CONFLICT`
on a partial index to a primary-key conflict, so a save that reuses the
existing row's id would fail on the pkey.

## HTTP

The service binds `0.0.0.0:8096` by default and addresses a workspace by the
pair it is keyed on, so a caller never has to look an id up first.

```
POST   /api/v1/workspaces                                    ensure
GET    /api/v1/workspaces/{user}/{project}                   get
DELETE /api/v1/workspaces/{user}/{project}                   destroy
POST   /api/v1/workspaces/{user}/{project}                   suspend
POST   /api/v1/workspaces/{user}/{project}/start             start
POST   /api/v1/workspaces/{user}/{project}/connect           connect
GET    /api/v1/workspaces/{user}/{project}/sessions          list sessions
POST   /api/v1/workspaces/{user}/{project}/sessions          open a session
POST   /api/v1/workspaces/{user}/{project}/prompt            prompt
POST   /api/v1/workspaces/{user}/{project}/interrupt         interrupt
POST   /api/v1/workspaces/{user}/{project}/terminal          run a command
GET    /api/v1/projects/{project}/workspaces                 list for a project
```

`prompt` without a `session_id` opens one first, so a caller that only wants to
send an instruction never has to make the session a separate step.

`core-api` proxies all of it, exactly as it does previews, so the frontend has
one origin. `MENZI_WORKSPACE_URL` points at the service; the default is
`http://127.0.0.1:8096`.

## Who is allowed to do this

The caller is identified by a header, and the path is a cross-check rather than
the source of truth:

| Header | Meaning |
|--------|---------|
| `x-menzi-user-id` | a person; the path's `user_id` must match it |
| `x-menzi-service` | a service principal, which must name the user it acts for |

The header name is `menzi-auth`'s existing `HEADER_USER_ID`, so the same
front-door that other services will use applies here. Missing header is
`401`; a mismatched or unparsable one is `403` or `400`; a user who is not in
`project_members` for the project is `403`. A service principal is accepted
only if it matches `MENZI_WORKSPACE_SERVICE`. **There is no token verification
yet** — the header is trusted because the service is meant to sit behind
`core-api`, and that trust boundary is the remaining piece of the auth work.

## Configuration

| Variable | Default | Meaning |
|----------|---------|---------|
| `MENZI_WORKSPACE_BIND` | `0.0.0.0:8096` | listen address |
| `MENZI_SOURCE_INSTANCE` | `mz-workspace` | the template a workspace is cloned from |
| `MENZI_OPENCODE_PORT` | `17999` | port opencode listens on inside the container |
| `MENZI_WORKSPACE_DOMAIN` | `dev.local` | suffix the published address is built from |
| `MENZI_WORKSPACE_SERVICE` | empty | the service principal allowed to act for any user |
| `MENZI_WORKSPACE_RECONCILE_SECS` | unset | interval for the reconcile loop |
| `MENZI_DATABASE_URL` | unset | with it unset the store is in memory and the authorizer is permissive |

## The template

`infra/local/workspace-image.sh` builds `mz-workspace`, the container every
workspace is copied from:

```
sudo infra/local/workspace-image.sh
```

One template for every project, with the project checked out at provision time
rather than baked in per project, because the alternative is a template set that
grows with the project count and has no expiry.

It carries the toolchain (`git`, `ripgrep`, `jq`, `build-essential`, …), the
opencode **the platform itself runs**, an unprivileged `menzi` user, and an
empty `/workspace` owned by it. opencode runs as a systemd unit on port 17999
as that user.

Pinning the opencode version is not a detail. The host's `/usr/local/bin/opencode`
is v2, which refuses unauthenticated requests on a non-loopback bind and would
need basic auth everywhere; the container the platform uses is v1.18. The script
copies the binary out of `menzi-supervisor` so a workspace speaks the same API
as everything else, warns when the versions differ, and fails the build if
opencode does not answer on the port.

The project is **not** bind-mounted from the host. See
[The write-permission problem](#the-write-permission-problem).

## What is verified

Against the live LXD on this machine and a real opencode in the container:

| Claim | How |
|---|---|
| a workspace provisions, gets the address LXD reports, and answers | `POST /api/v1/workspaces` → `ready` with `http://10.10.10.x:17999`; `opencode --version` in that container is 1.18.33 |
| `ensure` is idempotent | two calls return the same `id` and provision one container |
| a real agent run works in the container | `prompt` returned `finish_reason: "stop"` with a message id |
| `terminal` returns real output, real exit codes, and honours `cwd` | `exit_code: 0` and `uid=…` from inside the container; a failing command returns its own code |
| the agent can write in `/workspace` | a file written as the `menzi` user inside the workspace |
| destroy removes the container and keeps the record | container gone from `lxc list`, row `deleted` with `deleted_at` set |
| the record survives a restart | `GET` after a restart returns the same workspace from Postgres |
| membership is enforced | a user not in `project_members` gets `403` |
| an autonomous run is attributable | `workspace_audit` rows name the principal and the instance |

100 tests in `menzi-workspace` and 34 in `menzi-lxd` cover the logic behind
those calls.

## Not done here

- **Token verification.** The caller is a trusted header. Nothing checks a
  signature yet, so the service must not be exposed directly.
- **The frontend.** Nothing in the UI creates a workspace or routes a session
  through one. That needs the session→workspace decision below.
- **Streaming terminal.** `exec` captures output through a file and the files
  API rather than the exec websocket, so an interactive or long-running command
  is not streamed. Deliberate: it is much less code, and the five operations do
  not need a PTY.
- **Snapshots before destructive operations.** `LxdClient::snapshot_instance`
  exists and `destroy` does not use it.
- **Idle reaping.** `suspend_workspace` and `archive_workspace` exist; the
  reconcile loop runs on an interval but has no idle policy.

## Open questions these do not answer

1. **Does a workspace outlive a session?** Reaping is a product decision until
   this is settled.
2. **Who does an autonomous run belong to?** Its own service-owned workspace, or
   a person's workspace when they are not using it? Sharing means an unattended
   agent editing files a human is also editing.
3. **One opencode per workspace, or per session?** The interface supports both.
4. **Who owns cost and quota?** A container per user per project is a per-user
   bill, and `menzi-llm-gateway` already has an audit table.

## The write-permission problem

Found while building this, and it will bite every agent that writes files: a
host directory bind-mounted into an LXD container is `idmapped`, so a host
directory owned by uid 1000 appears inside as `nobody` and *no* process in the
container can write to it — not root, because `CAP_DAC_OVERRIDE` is evaluated
in the initial user namespace where the process is an unprivileged host uid.
Verified: root and uid 65534 both get `Permission denied`; `chmod 777` works and
files land on the host as uid 100000.

The template therefore ships `/workspace` **inside the container**, owned by the
agent user. If a workspace ever needs a host checkout, it needs a storage volume
with `security.shifted` (valid on a volume, **not** on a `disk` device) or
matching ownership — and that claim is still untested.
