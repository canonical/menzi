# Workspaces: plan to fix, complete and wire

Companion to `docs/workspaces.md`. That document describes the abstraction. This
one is the order of work, what is wrong with what exists, and what has to be
decided before it can be finished.

## Where this actually stands

`crates/workspace` is real: types, three traits, the manager, an HTTP router
with identity checks, a Postgres store, an audit sink, a binary, 100 tests. It
runs against real LXD and a real opencode. `core-api` proxies it, `dev.sh`
starts it, and the session proxy can route per session.

**Still not done: the frontend, and token verification.** The rest is listed
under "Not done here" and "Open questions" in `docs/workspaces.md`.

Facts established while writing this, all verified on this machine:

| Fact | How |
|---|---|
| `menzi-supervisor` is healthy, `10.10.10.251` on `menzinet` | `lxc info`, `lxc query .../state` |
| LXD reports the container address under `/1.0/instances/{name}/state` → `network.eth0.addresses[]` | same |
| `TenantExtractor` / `AuthenticatedUser` / `SessionToken` are **defined but used nowhere** | `grep` across `crates/` |
| The session proxy read **one** `opencode_url` for every request | `proxy.rs`, now `SessionRouter` with a fallback |
| The workspace service was absent from `dev.sh` | `grep`; now in `dev.sh` |
| `mz-workspace` — the template `MENZI_SOURCE_INSTANCE` defaults to — **did not exist** | built by `infra/local/workspace-image.sh` |
| A snap LXD trusts no client certificate and ships none in `~/.config/lxc` | `setup.sh` now mints and trusts one |
| `reqwest` here has both TLS backends compiled in, so `Identity::from_pem` was rejected with "incompatible TLS identity type" | `ClientBuilder::use_rustls_tls()` in `HttpLxdClient::build` |
| The opencode the platform runs is 1.18.33; the host's `/usr/local/bin/opencode` is 2.0.6, which demands a password on a non-loopback bind | `opencode --version` in `menzi-supervisor`, and a test container |

The last two mattered more than expected: without the first, no service in the
repo could talk to the LXD on this machine, previews included; without the
second, a workspace would have needed basic auth nothing in menzi sends.

## What was wrong with the code

Ordered by how much it cost to leave alone. All of these were real. All are
fixed except where noted.

### 1. The endpoint was a guess, not an address — fixed

`endpoint` returned `http://{instance}.dev.local:17999` and assumed DNS that
nothing serves. `resolve_endpoint` is now async and reads
`/1.0/instances/{name}/state`, taking the first global IPv4, and polls for up to
ten seconds — because LXD reports a container as running *before* it has an
address, and reports no address at all until it has finished booting. The DNS
form survives only as the published template, used when LXD yields nothing.

### 2. `is_running` did a full listing on every call — fixed

`LxdClient` gained `instance_exists` and `instance_state` (one `GET`, no list).
"Container up" and "opencode answering" stay separate facts: `connect` checks
the first with `instance_state` and the second with `gateway.health`, and says
which one failed.

### 3. Provisioning was not idempotent against LXD — fixed

`provision` adopts an existing container instead of copying onto a taken name.
That alone was not enough: LXD reports the copy operation as complete while the
instance is still busy, so the start was accepted and silently did nothing.
`start_and_wait` retries the start for a bounded window and only gives up when
the container has not come up at all.

### 4. The store was in-memory, so a restart lost every workspace — fixed

`PostgresWorkspaceStore` over the existing `workspaces` table, keyed
`(project_id, user_id)`, with a partial unique index in
`005_workspaces_lifecycle.sql`. `save` is an `UPDATE` then an `INSERT`, not an
`ON CONFLICT` on the partial index: Postgres will not route a primary-key
conflict to a partial-index arbiter, so re-saving a row's own id fails on the
pkey. Found by destroying a real workspace.

### 5. Nothing authenticated the caller — partly fixed

The router took `user_id` from the path and checked nothing: `ensure_workspace`
for user A, `destroy_workspace` for user B, both worked. Now:

1. the caller comes from `x-menzi-user-id` or `x-menzi-service`, and the path's
   `user_id` is a cross-check;
2. membership is checked against `project_members` before provisioning;
3. a service principal exists for autonomous runs and must name its user;
4. every lifecycle step is written to `workspace_audit`.

**Not fixed: the header is not verified against a token.** The service is meant
to sit behind `core-api` and that boundary is currently assumed, so it must not
be exposed directly. That is the rest of phase 4 and the next thing to do.

### 6. `terminal` had no timeout and no output — fixed, except streaming

`timeout_secs` is applied at the manager, and a timeout returns
`timed_out: true` rather than hanging. Output was the bigger surprise: `exec`
posted to the instance URL, which is LXD's *create* endpoint, so it answered
`409 Name already in use` and had never worked. The real endpoint is
`/1.0/instances/{name}/exec`, it is an async operation, and it returns no
output unless a websocket is attached. `exec_in` therefore redirects to a file,
reads it back through the files API, and writes the exit code to a second file.
Streaming a PTY is still not done; it needs the websocket and is bigger than the
five operations require.

### 7. Nothing was tested against real LXD or real opencode — fixed

