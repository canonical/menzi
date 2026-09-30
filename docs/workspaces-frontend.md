# Frontend plan: workspaces

How the UI starts using the workspace service, what the user sees while it
happens, and what has to be true on the backend before any of it is correct.

Companion to `docs/workspaces.md` (the abstraction) and
`docs/workspaces-plan.md` (the backend state of play).

## What changes for the user, in one sentence

Opening a project may now start a ~35 second, 1.5 GB container build, and every
session in that project happens inside it instead of a shared agent.

That single sentence is the whole design problem. Everything below is either
making that invisible when it can be, honest when it can't, or removing the
ways it breaks.

## Four things in the current code that constrain the design

Found while reading, not assumed:

1. **The session list is global, not per project.** `CodePage` calls
   `listSessions()` with no project filter (`opencode.ts:19`). The project id in
   the URL is decoration for the list. With one opencode per workspace there is
   no longer a single list to ask for, so this has to change — the list becomes
   "the sessions in *this* workspace".

2. **The event stream is a single global `EventSource`.** `useSessionStream`
   opens `/api/oc/event` (`events.ts:1`) and filters by session id client-side
   (`useSessionStream.ts:71`). The proxy serves that stream from the one
   registered `opencode_url`. **Left alone, chat in a workspace would show
   nothing**, because the events are arriving from a different opencode. This is
   a correctness blocker, not polish.

3. **Nothing authenticates anything.** The frontend keeps a bearer token in
   `localStorage` that nothing verifies; `core-api` has no auth middleware; the
   workspace service trusts an `x-menzi-user-id` header that nobody sets. The
   plan below does not fix this, but it must not make it worse, and it must not
   bake a second, worse version of it into the browser.

4. **The in-memory session router forgets on restart.**
   `InMemorySessionRouter` lives in the proxy process and nothing in production
   calls `POST /api/opencode/bind`. Bind a session today and a proxy restart
   silently routes it back to the shared opencode — the exact failure workspaces
   exist to prevent. The `workspace_sessions` table exists for this and is
   currently never written to.

## Decisions

### D1 — A workspace belongs to (user, project) and outlives sessions

Many sessions per workspace, one workspace per user per project. This is open
question 1 in the plan doc, and the frontend needs an answer, so here it is.

**Why:** the container holds the working tree, the toolchain and the agent's
state. A workspace per session means re-cloning and re-installing on every
conversation, which is the cost we are trying to amortise in the first place. It
also means the review panel reads the changes out of a container that still
exists, and a session you come back to tomorrow still has its files.

