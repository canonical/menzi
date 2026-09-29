# Web UI Gap Plan

| | |
|---|---|
| **Status** | In progress — §7, B1, B2 (backend + first slice), §6.1 and §6.2 landed |
| **Date** | 28-SEP-26 |
| **Scope** | The gap between what `docs/frontend-implementation-plan.md` specifies and what the frontend can honestly build today |
| **Depends on** | `docs/frontend-implementation-plan.md` (phases P0–P7) |
| **Supersedes** | Nothing. Corrects the backend inventory in the implementation plan, which is wrong — see §1 |

## Progress

| Item | State |
|---|---|
| §7.1 inventory generator | Done — `scripts/backend-inventory.mjs`, checked-in `docs/backend-inventory.md` |
| §7.2 contract gate | Done — `frontend/src/lib/backend-contract.test.ts` |
| §7.3 runbook | Done — status-code table in `docs/runbooks/local-dev.md` |
| §7.4 probe assertions | Done — `dev.sh` asserts expected status per service |
| B1 session-proxy + llm-gateway in dev | Done — `:8082` and `:8083`, both probed |
| B2 SSE edge | Backend done — `?after=&follow=true`, cursor, live broadcast. Frontend slice: backlog + live rows in the workspace. Typed parts (P4.5) still pending the §8.2 envelope decision |
| §6.1 bodyless orchestrator probe | Done — `GET /api/env/health` |
| §6.2 spec discovery | Done — `GET /api/env/specs`, picker replaces the free-text field |
| §6.3 spec persistence | Not started |
| §6.4 log stub | Not started |
| §6.5 previews entry point | Not started — still needs B3 |
| §6.6 axe exclusions | Open — upstream defects to file |
| B3–B10 | Not started |

---

## Table of contents