The gateway has tests over recorded response bodies, including the error shape
opencode actually returns:

```json
{"name":"NotFoundError","data":{"message":"Session not found: ses_nope"}}
```

and the service was then run against the live LXD and a real opencode in a real
container. That found four bugs no unit test would have: the create endpoint
used for exec, the async operation handling, the running-before-addressed race,
and the `ON CONFLICT` arbiter problem.

### 8. Smaller things — fixed

- `WorkspaceId` is in `menzi_common::ids`.
- A failed provision records the reason in `last_error` and stays retryable.
- `main.rs` reads `MENZI_SOURCE_INSTANCE` once and hands it to the manager,
  which owns it.
- The `security.shifted` claim is now stated as untested rather than as a fix.

## The ordering, and why

Phases 1–3 were correctness on the existing interface. Phase 4 is security and
had to land before anything was exposed. Phase 5 makes the interface real for
the interactive scenario. Phase 6 is the proxy, the only reason the interactive
scenario needs phase 5 at all. Phases 0–3, phase 4 apart from token
verification, and the core-api half of 5 and 6 are done.

### Phase 0 — the template instance — done

`mz-workspace` is built by `infra/local/workspace-image.sh`: one template for
every project, project cloned in at provision time, `/workspace` inside the
container owned by an unprivileged `menzi` user, opencode as a systemd unit on
17999, and the opencode binary copied out of the platform container so a
workspace speaks the same API as everything else. The build fails if opencode
does not answer on the port, which is what stops a version that demands a
password from being baked in silently.

### Phase 1 — address and lifecycle correctness — done

`instance_state`, `instance_exists`, `get_instance`, async `resolve_endpoint`
with a polled address wait, adopting `provision`, `start_and_wait`, a `destroy`
that survives a stop that has not landed yet, and a recorded failure reason.

### Phase 2 — persistence — done

`WorkspaceId`, migration `005_workspaces_lifecycle.sql` with the partial unique
index plus the audit and session tables, `PostgresWorkspaceStore`, and the
binary constructing it.

### Phase 3 — the opencode wire format — done

Gateway tests over recorded bodies, a timeout on every gateway call, and
`interrupt` on the trait.

### Phase 4 — identity and authorisation — mostly done

Everything except token verification, as above.

### Phase 5 — the interactive scenario — half done

`core-api` proxies every workspace route. **Not done: the frontend.** Session
creation in the UI still goes to the single shared opencode, because "which
opencode serves this session" has no durable answer until open question 1 is
settled. The `workspace_sessions` table exists for that answer.

### Phase 6 — per-session routing in the proxy — half done

`SessionRouter` is a trait in the session proxy, with
`InMemorySessionRouter` behind it, resolved from the session id in the path
(`/session/{id}/…` and the legacy `/api/session/{id}/…`) and overridable with
`x-menzi-workspace-endpoint`. `POST /api/opencode/bind` binds a session. **The
fallback is the important part**: an unbound session goes to the single
registered target exactly as before, so nothing that works today breaks.
**Not done: a `SessionRouter` that talks to the workspace service.** The proxy
has no idea where a workspace endpoint comes from, and that is the open
question, not an implementation detail.

### Phase 7 — lifecycle and leakage — half done

`reconcile` adopts a `provisioning` row whose container exists, reports one
that does not, and flags a live workspace whose container is gone. `reap`
destroys containers stuck in `provisioning`. The binary runs `reconcile` on
`MENZI_WORKSPACE_RECONCILE_SECS`. **Not done: the idle policy** (idle N
minutes → archive), which needs open question 1, and **snapshots before
destructive operations**, which is small and could be done now.

## Open questions

1. **Does a workspace outlive a session?** If yes, reaping is a real product
   decision. If a workspace is just "while a session is open", the lifecycle is
   much simpler and the answer to "which opencode serves this session" is "the
   one the session is bound to, for the session's life".
2. **Autonomous runs and ownership.** Which container does a nightly run on
   project X for user A get? Sharing A's container means an unattended agent
   editing files a human is also editing.
3. **Is one opencode per workspace enough, or one per session?** The interface
   supports both; multi-session-per-container needs opencode to support it and
   probably needs locking.
4. **Who owns cost and quota?** A container per user per project is a per-user
   resource bill.

## Risks worth stating plainly

- **The proxy is the hard part, and it is half done.** The resolver falls back,
  so the risk is bounded — but a `SessionRouter` that cannot find a workspace
  will silently send traffic to the shared opencode, which is the bug the whole
  design exists to prevent. It needs to fail loudly once the fallback stops
  being the right default.
- **Two opencode APIs in play.** v2 (`/session`, `/session/{id}/message`,
  `summary.diffs`) and the legacy `/api/session/...` envelope the frontend still
  uses. The gateway speaks both for listing and v2 for the rest, and the
  template is pinned to the version the platform runs. Moving that pin moves the
  API the product depends on.
- **The template goes stale.** A template with opencode pinned in it is a
  version to maintain, and phase 0's answer only makes that cheaper, not free.
- **Disk.** LXD is on the same volume as the repo. One workspace is ~1.5G; the
  pool deserves its own volume.