**Cost:** idle containers linger, so reaping becomes a policy question rather
than a correctness one. `suspend_workspace`, `archive_workspace` and `reap`
already exist; only the trigger is missing (D2's "Not now" and an idle window).

**Reversible?** Barely, and it would be expensive. Worth being sure.

### D2 — Provision on view, not on "New session"

Entering a project's **Code** section starts provisioning in the background if
there is no workspace. The user does not have to ask for it, and by the time
they have read the page and decided what to type it is usually ready.

**Why not on click:** a 35 second spinner on the primary action, with no
guarantee it will ever finish, is the worst thing this feature could do. **Why
not at project creation:** it spends 1.5 GB on projects the user may never open
a chat in, and it makes creating a project slow.

**Why only the Code section:** Design, Project and Previews don't need a
container. Provisioning on every project page visit would create containers for
browsing.

**The cost is real, so it gets a control.** The status line includes "Not now",
which destroys the container and stops the prefetch. A user who is only reading
a project should not silently accumulate containers, and telling them it is
happening is the minimum.

### D3 — The session list is the workspace's; the tabs stay

Sessions are listed from the workspace's own opencode, newest first, and shown
as the existing tabs. Tab labels improve from `ses_f111c7… · 29 Sep` to the
session's first prompt.

**Why not a session rail now:** the Code page is already a two-column split
(chat | review), and the layout work earlier established that a third column
breaks below `64.75rem`. A rail is a later stage, once sessions actually
accumulate and we can see whether tabs have become unusable. Don't build it
speculatively.

**Why the labels matter:** with a long-lived workspace the list grows without
bound, and `ses_f111c7… · 29 Sep 2026` does not let anyone find last Tuesday's
conversation. This is a small change with a large effect on findability.

### D4 — Identity: the browser never asserts who it is

The workspace service reads `x-menzi-user-id`. `core-api` **sets** that header
from the verified caller and **strips** anything the client sent, exactly as it
already strips hop-by-hop headers. The frontend therefore sends no identity
header at all and gets whatever `core-api` decides.

**Why:** if the browser sends its own user id, then any user can be any user by
editing one header, and the membership check inside the workspace service is
decorative. Putting the trust boundary in `core-api` — the one hop the browser
cannot skip — is the whole point of having a gateway.

**Consequence:** until `core-api` has a real token check, this is a dev-mode
passthrough and the workspace service must stay on the private network. That is
already true, and D4 keeps it true by construction rather than by accident.

## User flows

### A — first visit, no workspace

`GET /workspaces/:user/:project` → `404`.

1. The page renders immediately. Nothing blocks on the workspace.
2. The status line appears where the session list will be: *"Setting up your
   workspace — this takes about 30 seconds."*
3. A `POST /workspaces` goes out in the background (prefetch, D2).
4. The session list is empty, and its empty state now names the workspace
   rather than pretending sessions are one click away.
5. **New session is disabled, with the reason next to it**, not a button that
   spins forever. Disabled-with-a-reason teaches the model in one glance;
   a spinner on a 35 second action teaches nothing and looks broken.
6. Poll `GET` every 3 s while the status is `requested` or `provisioning`. Stop
   on `ready`, or on a failure.
7. On `ready` the status line collapses to a quiet "Workspace ready" chip,
   the button enables, and if the user clicks immediately the call is fast.

### B — returning visit

`GET` → `ready`. Sessions list from the workspace. **The UI is identical to
today.** No new chrome, no new step, no new thinking required. This is the case
that matters most: the feature must be invisible when it is working.

### C — setup failed

`GET` → `requested` with `last_error`.

*"Workspace setup failed"* in a negative chip, the reason beneath it, and a
**Retry** button. Retry is `POST /workspaces` again, which is idempotent and
safe — it adopts an existing container or copies a new one. The user never has
to guess whether retrying is right.

The reason is the backend's `last_error` verbatim. "the image `mz-workspace`
could not be copied" is actionable; "something went wrong" is not.

### D — the container disappeared

Someone deleted it in LXD, or a reaper got it. Today `reconcile` reports the
container as reaped but **leaves the record saying `ready`** — so `GET` lies to
the UI. That is a backend defect this integration exposes, and it has to be
fixed (stage 0): reconcile must mark such a workspace `requested` so the truth
comes from the server and the UI simply shows flow A again.

The UI still defends itself: a session call that 404s while the workspace says
`ready` triggers a workspace refetch rather than an error toast.

### E — not a member

`GET` → `403`. Show *"You don't have access to a workspace for this project."*
No prefetch, no retry, no provisioning. The project is browsable; the workspace
is not. This is the state that most needs to be *quiet* rather than alarming —
it is a permission, not a fault.

### F — paused

`GET` → `idle`. *"Your workspace is paused"* with **Resume**. Session creation
still works (the backend starts the container on demand), so the button is
enabled; the hint just explains the first click will be slower.

### G — destroying

On the project's **Project** section, a Workspace card: status chip, instance
name, created date, and actions **Pause / Resume / Delete**. Delete uses
`ConfirmationModal`, matching `PreviewsPage`, and says what is lost: the
container, the working tree, and anything not committed. Not a toast-only
undo — a container cannot be undone, and pretending otherwise would be a lie.

### Robustness, by case

| Case | Behaviour |
|---|---|
| Two tabs on the same project | Both prefetch; the backend is idempotent and per-key locked, so one container. Both polls converge. No UI coordination needed. |
| Refresh mid-provision | The poll resumes from server state. The UI keeps **no** provisioning state in component state — every state shown is read from `GET`. |
| Provisioning runs long | Show elapsed time. After 90 s, swap the spinner for "Still working" plus a manual check, rather than an indefinite promise. |
| A second browser tab creates a session | Both see it on the next list poll (10 s while running). |
| Network drops | `DataState` already renders errors with a retry. The status line degrades to "Can't reach the workspace service" and does not pretend the workspace is fine. |
| Tab closes while provisioning | The container finishes provisioning server-side and the record says `ready` when the user returns. Nothing is wasted, nothing is lost. |

## Backend prerequisites

All four are done, because the frontend cannot be correct without them.

| # | Change | Why it blocks | Where |
|---|---|---|---|
| B1 | `core-api` resolves the caller and **replaces** any `x-menzi-user-id` or `x-menzi-service` a client sent | D4. Without it the membership check is decorative. | `crates/core-api/src/identity.rs`, `modules/workspaces_proxy.rs` |
| B1 | `GET /api/v1/me`, so the browser learns who it is rather than claiming it | The auth store is empty until something resolves it, and without a user the workspace query never enables. | `crates/core-api/src/whoami.rs`, `frontend/src/stores/useCurrentUser.ts` |
| B2 | The event stream resolves through `SessionRouter` from `?session=` | Constraint 2. Without it, streamed chat is silently empty in a workspace. | `crates/session-proxy/src/proxy.rs` |
| B3 | Bindings persist in `workspace_sessions`, and the proxy's `WorkspaceRouter` reads them | Constraint 4. Without it, every session breaks on a proxy restart. | `crates/workspace/src/registry.rs`, `crates/session-proxy/src/router.rs` |
| B4 | `reconcile` marks a vanished container `requested` with the reason | Flow D. Without it, `GET` lies and the UI shows a dead workspace as ready. | `crates/workspace/src/manager.rs` |

B2 and B3 are the ones that make "robust" true rather than aspirational.

### Identity, honestly

`TokenCallerResolver` verifies a bearer token against a new `session_tokens`
table. `DevCallerResolver` and `ServiceCallerResolver` exist for development and
for the proxy's own session lookups.

**Token issuance is not implemented** — there is no login endpoint — so
`dev.sh` sets `MENZI_DEV_USER_ID` to the first seeded user and that is what the
gateway forwards. The boundary is still real in the sense that matters: the
browser's header is *discarded* and the gateway's own decision is what reaches
the workspace service, which
`a_client_supplied_identity_is_replaced_not_forwarded` pins down. A real login
flow replaces the resolver, not the header handling.

A service principal is what the session proxy uses, because it holds no user for
a session it is asked to route. `GET /api/v1/sessions/{id}/workspace` accepts one;
a user may only look up their own.

## Stages

Each ships on its own and each is testable on its own.

**Stage 0 — backend prerequisites (B1–B4).** Done.

**Stage 1 — the client, with no behaviour change.** Done. `lib/api/workspaces.ts`
mirroring `previews.ts`, the `Workspace` type, `apiPaths.workspaces` and
`queryKeys.workspaces` keyed on `(userId, projectId)`. 16 tests over request
shape and error mapping.

**Stage 2 — read-only surface.** Done. The Workspace card on the Project page:
status, instance, created, Pause / Resume / Delete with a confirmation. 12 tests.

**Stage 3 — provisioning UX.** Done, except **"Not now"**. The status line, the
prefetch, the poll, the failure and retry states, the blocked-reason line, the
new empty-state copy. 16 tests over the phase machine and the session gate, 12
over the status line, 12 over the Code page's use of it.

**Stage 4 — session binding.** Done. Sessions listed and created through the
workspace, the per-session event stream, and the proxy routing verified against a
real container.

**Stage 5 — polish.** Not done. Session titles in the tabs **are** done, because
findability was a stated problem. Still open: reduced motion, keyboard and focus
behaviour for the status line, and a session rail if tabs turn out not to scale.

## Files

New:

```
lib/api/workspaces.ts
lib/api/whoami.ts
lib/types.ts                    + Workspace, WorkspaceSession, PromptOutcome
stores/useCurrentUser.ts         who the browser is, from the gateway
features/workspaces/useWorkspace.ts        phase, gate, prefetch, poll
features/workspaces/reasons.ts             why a session is blocked
features/workspaces/WorkspaceStatus.tsx    the status line
features/workspaces/WorkspaceCard.tsx      the Project page card
```

Changed:

```
lib/routes.ts                   + apiPaths.workspaces, queryKeys.workspaces
lib/api/events.ts               + eventStreamFor(session)
lib/chat/useSessionStream.ts    session-scoped event stream url
features/sessions/CodePage.tsx  workspace gate, prefetch, status line, tab titles
features/projects/ProjectPage.tsx + the workspace card
stores/auth.ts                  + setUser
App.tsx                         resolves the current user once
strings/catalogue.ts            all new copy
styles/index.scss               the status line
```

## Copy

Specific, because vague copy is why provisioning feels like a hang.

```ts
workspace: {
  ready: 'Workspace ready',
  settingUp: 'Setting up your workspace — this takes about 30 seconds.',
  settingUpElapsed: 'Still setting up ({elapsed}). This can take a couple of minutes.',
  slowHint: 'You can keep browsing while this finishes.',
  pauseHint: 'Your workspace is paused. Starting a session will wake it.',
  failedTitle: 'Workspace setup failed',
  retry: 'Try again',
  notAMemberTitle: 'No workspace for you in this project',
  notAMemberBody: 'Ask a project maintainer for access if you need an agent to work on it.',
  unreachable: "Can't reach the workspace service.",
  reasons: {
    'setting-up': 'Your workspace is still being set up.',
    failed: 'The workspace could not be set up.',
    paused: 'Your workspace is paused.',
    'not-permitted': 'You do not have access to a workspace in this project.',
    unreachable: "Can't reach the workspace service.",
    absent: 'There is no workspace for this project yet.',
    loading: 'Checking your workspace.',
  },
  card: {
    title: 'Workspace',
    none: 'No workspace yet',
    noneBody: 'One is set up when you open a session in this project.',
    columns: { status: 'Status', instance: 'Container', created: 'Created', actions: 'Actions' },
    actions: { pause: 'Pause', resume: 'Resume', delete: 'Delete' },
    statuses: { requested: 'Setting up', provisioning: 'Setting up', ready: 'Ready', running: 'In use', idle: 'Paused', archived: 'Archived', deleted: 'Deleted' },
    deleteTitle: 'Delete this workspace?',
    deleteBody: 'The container and every change in it are removed. This cannot be undone.',
    deleteConfirm: 'Delete workspace',
    deleteCancel: 'Keep it',
    toasts: { paused: 'The workspace is paused.', resumed: 'The workspace is running.', deleted: 'The workspace was deleted.', failed: 'That did not work.' },
  },
  sessions: {
    emptyTitle: 'No sessions yet',
    emptyBody: 'This project has its own agent, so sessions here do not share state with anything else.',
  },
}
```

`notNow` is gone: it is the one piece of copy for a control that was not built.

## What I am deliberately not doing

- **A `/workspaces/:id` page.** The workspace is a property of a project the
  user is already on, with no handle users have a reason to hold. A page would
  add a concept and a route for nothing. A detail page becomes justified if and
  when terminals or logs ship.
- **A session rail.** D3. Three columns break the layout below `64.75rem`, and
  tabs have not been shown to fail yet.
- **"Not now" on the status line.** D2 spends real resources silently, and this
  was the control that let a user decline it. It needs a destroy-and-suppress
  decision that has no home yet, so the status line tells the truth and the card
  on the Project page is where deletion lives. The copy is written when the
  control is.
- **A login screen.** There is none in the product, and the gateway's dev
  resolver is what stands in for it. See the identity note above.
- **Autonomous runs in the UI.** Open question 2 (whose container does an
  unattended run get) has no answer, and inventing a screen for it would
  hard-code a guess.

## Verified against the running stack

Every one of these was run against real LXD, a real container and a real
opencode, through `core-api` exactly as the browser reaches them.

| Claim | Result |
|---|---|
| entering a project reads the workspace, then prefetches one | `GET` → 404, `POST` → `ready` with a real container address |
| polling reports the truth | `ready` → container deleted behind it → `requested`, "the container is gone" → re-provisions on the next ensure |
| a session is created in the workspace and listed there | `[{"id":"ses_…","title":"final"}]` |
| a real agent run inside the container | `finish_reason: "stop"` |
| the proxy sends that session to the workspace's opencode | proxied `GET /session/{id}/message` → 200, while the shared opencode answers `NotFoundError` for the same id |
| an unbound session still falls back to the registered target | an answer from the shared opencode, not an error |
| the event stream is narrowed to its session | `?session=` reaches the workspace's opencode |
| a non-member is refused | 403, phrased as a permission rather than a fault |
| a browser cannot assert an identity | the request's own `x-menzi-user-id` is discarded and the gateway's decision is what arrives |
| pause, resume, delete | `running → idle → ready → deleted`, container removed |
| a terminal in the workspace | `opencode --version` from inside the container |

## The one call I would like ruled on

**How long before an idle workspace is archived.** It bounds the cost D2
spends, and it is a product decision, not a technical one. My default would be
24 hours with a pause before the archive, because a working day is the obvious
unit and a user who comes back after a weekend should not have lost their
container — but if workspaces are cheap enough on the target hardware, "never"
is a defensible answer and simpler.