1. [Why this document exists](#1-why-this-document-exists)
2. [Verified backend inventory](#2-verified-backend-inventory)
3. [The two levers that matter](#3-the-two-levers-that-matter)
4. [Backend work, ordered by leverage](#4-backend-work-ordered-by-leverage)
5. [Frontend work, in dependency order](#5-frontend-work-in-dependency-order)
6. [Deviations to resolve in the work already shipped](#6-deviations-to-resolve-in-the-work-already-shipped)
7. [Process fix: make the inventory impossible to get wrong](#7-process-fix-make-the-inventory-impossible-to-get-wrong)
8. [Decisions needed before starting](#8-decisions-needed-before-starting)
9. [Definition of done](#9-definition-of-done)

---

## 1. Why this document exists

While implementing P4/P5 I reported that the backend was essentially empty and marked those phases blocked. **That was wrong**, and the implementation plan still carries the error.

The mistake was reading non-2xx responses as absence. `dev.sh` reported:

```
orchestrator up (415) at http://127.0.0.1:8081/api/env/status
previews up (405) at http://127.0.0.1:8095/api/v1/previews
```

`415` and `405` mean *the route exists* and the request was malformed — wrong media type, wrong method. They are evidence of a working endpoint, not a missing one. I never opened the Rust and inferred everything from status codes. The orchestrator turned out to have a complete environment API, and the P5.2 Env tab is now built against it with real data.

Two consequences:

- Phases previously marked "blocked" in the implementation plan need re-evaluating against §2 below, not against status codes.
- The inventory has to become a checked artifact, or this will recur. That is §7, and it is the most important item here.

---

## 2. Verified backend inventory

Read from the Rust sources on 28-SEP-26 and confirmed against the running dev stack. This is the baseline everything below depends on.

### 2.1 Services with real routes

| Service | Port | In `dev.sh` | Routes |
|---|---|---|---|
| `core-api` | 8080 | yes | `GET /health`, `GET/POST /api/v1/orgs`, `GET /api/v1/orgs/{id}`, `GET /api/v1/projects/{id}/previews` (proxy), `POST /api/v1/previews`, `GET/DELETE /api/v1/previews/{id}`, `POST /api/v1/previews/{id}/reset`, `POST /api/v1/previews/{id}/restart` |
| `previews` | 8095 | yes | `GET /api/v1/projects/{project_id}/previews`, `POST /api/v1/previews`, `POST /api/v1/previews/{id}`, `…/reset`, `…/restart` |
| `orchestrator` | 8081 | yes | `POST /api/env/specs`, `POST /api/env/launch`, `POST /api/env/relaunch`, `GET /api/env/status`, `POST /api/env/exec`, `POST /api/env/logs` |
| `session-proxy` | — | **no** | `POST /api/opencode/register`, `POST /api/tunnel/register`, `GET /api/tunnel/{id}/status`, `POST /api/tunnel/{id}/fanout`, `GET /api/tunnel/{id}/transcript`, `ANY /api/{*rest}` (allowlisted forward to opencode) |
| `llm-gateway` | — | **no** | `POST /v1/chat/completions`, `POST /v1/embeddings`, `GET /v1/models` |

All five respond to `EnvResponse`/`ApiError` style JSON envelopes and reject unknown routes. `core-api` authenticates nothing.

### 2.2 Services that start an HTTP server with **zero routes**

These are the highest-leverage gaps in the whole backend. Each has `create_router() -> axum::Router::new()` and nothing else — the binary binds a port and serves an empty router.

| Crate | Domain logic present | Routes |
|---|---|---|
| `design-service` | `types.rs`, `impact.rs`, `mcp_tools.rs` | none |
| `notifications` | `types.rs`, `dispatch.rs`, `preferences.rs` | none |
| `git-integration` | `types.rs`, `tokens.rs`, `webhooks.rs` | none |

`supervisor` has no HTTP router at all; it is the per-session runtime (opencode, MCP, safety net, config render).

### 2.3 Auth

`menzi-auth` defines `TenantExtractor` in `crates/auth/src/tenant.rs`. It is **referenced nowhere else**, and no router in any crate installs middleware. There is no login endpoint, no token issuance, and no tenant scoping. The frontend reads `localStorage.menzi_token` and sends a bearer header that nothing checks.

### 2.4 Transcript streaming

`transcript_events` in `crates/session-proxy/src/proxy.rs` takes only `Path<SessionId>` — no `Query` extractor, no `after` cursor, no `follow` flag. It returns the entire event array as one JSON blob.

`fanout_subscribe` registers a viewer server-side and returns `{"status":"subscribed"}` immediately. **There is no channel from server to client.** The archiver, fanout and viewer registry all exist in `transcript.rs`/`fanout.rs`, and `crates/events` provides NATS JetStream (`bus.rs`, `streams.rs`, `nats_kv.rs`). The missing piece is the HTTP streaming edge, not the machinery beneath it.

### 2.5 Database

23 tables exist across four migrations. This is the key finding for P6: **the data model is largely built.**

| Tables | Plan phase they unblock |
|---|---|
| `design_clauses`, `design_amendments`, `design_revisions`, `design_waivers`, `design_baselines`, `design_conformance_reports`, `design_discussions` | P6.2, P6.3, P6.4 |
| `git_repos`, `git_prs` | P5.1 (Changes, Create PR) |
| `notifications`, `notification_preferences`, `web_push_subscriptions` | P6.5, settings notifications tab |
| `projects`, `project_members`, `templates`, `workspaces`, `environments` | P3.1, P4.1 |
| `llm_credentials`, `llm_usage` | P6.7, P6.8 |
| `previews` | P6.1 (done) |
| `orgs`, `users`, `audit_logs`, `jobs` | P6.6 |

There is **no `sessions` table.** Sessions are runtime state held by the supervisor and session-proxy, so the P4 workspace has nothing to read for session metadata. Decide whether that is intended (§8).

### 2.6 Frontend status

P0–P3 and P7 are complete. P4.1 and P5.2 are complete. Build, lint and 41 tests pass. The remaining P3/P4/P5/P6 items are listed in §5.

---

## 3. The two levers that matter

Almost all remaining frontend work is blocked on one of two things. Everything else is detail.

**Lever A — expose the tables that already exist.** `design-service`, `notifications` and `git-integration` are three empty routers sitting on top of implemented domain logic and a populated schema. Adding read routes is mechanical work, not greenfield design. It unblocks P6.2, P6.3, P6.4, P5.1 and P6.5 — five phases and roughly half of P6's acceptance criteria.

**Lever B — give `session-proxy` an HTTP streaming edge, and run it.** The event log, fanout and NATS are all there; only the push path to the browser is missing, and the service is not in `dev.sh`. This unblocks P4.4 through P4.11, which is the largest single block of unimplemented UI and the design doc's main screen.

Sequencing note: Lever A is cheaper and de-risks the release independently. Lever B is the one that makes the product coherent. Doing A first is defensible; doing B first makes more of the existing backend visible.

---

## 4. Backend work, ordered by leverage

### B1 — Put `session-proxy` and `llm-gateway` in `dev.sh`
*Unblocks: all of P4. Smallest change with the largest effect.*

`dev.sh` starts `core-api`, `orchestrator` and `previews` only. The session workspace currently has no backend to talk to in dev, which is the actual reason P4.4+ is unbuildable. Add both binaries to the build list and the process list, and add a health probe for each. Note that a generic `wait_http` cannot probe `/api/env/status` (it is a GET that requires a JSON body and answers `415` to a bodyless probe) — see §6.5.

### B2 — Add the SSE edge to `session-proxy`
*Unblocks: P4.4, and the live half of P4.5, P4.6, P4.7, P4.11, P6.5.*

- `GET /api/tunnel/{id}/transcript?after=<seq>&follow=true` returning `text/event-stream`, backed by the existing archiver and fanout.
- Add an `after` cursor to the existing non-follow path so the initial load is paginated rather than a full blob.
- Contract decision needed on the event envelope (§8): the frontend reducer needs a stable, typed part shape, not whatever the archiver happens to store.

### B3 — Sessions API in `core-api`
*Unblocks: P4.2 threads, P4.3 turns, P3.1 projects.*

Nothing creates or lists a session, and there is no session table. Minimum viable: create, list by project, get by id, and a thread/turn read model. Without this the P4 workspace has a hardcoded-looking UUID route parameter, which is what it has now.

### B4 — Expose `design-service`
*Unblocks: P6.2, P6.3, P6.4.*

Read routes over the seven existing design tables, plus amendment transitions (the §10.1 lifecycle). The clauses catalogue with scope/area/status/level filters and the conformance violation counts are the two acceptance-critical screens.

### B5 — Expose `git-integration`
*Unblocks: P5.1.*

Per-turn diff, VCS status and diff against the review base, and Create PR against `git_repos`/`git_prs`.

### B6 — Expose `notifications`
*Unblocks: P6.5 inbox, and the settings notifications tab.*

Needs to be session-aware, because the inbox is defined as "sessions blocked on permission or form, idle with changes, approvals waiting, previews commented" — that is a join across notifications, sessions and previews, not a plain list query.

### B7 — Auth and tenancy
*Unblocks: P3.10 login, and makes every other screen trustworthy.*

Wire the existing `TenantExtractor` as middleware on `core-api` and `session-proxy`, add token issuance, scope every query by tenant, and add the org/user membership checks that `project_members` already models. Until this lands, no screen can be said to enforce anything, and the frontend's bearer token is decorative.

### B8 — Permissions service
*Unblocks: P4.7 inline permission cards, P5.4 per-exposure permission check, P6.6 roles.*

Required by design doc §7.6, which is explicit that shells and TCP forwards are code execution. This is a security control, not a convenience.

### B9 — Environment teardown, log streaming, spec persistence
*Unblocks: the remainder of P5.2.*

Three separate gaps in one crate:
- No teardown route. P5.2 asks for it; the orchestrator has stop/start but no teardown.
- `logs_handler` returns a hardcoded `{"lines": [], "truncated": false}`. The log viewer built in P5.2 is wired correctly and will always be empty until this returns real lines.
- Specs live in `HashMap` in process memory, so every orchestrator restart silently drops them. Needs persistence.

### B10 — Cost and audit reads
*Unblocks: P6.7, P6.8.*

Read routes over `llm_credentials` and `llm_usage`. Credential writes must be write-only per §12.7 — never return key material on read. `llm-gateway` already exposes `/v1/models`.

---

## 5. Frontend work, in dependency order

Each item names the backend work it waits on. Nothing here should start before its dependency lands, because each one is a screen that would otherwise render an empty state.

| Order | Phase | Item | Waits on |
|---|---|---|---|
| 1 | P3.1 | Projects list from `projects`/`project_members` | B3 |
| 2 | P3.2 | Environment page, reusing the P5.2 env client | B9 |
| 3 | P4.2 | Threads column, `ListTree` with forks | B3 |
| 4 | P4.3 | Turns column with "Revert files" and "Restore machine" | B3 |
| 5 | P4.4 | Event-sourced reducer, cursor-paginated load, SSE follow | B2 |
| 6 | P4.5 | Render typed parts: text, collapsed reasoning, tool calls, inline diff, subagent cards, compaction markers | B2 |
| 7 | P4.6 | Composer: model/agent/command pickers, Queue vs Steer, editable queued items, interrupt | B2, B10 |
| 8 | P4.7 | Inline permission and form cards; "Always" writes a revocable permission | B8 |
| 9 | P4.8 | opencode client through `/s/{id}/oc` with `@opencode/client` | B1 |
| 10 | P4.9 | Virtualise the transcript with `@tanstack/react-virtual` | after 5 |
| 11 | P4.10 | Apply `session.environment` after relaunch; show the `env-status` entry | B3 |
| 12 | P4.11 | Mirror blocked sessions to the inbox | B6 |
| 13 | P3.10 | `LoginPageLayout` and a real login route | B7 |
| 14 | P5.1 | Changes tab: per-turn diff, VCS status, Create PR (CodeMirror 6) | B5 |
| 15 | P5.3 | Terminal tab: opencode PTYs and platform exec (`@xterm/xterm`) | B2 |
| 16 | P5.4 | Preview iframe with per-exposure permission check | B8 |
| 17 | P5.5 | Files tab: list, search, read, read-mostly editor | B3 |
| 18 | P5.6 | Design clauses relevant to touched paths | B4 |
| 19 | P5.7 | "Send to agent" on build/readiness failure | B2 |
| 20 | P5.8 | Theme CodeMirror and xterm from Vanilla tokens + Ubuntu Mono | after 14, 15 |
| 21 | P6.1 | Preview detail: exposures, Reset/Restart/Get a fresh copy/Start a session, TTL and hibernation | B9 |
| 22 | P6.2 | Design clause catalogue, clause pages, amendments by state, design map, debt dashboard | B4 |
| 23 | P6.3 | Amendment detail: redline diff, anchored comments, votes, approvals, impact report | B4 |
| 24 | P6.4 | Design discussion chat reusing the session components | B4 |
| 25 | P6.5 | "Needs you" inbox | B6 |
| 26 | P6.6 | Admin: users, quotas, roles via `@canonical/rebac-admin` | B7, B8 |
| 27 | P6.7 | Model accounts: add/test/remove keys write-only, spend per project, default model | B10 |
| 28 | P6.8 | In-session live cost counter and "who pays" | B10 |
| 29 | P6.9 | "Decided automatically" markers with confidence and override | B8 |
| 30 | P6.10 | Project AI settings: credential policy, allowed models, budgets, classification | B7, B10 |
| 31 | P7.6 | Storybook for the component library | none |
| 32 | P7.7 | Renovate config for the pinned versions | none |

P7.6 and P7.7 depend on nothing and can be done whenever there is capacity; they are pure hygiene.

### 5.1 Retractions from the implementation plan

The plan's P2.9 removed 19 frontend calls to endpoints that 404. Several of those correspond to tables that exist but have no routes. If B4–B6 land, the corresponding frontend modules must be rebuilt against real contracts rather than restored from git. Do not `git revert` those deletions.

---

## 6. Deviations to resolve in the work already shipped

Ordered by how much they will bite.

### 6.1 `dev.sh` cannot health-check the orchestrator
`wait_http` issues a bodyless `GET /api/env/status`, which returns `415`. The script reports "orchestrator up (415)", which reads as an error and is not one. Either add a bodyless `GET /api/env/health` route, or special-case the probe. This will also apply to any new `GET`-with-body route.

### 6.2 No way to discover registered environment specs
`EnvPanel` takes a free-text environment name defaulting to `dev`, because the orchestrator exposes no way to list specs. Add `GET /api/env/specs` and drive the panel from a picker.

### 6.3 Environment specs are in-memory
They are lost on every orchestrator restart, after which the UI correctly shows the spec-missing state with no way forward from the browser. Persist them (B9).

### 6.4 The log viewer is wired to a stub
`POST /api/env/logs` always returns `lines: []`. P5.2's log panel is correct against the contract and will always be empty.

### 6.5 Previews is unreachable through the UI
`PreviewsPage` is fully built and works, but it needs a `projectId`, and the only route that would supply one (`ProjectPage`) renders the unavailable state. The previews screen is currently reachable only by typing a URL. This resolves when B3 lands, but it means P6.1 cannot be demonstrated in a review today.

### 6.6 Six axe rules disabled in `src/testing/axe.ts`
These fail on `@canonical/react-components@4.11.3`'s own markup, not ours: `ThemeSwitcher` puts `aria-selected` on a plain `<button>`; `Tabs` sets the invalid `role="none presentation"` on its `<li>`, cascading into `aria-required-parent` and `aria-required-children`; `Input` sets `aria-errormessage` without a matching `aria-describedby`; `ApplicationLayout` renders a duplicate banner that CSS hides but jsdom does not. The exclusions are centralized and every other rule, including all serious and critical ones, still fails CI.

**Action:** file upstream issues and remove the exclusions on upgrade. Leaving them silently in place means real regressions of those same rule classes would pass unnoticed.

### 6.7 Vendored accessibility fixes are not an option
If the exclusions stay, the alternative to consider is patching the library through Vite's alias/resolve mechanism. That trades a dependency upgrade for a maintained fork, so it is not obviously better. Recorded here as a considered-and-rejected option rather than an oversight.

### 6.8 Session workspace navigation
The immersive workspace exposes only a link back to `/projects`. Inbox, settings and admin are unreachable from inside a session. Acceptable for now; revisit with P4.11 when the inbox is real.

---

## 7. Process fix: make the inventory impossible to get wrong

This is the item that prevents a repeat of §1, and it is cheap.

### 7.1 Generate the inventory instead of inferring it
Add `scripts/backend-inventory.mjs` that extracts every `.route(...)` registration from `crates/*/src/**/*.rs` and writes `docs/backend-inventory.md` with service, port, method, path and handler. Run it as part of the backend test suite. A route added without regenerating the inventory fails CI.

### 7.2 Assert the frontend only calls routes that exist
Add a vitest that reads `apiPaths` from `frontend/src/lib/routes.ts`, normalises the paths against the generated inventory, and fails on any path with no matching route. A path with a `{}` placeholder matches any segment. This alone would have caught the original error, and it keeps catching it as the backend moves.

### 7.3 Never infer capability from a status code
`415` and `405` mean the route exists. A missing route is `404`. Add this to the runbook in `docs/runbooks/` so the next person does not repeat the mistake.

### 7.4 Probe shape, not just reachability
`dev.sh` should assert the expected status class, not merely that something answered. `200/2xx/3xx` for health, and a specific expected code where a `4xx` is the correct answer.

---

## 8. Decisions needed before starting

1. **Are sessions supposed to be persisted?** There is no `sessions` table while 22 other domains have one. Either session metadata is deliberately runtime-only, or a table is missing. This determines whether P4.2/P4.3 are a read model or a new resource, and it is the cheapest question here to answer.
2. **What is the transcript event envelope?** P4.5 renders typed parts and P4.4 reduces over the log. The envelope must be stable and typed before any of that UI is written, or the reducer gets rewritten. Needs design-doc §8.2 signed off.
3. **Should `GET /api/env/status` keep taking a body?** It is legal and works, but it breaks generic health probes and is unusual enough to surprise the next reader. Consider `GET /api/env/status/{session}/{environment}` and keep the body form as an alias.
4. **Where does `rebac-admin` fit?** It is still not installed. It needs B8, and the answer determines whether roles live in a separate service or inside `core-api`.
5. **P6.1 has no entry point.** Is the previews screen meant to be reached from a project page (blocked on B3) or from its own top-level route? Affects whether B3 is on the critical path for P6.

---

## 9. Definition of done

The frontend is done when all of the following hold.

- Every plan phase P0–P7 is either implemented against a live endpoint or has a written, dated reason it is not, and that reason is current rather than inherited from the incorrect inventory in §1.
- Every screen in design doc §10.1 has a screen, and every §10.1 lifecycle transition is reachable from the UI — the P6 acceptance criterion, currently unmet and reachable only after B4.
- `npm run build`, `npm run lint` and `npm test` pass in `frontend/`, and `dev.sh` brings up a stack where each screen's data loads.
- The axe exclusions in `src/testing/axe.ts` are gone, either because the upstream defects are fixed or because a decision in §8 accepts them permanently.
- §7.1 and §7.2 are in CI, so a frontend call to a nonexistent route fails the build rather than reaching production.
