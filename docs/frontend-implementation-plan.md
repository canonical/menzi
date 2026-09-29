# Web UI Implementation Plan

| | |
|---|---|
| **Status** | Draft for review |
| **Date** | 28-SEP-26 |
| **Scope** | `frontend/` — bring the web UI to the architecture and design standards in the Menzi design plan (§8, §9, §10, §12, §13) |
| **Inputs** | Design plan §8 (Web UI architecture), §13 (UX and Canonical design standards), §6/§7 (agent runtime, environments), §10 (design governance) |

---

## Table of contents

1. [Diagnosis — why the UI looks primitive](#1-diagnosis--why-the-ui-looks-primitive)
2. [Current-state inventory](#2-current-state-inventory)
3. [Backend contract gaps the UI depends on](#3-backend-contract-gaps-the-ui-depends-on)
4. [Target frontend architecture](#4-target-frontend-architecture)
5. [Work plan by phase](#5-work-plan-by-phase)
6. [Automated enforcement (design doc §13.6)](#6-automated-enforcement-design-doc-136)
7. [Risks, decisions needed, open questions](#7-risks-decisions-needed-open-questions)

---

## 1. Diagnosis — why the UI looks primitive

The UI looks primitive for **four independent reasons**. All four were confirmed by
building the app and inspecting the emitted bundle. The first is the dominant one.

### 1.1 Vanilla Framework is declared but never compiled into the app

**This is the root cause.** The entire shipped stylesheet is **1,306 bytes** and
contains only hand-written rules. Vanilla Framework contributes **zero CSS**.

`frontend/dist/assets/index-B5Op3f-p.css` in full:

```css
body{margin:0;font-family:Ubuntu,...}
.l-application{min-height:100vh}
.l-application__header{position:fixed;top:0;left:0;width:250px;...}
...
.p-tabs__link.is-active{border-bottom:2px solid #007bff}
```

Every element in the app is already written with correct Vanilla class names
(`p-card`, `p-button--positive`, `p-tabs__link`, `l-application`, `p-form__control`),
but **none of them are styled**, because no Vanilla CSS is present.

There are two independent bugs stacked on top of each other:

**Bug A — a Sass `@import` inside a plain `.css` file.** `src/styles/index.css:1`:

```css
@import '../../node_modules/vanilla-framework/scss/_vanilla.scss';
```

`index.css` is a `.css` file, so Vite never runs the Sass preprocessor on it. Sass
partials are not compiled, and the bare `@import` is dropped from the output. The
file needs to be `index.scss` for any of this to work.

**Bug B — even as SCSS, that import emits nothing.** In `vanilla-framework@4.59.0`,
`scss/_vanilla.scss` is a manifest that `@import`s every partial and then only
**defines** a mixin; it never invokes it:

```scss
// node_modules/vanilla-framework/scss/_vanilla.scss:105
// Include all the CSS
@mixin vanilla {
  @include vf-base;
  @include vf-p-buttons;
  ...
}
```

The individual partials (`_patterns_buttons.scss` and friends) contain **only
`@mixin` definitions** — no rules. The consumer must call `@include vanilla;`
explicitly. Importing `_vanilla.scss` alone produces a file of nothing but
`@classreference` comments.

**Verified fix.** Compiling the correct pair:

```scss
@import "vanilla-framework/scss/_vanilla";
@include vanilla;
```

| | Current build | Correct build |
|---|---|---|
| CSS size | **1,306 bytes** | **639,903 bytes** |
| CSS rules | ~15 hand-written | **2,783** |
| `.p-button--positive` hits | 0 | 41 |
| `.p-button--negative` hits | 0 | 36 |
| `.p-card` hits | 0 | 18 |
| `.p-table` hits | 0 | 74 |
| `.p-tabs__link` hits | 0 | 74 |
| `.l-application` hits | 0 | 10 |
| `.p-form__control` hits | 0 | 2 |

That single missing `@include` is the difference between "primitive" and
"Canonical". Everything else in this document is smaller by comparison.

**Bug C — `sass` is not a declared dependency.** `frontend/package.json` has no
`sass` entry. `node_modules/sass` exists only because npm auto-installed
`vanilla-framework`'s peer dependency (`"peer": true` in the lockfile). This works
by accident and will break on a different npm version or a stricter CI runner.

**Bug D — the hand-written CSS actively fights Vanilla.** `src/styles/index.css`
re-implements Vanilla classes with hard-coded hex values — `#007bff`, `#ccc`,
`#dc3545`, `1px`, `0.25rem` — which design doc §13.3 explicitly forbids ("No
hard-coded colours, spacing, sizes, radii or shadows"). Once Vanilla loads, this
file must be deleted, not merged. It also overrides `.p-tabs__link` and
`.p-label` with a Bootstrap-blue `#007bff` instead of the Vanilla theme token.

### 1.2 `@canonical/react-components` is installed and never imported

`@canonical/react-components@4.11.3` is a declared dependency. It is imported
**zero times**:

```
$ grep -rn "react-components" frontend/src/
(no matches)
```

Every component in the app is hand-rolled HTML with Vanilla class names:

| Written by hand | Available in react-components, unused |
|---|---|
| `<table class="p-table">` ×4 pages | `MainTable`, `Table`, `TableHeader`, `TableCell`, `TableRow`, `ScrollableTable`, `TablePagination`, `ColumnSelector` |
| `<ul class="p-list">` | `List` |
| `<span class="p-label">` | `StatusLabel`, `Badge`, `Chip` |
| `<button class="p-button--positive">` | `Button`, `ActionButton`, `SummaryButton` |
| `<button class="p-button--link">` | `ContextualMenu`, `ActionButton` |
| hand-rolled `p-tabs` (Admin, Settings) | `Tabs` |
| hand-rolled `p-form` (Settings) | `Form`, `Field`, `Input`, `Switch`, `CheckboxInput`, `Select`, `Textarea` |
| `<div class="p-card">` | `Panel`, `Card`, `ContentCard` |
| `<h1 class="l-application">` layout | `ApplicationLayout`, `SideNavigation`, `SidePanel`, `CustomLayout` |
| none | `NotificationProvider`, `Notifications`, `ConfirmationModal`, `SegmentedControl`, `Accordion`, `EmptyState`, `SearchAndFilter`, `Spinner`, `Loader`, `Tooltip`, `SkipLink`, `ThemeSwitcher`, `LoginPageLayout`, `Code`, `CodeSnippet` |

71 components are available; the app uses 0. The whole design-doc §13.2 mapping
table is unused.

This also means the design doc §13.6 clause *"UI MUST use react-components or
Vanilla patterns where they exist"* is violated in every single file, and there is
no check that would catch it.

### 1.3 The styling pipeline is not set up for Vanilla at all

- `sass` is not a dependency (see Bug C).
- `vite.config.ts` has `resolve: { conditions: ['sass'] }`, which is the right
  idea — it makes `vanilla-framework`'s `exports` map resolve its `.` entry to
  `_index.scss` — but `import 'vanilla-framework'` is never used and `_index.scss`
  points at `./scss/vanilla`, a file that does not exist in the published package
  (the published manifest is `scss/_vanilla.scss`). So the bare-specifier path is
  also broken.
- There is no Vanilla `settings` override file, no theme handling, and no
  light/dark/paper support. Design doc §13.2 requires themes via **theme classes**
  on a wrapper element, not Sass variable overrides. `index.html` has no theme
  wrapper and no `<html class="is-light">`.
- The Ubuntu font is declared as a plain `font-family` on `body` rather than
  through Vanilla's font asset settings (design doc §13.3).
- There is no stylelint config to catch hard-coded values (design doc §13.6).

### 1.4 The app is a thin shell over a mostly non-existent backend

This is the reason the pages *behave* as primitives too, and it is independent of
styling. Of the **25 distinct endpoints the frontend calls, 19 do not exist**
anywhere in the Rust workspace.

| Frontend call | Backend |
|---|---|
| `GET/POST /api/v1/projects`, `GET /api/v1/projects/{id}` | **404 — no route** |
| `GET/POST /api/v1/projects/{id}/sessions` | **404 — no route** |
| `GET /api/v1/sessions/{id}` | **404 — no route** |
| `POST /api/v1/sessions/{id}/prompts` | **404 — no route** |
| `GET /api/v1/sessions/{id}/messages` | **404 — no route** |
| `GET/POST /api/v1/workspaces/{id}/environment/*` | **404 — no route** |
| `GET /api/v1/projects/{id}/design/clauses` | **404 — no route** |
| `GET/POST /api/v1/projects/{id}/design/amendments` | **404 — no route** |
| `POST /api/v1/projects/{id}/design/amendments/{id}/approve` | **404 — no route** |
| `POST /api/v1/projects/{id}/design/amendments/{id}/reject` | **404 — no route** |
| `GET /api/v1/projects/{id}/previews` | ✅ proxied to `menzi-previews` |
| `GET/POST /api/v1/previews`, `/{id}`, `/{id}/reset`, `/{id}/restart` | ✅ proxied to `menzi-previews` |

`crates/core-api` is 429 lines with three modules (`health`, `orgs`,
`previews_proxy`) and executes exactly three SQL statements, all against `orgs`.
22 of the 24 migrated tables are never read or written by any Rust code.

Consequences visible in the browser:

- Every page's `useEffect` fires a 404, `.catch(console.error)` swallows it, and
  the page renders an **empty table with no error state**. There is no error
  boundary, no error UI, no empty state, no retry — a failed fetch is
  indistinguishable from "no data".
- `AdminPage` and `SettingsPage` are **pure placeholders** — static `<p>` text and
  uncontrolled `<input>`s with no state, no submit handler and no API. They are
  the only two pages that "work", because they do nothing.
- `DesignPage` and `PreviewsPage` are the only pages with any real wiring, and
  Previews is the only one with a backend.

### 1.5 Routing, auth and state are scaffolding

- **No catch-all route.** `App.tsx` defines 8 routes and no `*`. Any unmatched URL
  renders a blank page.
- **Two nav links point at non-existent routes.** `Layout.tsx:20` links to
  `/design`, but the route is `/projects/:projectId/design` → **blank page**.
  `ProjectsPage.tsx:36` links to `/projects/${project.id}`, which matches no route
  → **blank page**. There is no link from anywhere to previews, design, or
  sessions, and no link to the environment page.
- **Navigation bypasses the router.** `Layout.tsx` and `ProjectsPage` use raw
  `<a href>`, forcing a full page reload and discarding all client state, while
  `react-router-dom@7` is installed and `useParams` is already used elsewhere.
- **There is no login.** `Layout` reads `useAuthStore().user`, but nothing ever
  calls `login()`, so the user footer never renders and "Sign out" is unreachable.
  The token is read from `localStorage.menzi_token` and sent as a bearer token,
  but the backend has **no auth middleware on any route** (`menzi-auth`'s
  `TenantExtractor` is defined and never used; there is no `/auth/*` endpoint;
  `menzi login` in the CLI only logs a line).
- **Data fetching is hand-rolled `useEffect` + `useState` + `Promise.all`** in all
  five data pages. Design doc §8.2 calls for TanStack Query, an event-sourced
  per-session store, and a reducer over the log.
- **No live updates anywhere.** `axum` has the `ws` feature enabled workspace-wide
  and **no handler uses it**. There is no SSE and no streaming response anywhere in
  the backend. The session-proxy exposes a
  `GET /api/tunnel/{session_id}/transcript` that returns a flat array with **no
  cursor and no follow** parameter, so the "live" conversation is impossible
  without backend work.
- **No terminal, no diff viewer, no code viewer, no iframe preview** — the three
  highest-value panels of the session screen (design doc §8.3) are absent.
  CodeMirror 6, xterm.js and TanStack Virtual/Virtual are not even dependencies.

### 1.6 Test and tooling coverage

| Capability | Design doc §13.6 | Status |
|---|---|---|
| Component tests | required | **None.** `vitest` runs with `environment: 'node'`; no jsdom, no `@testing-library/*` |
| axe accessibility | required | **Absent** |
| Storybook | required | **Absent** |
| Visual regression | required | **Absent** |
| Stylelint (no literals) | required | **Absent** |
| Copy linter | required | **Absent** |
| ESLint ban on raw `<button>`/`<table>`/`<dialog>` | required | **Absent** |
| String catalogue | required | **Absent** — strings are inline literals in JSX |
| i18n | required | **Absent** |
| CI: `npm run build && lint && test` | present ✅ | works; Node 22, `npm ci` |

The 19 passing tests all cover the thin `api.ts` wrappers against `fetch` mocks.
They give the appearance of coverage while testing nothing a user sees. There are
**no tests for any component or page**.

---

## 2. Current-state inventory

### 2.1 Files

```
frontend/
├─ index.html                  12 lines   no theme wrapper, no meta theme-color
├─ package.json                           6 deps, 0 declared for styling/test/a11y
├─ vite.config.ts               21 lines   sass condition set; node test env
├─ eslint.config.js             28 lines   base recommended only
├─ tsconfig.json                          strict, noUnusedLocals — good
├─ vitest.setup.ts
└─ src/
   ├─ main.tsx                  10 lines
   ├─ App.tsx                   30 lines   8 routes, no catch-all, no error boundary
   ├─ components/Layout.tsx     36 lines   raw <a>, no router, no layout component
   ├─ lib/api.ts                34 lines   fetch wrapper, localStorage token
   ├─ lib/types.ts             110 lines   hand-written types, no codegen
   ├─ stores/auth.ts            24 lines   zustand, login() never called
   ├─ styles/index.css         117 lines   BROKEN Vanilla import + hard-coded hex
   └─ features/
      ├─ projects/ProjectsPage.tsx    47   table of name/slug/role
      ├─ sessions/SessionPage.tsx      76   message list + text input
      ├─ environments/EnvironmentPage  92   component list + 3 buttons
      ├─ previews/PreviewsPage.tsx     80   table + 3 actions
      ├─ design/DesignPage.tsx        101   two tables + approve/reject
      ├─ admin/AdminPage.tsx           46   PLACEHOLDER
      └─ settings/SettingsPage.tsx     86   PLACEHOLDER
```

### 2.2 Dependency changes required

**Add:**

| Package | Purpose | Design doc |
|---|---|---|
| `sass` (dev) | compile Vanilla | §13.1 — **must be explicit, not a phantom peer** |
| `@canonical/react-components` (promote to a real import) | components | §13.1 |
| `@canonical/rebac-admin` | permissions admin | §13.1 |
| `@tanstack/react-query` | REST data layer | §8.2, §8.7 |
| `@tanstack/react-virtual` | transcript virtualisation | §8.2, §8.7 |
| `@opencode/client` | typed opencode client | §6.3, §8.7 |
| `codemirror` / `@codemirror/*` | files + diffs | §8.7 |
| `@xterm/xterm` + `@xterm/addon-fit` | terminals | §8.7 |
| `jsdom` + `@testing-library/react` + `user-event` | component tests | §13.5, §13.6 |
| `vitest-axe` / `axe-core` | a11y gate | §13.5, §13.6 |
| `stylelint` + `stylelint-config-standard-scss` | no literals | §13.6 |
| `prettier` | formatting | — |
| `@canonical/vanilla-framework` (settings only) | theme tokens | §13.3 |
| storybook + builders (dev) | component catalogue | §13.6 |

**Remove:** the hand-written `p-*` class overrides in `src/styles/index.css`.

### 2.3 Target directory shape

```
frontend/src/
├─ app/                     providers, router, error boundaries, query client
│  ├─ App.tsx  router.tsx  providers.tsx  ErrorBoundary.tsx  QueryClient.ts
├─ components/              shared, Vanilla/react-components only
│  ├─ AppShell/  PageHeader/  DataTable/  StatusLabel/  EmptyState/
│  ├─ ConfirmDialog/  CopyButton/  RelativeTime/  ErrorState/  Skeleton/
│  └─ conversation/  environment/  design/  previews/  (feature widgets)
├─ lib/
│  ├─ api/          client.ts, errors.ts, orgs.ts, projects.ts, sessions.ts,
│  │                workspaces.ts, environments.ts, previews.ts, design.ts,
│  │                notifications.ts, users.ts
│  ├─ opencode/     client.ts (wraps @opencode/client), events.ts (log+SSE)
│  ├─ types/        generated.ts (from the pinned OpenAPI document)
│  ├─ format/       time.ts, dates.ts, numbers.ts
│  └─ routes.ts     single source of truth for paths
├─ hooks/           usePolling, useDebounced, usePermission, useOrg
├─ stores/          session.ts (log reducer), ui.ts, org.ts
├─ strings/         catalogue.ts  ← every UI string, one reviewable file
├─ styles/
│  ├─ index.scss            @import _vanilla; @include vanilla;
│  ├─ _settings.scss        Vanilla settings overrides ONLY
│  ├─ _theme.scss           light/dark/paper via theme classes
│  └─ _components.scss      only the documented custom components
└─ features/        projects, sessions, environments, previews, design, inbox,
                    settings, admin
```

---

## 3. Backend contract gaps the UI depends on

These are **not** frontend work, but the frontend cannot reach the design doc's
target state without them. Listed so they can be sequenced in parallel.

| # | Gap | Blocks | Design doc |
|---|---|---|---|
| B1 | `GET /api/env/status` is registered for GET but takes a JSON body | Environment tab | §7.3 |
| B2 | `/api/env/logs` is a hard-coded stub returning `lines: []` | Logs pane | §7.6 |
| B3 | No auth on any route; no `/auth/*`; no OIDC; `TenantExtractor` unused | Everything | §14 |
| B4 | No SSE / WebSocket / streaming anywhere | Live conversation, live logs, presence | §8.2 |
| B5 | `session-proxy` transcript endpoint has no cursor and no follow | Resume, fan-out | §8.1, §8.2 |
| B6 | No `projects`, `sessions`, `workspaces`, `environments`, `design` routes | 5 of 7 pages | §4.2 |
| B7 | `core-api` serves no OpenAPI document (`ApiDoc` covers 4 of 10 routes) | Typed client | §8.7 |
| B8 | `llm-gateway` ignores `stream: true`; no cost/usage surfaced | Live cost counter | §12.4 |
| B9 | Notifications router is **empty** (`Router::new()`, all 404) | Inbox, "Needs you" | §9.7 |
| B10 | `git-integration` router is **empty**; no webhook ingress | Create PR, PR comments | §9.5 |
| B11 | `design-service` router is **empty** | Design tab | §10.9 |
| B12 | `workflow` router is **empty** | Job progress, prebuild status | §7, §10.7 |
| B13 | In-memory state everywhere; 22 of 24 tables unused | Any real data | §5, §6 |
| B14 | No CORS layer; SPA on a different origin is blocked | Dev and prod | §14 |
| B15 | `session-proxy` and `supervisor` are not started by `infra/local/dev.sh` | The whole session flow | §6 |
| B16 | `llm-gateway` `GET /v1/models` returns a bare array with `models: []` | Model picker | §8.3 |
| B17 | Rendered `opencode.json` points at `/v1/p/openrouter`, a path no service serves | Agent LLM calls | §12.3, App. D |
| B18 | `menzi-supervisor mcp design|env` referenced in config; binary has no such subcommand | Design + env MCP | §11 |
| B19 | `create_org`/`get_org` return HTTP 200 `{id:"placeholder"}` on any DB error | Error handling | §4.2 |
| B20 | `EnvRelaunchRequest.reset_data` and `EnvExecRequest.component` are parsed and discarded | Relaunch + reset data | §7.4 |

---

## 4. Target frontend architecture

Aligned with design doc §8 and §13.

### 4.1 The proxy rule (design doc §8.1)

The browser **never** talks to opencode directly.

```
Browser
  ├─ platform API  ──────────────▶ Core API        (/api/v1/*)
  ├─ @opencode/client baseUrl
  │  = /s/{platformSessionId}/oc ─▶ Session Proxy  (allowlisted, fanned out)
  └─ WebSockets ─────────────────▶ PTY / exec      (terminals only)
```

The proxy's allowlist already exists in `crates/session-proxy/src/config.rs` and
defines the reachable surface: `session.*`, `session/:id/prompt|log|interrupt|diff`,
`vcs/status`, `vcs/diff`, `model`, `agent`, `switchModel`, `switchAgent`, `fs/*`,
`pty/*`, `config` (GET only). Deny-listed: `POST /api/config`, `/api/auth/*`,
`/api/plugin/*`, `/api/mcp/*`.

### 4.2 Data layer (design doc §8.2)

- **REST** → TanStack Query, with query keys derived from `lib/routes.ts`.
- **Session state** → an event-sourced reducer over the opencode session log.
  Initial load is a cursor-paginated message list; live updates resume with
  `log?after=<seq>&follow=true`. The store holds `lastSequence` and replays
  exactly from there after a reconnect.
- **Rendering** → `@tanstack/react-virtual` for the transcript.
- **Transport** → SSE for events, WebSocket for terminals only. Until B4 lands,
  a `usePolling` fallback with exponential backoff, behind one interface so the
  swap is a single change.

### 4.3 Screens (design doc §8.3)

| Screen | Route | Priority |
|---|---|---|
| Org switcher + global shell | `/` | P1 |
| **"Needs you" inbox** | `/inbox` | P3 |
| Projects list | `/projects` | P1 |
| Project detail (repos, `devenv.yaml`, members, policies, prebuild) | `/projects/:projectId` | P2 |
| **Session workspace** (the main screen) | `/projects/:projectId/sessions/:sessionId` | P2 |
| Environment | `/workspaces/:workspaceId/environment` | P3 |
| Previews list | `/projects/:projectId/previews` | P4 |
| Preview detail (exposures, actions, feedback) | `/previews/:previewId` | P5 |
| Design catalogue | `/projects/:projectId/design` | P5 |
| Amendment detail (redline, comments, approvals, impact) | `/projects/:projectId/design/amendments/:amendmentId` | P6 |
| Design discussion (chat + live brief) | `/projects/:projectId/design/discussions/:discussionId` | P6 |
| Settings (model accounts, notifications, personal layer) | `/settings` | P4 |
| Admin (users, quotas, roles via rebac-admin) | `/admin` | P6 |
| Not found | `*` | P1 |

### 4.4 The session workspace (design doc §8.3 diagram)

Three columns, all inside `ApplicationLayout`:

- **Left** — `SideNavigation`: THREADS (`ListTree`, forks inline) and TURNS
  (each turn with "Revert files" and "Restore machine").
- **Centre** — the conversation: parts rendered by type (text, collapsed
  reasoning in an `Accordion`, tool calls with inline diffs, subagent cards,
  compaction markers, model/agent switch markers), inline permission and form
  cards, and the composer (model picker, agent picker, attachments, Queue/Steer
  `SegmentedControl`, queued items, Interrupt).
- **Right** — `SidePanel` with `Tabs`: Changes, Env, Terminal, Preview, Files,
  Design.

---

## 5. Work plan by phase

Phases are ordered so each one leaves the app in a shippable, demonstrable state.
**P0 alone transforms the look of the product** and should land first and fast.

---

### P0 — Make Vanilla actually load (day 1, ~1 hour)

**This is the single highest-value change in the plan.** It converts a 1.3 KB
stylesheet into 640 KB of Canonical design system and is the whole difference
between the current appearance and the target.

| # | Task | File |
|---|---|---|
| P0.1 | Add `sass` to `devDependencies`; make it explicit, not a phantom peer | `package.json` |
| P0.2 | Rename `src/styles/index.css` → `src/styles/index.scss` so Vite runs the Sass preprocessor | `src/styles/index.css` |
| P0.3 | Replace the import with the form that actually emits CSS: `@import "vanilla-framework/scss/_vanilla";` **and `@include vanilla;`** | `src/styles/index.scss` |
| P0.4 | Delete the entire hand-written block — layout hacks, `.p-tabs__*` overrides, `.p-label`, `.p-form__control`, `.p-button--*`. Vanilla provides all of them and the design doc §13.3 forbids overriding Vanilla classes | `src/styles/index.scss` |
| P0.5 | Add the theme wrapper so Vanilla themes work via classes, not Sass variables: `<html class="is-light">` + a `ThemeSwitcher` later | `index.html` |
| P0.6 | Set the Ubuntu font through Vanilla's font asset settings, not a raw `body { font-family }` | `src/styles/_settings.scss` |
| P0.7 | Import the **first** react-components module to prove the SCSS-through-node_modules path works | `src/main.tsx` |
| P0.8 | Build and assert the emitted CSS contains Vanilla | CI check in P6.1 |

**Acceptance:** `npm run build` emits > 300 KB of CSS containing
`.p-button--positive`, `.p-card`, `.p-table`, `.l-application`. No hex literals
remain in `src/styles/`.

**Risks:** `@include vanilla` pulls the whole framework (~640 KB raw, ~90 KB
gzipped). If that is unacceptable, import individual partials and include only the
patterns the app uses — but the `@mixin vanilla` route is the documented one and
is preferable to hand-maintaining a partial list.

---

### P1 — Application shell, routing, and real feedback states (1 day)

The app currently shows a blank page for two of its own nav links and renders
nothing at all for an unknown URL. Fix the frame before building screens in it.

| # | Task | Notes |
|---|---|---|
| P1.1 | Replace `Layout.tsx` with `ApplicationLayout` + `SideNavigation` from react-components | §13.2 "Application layout" |
| P1.2 | Convert every `<a href>` to react-router `<Link>`; kill full page reloads | P1.1 |
| P1.3 | Add `path="*"` → `EmptyState` "Page not found" | Blank-page bug |
| P1.4 | Fix the `/design` nav link → `/projects/:projectId/design` | Blank-page bug |
| P1.5 | Add `/projects/:projectId` and wire `ProjectsPage` rows to it | Blank-page bug |
| P1.6 | Add a route-level `ErrorBoundary` and a global one | §13.5 |
| P1.7 | Build a shared `<DataState>`: loading `Spinner`/`Loader`, error `ErrorState` with retry, empty `EmptyState`, content | Replaces every `.catch(console.error)` |
| P1.8 | Add a route transition `Loader` | |
| P1.9 | Move all UI strings into `src/strings/catalogue.ts` | §13.4 |
| P1.10 | Switch to react-router data APIs or at minimum make every `useParams` route guarded | |

**Acceptance:** every route in the app renders; no link produces a blank page; any
failing API call shows a retryable error rather than an empty table.

---

### P2 — Rebuild the data layer (2 days)

| # | Task | Notes |
|---|---|---|
| P2.1 | Add `@tanstack/react-query`; create a `QueryClient` with sane defaults | §8.2 |
| P2.2 | Create `src/lib/routes.ts` as the single source of truth for every path; derive query keys from it | |
| P2.3 | Split `lib/api.ts` into per-domain modules under `lib/api/` | |
| P2.4 | Proper `ApiError` handling: parse the error body, distinguish 401/403/404/409/5xx, surface a message | Today `await response.text()` is thrown raw into `console.error` |
| P2.5 | Migrate all five data pages from `useEffect`+`Promise.all` to query hooks | |
| P2.6 | Handle 401 globally: redirect to login, clear the token | B3 |
| P2.7 | Add `usePolling` for anything the backend cannot stream yet (B4) | §8.2 |
| P2.8 | Replace `lib/types.ts` with types generated from the pinned OpenAPI document | §8.7, B7 |
| P2.9 | Until B7, write the platform types by hand **against the actual routes** and delete the 19 calls to endpoints that do not exist | Prevents the UI encoding a fiction |

**Acceptance:** no `useEffect`-and-`setState` data fetching remains outside
`hooks/`; every list has loading, error, empty and content states.

---

### P3 — Convert the pages to Vanilla components (2 days)

Every table, form, tab set and list is currently hand-rolled HTML.

| # | Task | Maps to design doc §13.2 |
|---|---|---|
| P3.1 | `ProjectsPage` → `MainTable` + `SearchAndFilter` + `Pagination` | Lists → Tables with sorting and pagination |
| P3.2 | `EnvironmentPage` → `Panel`, `List`, `StatusLabel`, `ActionButton`; `Launch`/`Relaunch`/`Tear down` become `ConfirmationModal` for the destructive one | States → Status labels; destructive → Modal |
| P3.3 | `PreviewsPage` → `MainTable` + `ContextualMenu` per row + `StatusLabel` badges | Actions → Contextual menu |
| P3.4 | `DesignPage` → `MainTable` for clauses, `MainTable` for amendments, `Badge` for enforced/implementing/waived, `Accordion` for filters | §10.9 |
| P3.5 | `AdminPage` → real `Tabs`, then `@canonical/rebac-admin` for roles | §13.1 |
| P3.6 | `SettingsPage` → real `Tabs`, `Form`/`Field`/`Input`, `Switch`, **wired to the API** with validation and a `Spinner` on submit | §12.7 |
| P3.7 | `Layout` → `NotificationProvider` + `Notifications` (toasts) | Notifications (inline and toast) |
| P3.8 | Add a `ThemeSwitcher` bound to Vanilla theme classes (light/dark/paper) | §13.2 |
| P3.9 | Add `SkipLink` and correct landmark structure | §13.5 |
| P3.10 | Add `LoginPageLayout` and a real login route | §13.2; blocked on B3 |

**Acceptance:** zero raw `<table>`, `<button>`, `<dialog>`, `<form>` or
hand-rolled `p-tabs` markup remains in `src/features/`.

---

### P4 — The session workspace (design doc §8.3) — 4 days

The main screen, and currently a 76-line placeholder with a text input.

| # | Task | Notes |
|---|---|---|
| P4.1 | Three-column `ApplicationLayout` with `SideNavigation` and `SidePanel` | §8.3 |
| P4.2 | **Threads** column: `ListTree` with forks | §8.3 |
| P4.3 | **Turns** column: each turn with "Revert files" and "Restore machine" | §8.5 |
| P4.4 | **Conversation**: event-sourced reducer over the session log, cursor-paginated initial load, `after=<seq>&follow=true` live | §8.2; blocked on B4/B5 |
| P4.5 | Render parts by type: text, collapsed reasoning (`Accordion`), tool calls (shell output, edit with inline diff), subagent cards, compaction markers, model/agent switch markers | §8.3 |
| P4.6 | **Composer**: model picker, agent picker, command/skill pickers, attachments, **Queue vs Steer** `SegmentedControl`, editable/cancellable queued items, Interrupt with optional resume | §8.3 |
| P4.7 | **Inline cards** for permission requests and form questions; "Always" creates a saved permission, revocable in project settings | §8.3, §6.5 |
| P4.8 | Wire the opencode client through the session proxy at `/s/{id}/oc` with `@opencode/client` | §8.1 |
| P4.9 | Virtualise the transcript with `@tanstack/react-virtual` | §8.2 |
| P4.10 | After each relaunch, apply `session.environment` variables and show the `env-status` instruction entry | §8.4 |
| P4.11 | Mirror blocked sessions to the global inbox | §8.3, §9.7 |

**Acceptance:** a real session streams, renders typed parts, accepts prompts with
queue/steer, and answers a permission request inline.

---

### P5 — Right-hand tabs (design doc §8.3) — 4 days

| # | Task | Library | Notes |
|---|---|---|---|
| P5.1 | **Changes**: per-turn diff, VCS status/diff vs review base, Create PR | CodeMirror 6 | §8.3, §8.7 |
| P5.2 | **Env**: topology, Launch/Relaunch/Tear down, per-component logs, exposure URLs | — | Blocked on B1, B2, B20 |
| P5.3 | **Terminal**: opencode PTYs and platform exec, persistent PTY handoff | `@xterm/xterm` | §7.6, §8.3; blocked on B4 |
| P5.4 | **Preview**: iframe of an `http` exposure through the gateway, with a **per-exposure permission check** — shells and TCP forwards are code execution | §7.6, §9.6 | |
| P5.5 | **Files**: list, search, read, read-mostly editor | CodeMirror 6 | §8.3 |
| P5.6 | **Design**: clauses relevant to touched paths, inline violations | — | §8.3 |
| P5.7 | **Send to agent**: post logs as a synthetic message on build/readiness failure | — | §7.10, §8.4 |
| P5.8 | Theme every third-party widget from Vanilla tokens + Ubuntu Mono; document each as a custom component with justification | — | §13.2 "Gaps" |

**Acceptance:** all six tabs render real data; CodeMirror and xterm read their
colours from Vanilla theme tokens so light and dark both work.

---

### P6 — Governance, previews, admin (4 days)

| # | Task | Design doc |
|---|---|---|
| P6.1 | Previews: list with `SearchAndFilter`; create (pinned/live `SegmentedControl`, draft labelling); detail page with exposures, Reset/Restart/Get a fresh copy/**Start a session from this**, feedback threads, **Address with agent**; TTL and hibernation state | §9.4 |
| P6.2 | Design: clause catalogue with scope/area/status/level filters and enforced/implementing/waived badges + violation counts; clause pages; amendments grouped by state; design map; design debt dashboard | §10.9 |
| P6.3 | Amendment detail: redline diff, sentence-anchored comments, 👍/👎, approvals panel, impact report, origin discussion | §10.3 |
| P6.4 | Design discussion: chat plus live brief, reusing the session UI components | §10.2 |
| P6.5 | "Needs you" inbox: sessions blocked on permission/form, idle with changes, approvals waiting, previews commented | §8.3, §9.7 |
| P6.6 | Admin: users, quotas, roles via `@canonical/rebac-admin` | §13.1, §9.8 |
| P6.7 | Settings → Model accounts: add/test/remove keys **write-only** (replace or test, never reveal); spend per project; default model | §12.7 |
| P6.8 | In-session live cost counter and a "who pays" indicator | §12.7 |
| P6.9 | "Decided automatically" markers with confidence and one-click override on auto-approved actions | §12.7 |
| P6.10 | Project AI settings: credential policy, allowed models, role→model mapping, budgets, data classification, decision list with mode/thresholds/shadow accuracy | §12.2, §12.6 |

**Acceptance:** every object in design doc §10.1 has a screen and every lifecycle
transition in §10.1 is reachable from the UI.

---

### P7 — Quality gates (2 days, running alongside P1–P6)

See §6 below for the full enforcement plan. Phase ordering note: **P7.1 and P7.2
should land in P0/P1**, not at the end — they are what stop the regression that
caused the current problem.

---

## 6. Automated enforcement (design doc §13.6)

The design doc's §13.6 is a self-referential dogfooding list: Menzi's own UI must
satisfy the clauses Menzi will check in other people's code. This is what makes
the current state impossible to reach twice.

### 6.1 The Vanilla regression guard — **P0, ship first**

A build-time assertion that Vanilla is actually in the bundle. This is the check
whose absence allowed a 1.3 KB stylesheet to ship as "Canonical".

```json
// package.json
"build": "tsc -b && vite build && node scripts/assert-vanilla.mjs"
```

`scripts/assert-vanilla.mjs` fails the build if the emitted CSS is below a
threshold or missing sentinel classes. Prevents the exact regression in §1.1.

### 6.2 ESLint — ban raw markup where a component exists

- `no-restricted-syntax` on `JSXOpeningElement` for `button`, `table`, `thead`,
  `tbody`, `tr`, `th`, `td`, `dialog`, `select`, `input`, `label`, `form` in
  `src/features/**` and `src/components/**`.
- `no-restricted-imports` banning every UI library other than
  `@canonical/react-components` and `vanilla-framework`.
- Existing `react-hooks` and `react-refresh` rules stay.

### 6.3 Stylelint — no hard-coded values

- `color-no-hex`, `color-no-rgb`, `declaration-property-unit-allowed-list`
  restricted to `rem|em|ch|%`, `number-max` for spacing, `unit-no-unknown`.
- An allowlist file for the documented custom components (§13.2 "Gaps").
- Vanilla class overrides (`selector-class-pattern` targeting `p-*`/`l-*`) banned
  outright per §13.3.

### 6.4 Copy linter

Custom ESLint rules over `src/strings/catalogue.ts` implementing §13.4: sentence
case, no "please"/"thank you", no "e.g."/"i.e.", prefer "Open" over "View",
button labels verb-first and ≤ 3 words, notification titles verb-phrased without
punctuation and descriptions with it, error copy that does not blame. Agent output
is content, not UI copy, and is exempt.

### 6.5 Accessibility — axe

- `vitest` environment moves from `node` to `jsdom`; add
  `@testing-library/react` and `user-event`.
- `vitest-axe` on every component render; **zero serious violations** fails CI.
- Keyboard-path tests for permission cards, form cards, the composer and all
  `ContextualMenu` actions (§13.5).
- Streaming output through throttled polite live regions (§13.5).
- xterm.js screen-reader mode with an escape shortcut (§13.5).
- Manual Orca/NVDA pass per release — not automatable.

### 6.6 Storybook + visual regression

- Storybook for every component and state across light/dark/paper.
- Visual regression on every story. This is also the **gate for Vanilla upgrades**,
  since Vanilla has shipped breaking changes in minor releases.
- CI check that a custom component has a Storybook entry **and** a written
  justification (§13.6).
- Light and dark checked in the UI definition of done (§13.7).

### 6.7 Pinning

Pin Vanilla and react-components exactly; Renovate for updates gated on the visual
regression suite. `sass` pinned to a version the Vanilla peer range accepts.

---

## 7. Risks, decisions needed, open questions

### 7.1 Risks

| Risk | Mitigation |
|---|---|
| `@include vanilla` ships ~640 KB raw (~90 KB gzipped) | Acceptable for an internal developer tool; measure in P0. If it matters, import individual partials and include only what is used — but that needs a process to keep the list honest |
| The UI outruns the backend (19 of 25 endpoints do not exist) | P2.9: delete calls to non-existent endpoints. Build screens against the routes that exist and mock the rest, or land the backend first. **Do not let the UI encode a contract the backend does not implement** — that is how the current 19 phantom endpoints got there |
| No auth anywhere (B3) | Everything behind a login is decorative until OIDC lands. Sequence P3.10 after B3 |
| No live updates anywhere (B4) | `usePolling` behind a single interface so SSE is a swap, not a rewrite |
| opencode v2 API churn (§6.3, §8.6) | Pin the version, read it from server-info, keep the compatibility layer in the proxy, contract-test in CI, flag experimental routes behind UI feature flags |
| react-components ships 71 components with their own SCSS | Confirm the SCSS-through-`node_modules` path in P0.7 before adopting broadly |
| Pragma vs Vanilla direction (§20) | **Open question for Canonical's design team** — confirm at kick-off. Do not adopt Pragma components in new work until answered |

### 7.2 Decisions needed before implementation starts

1. **How much Vanilla to load.** Full `@include vanilla` (~640 KB) or a curated
   partial list? Recommendation: full, for an internal tool, and revisit after
   measuring.
2. **How to handle the 19 non-existent endpoints.** (a) Build the backend first,
   (b) build the UI against a mock layer, (c) ship screens for the endpoints that
   exist. Recommendation: (c) for the previews path — it is genuinely wired — plus
   (a) for the session path, which is the product's core loop.
3. **React 18 vs 19.** `react-components@4.11.3` supports both; the app pins 18.
   Align deliberately.
4. **Route structure.** The design doc's screens imply nesting under
   `/projects/:projectId`. Confirm, because the current flat
   `/workspaces/:workspaceId/environment` does not fit the target IA.
5. **The CI copy linter and stylelint will fail on the current codebase.** Agree
   whether to land them with a ratchet (fix forward, new code only) or fix
   everything first. Recommendation: ratchet.

### 7.3 Explicitly out of scope for the frontend

- Building the missing backend endpoints (tracked in §3, owned by the services).
- The `collocate` spike (§7.7) — backend.
- Remote Design MCP for laptops (§11.7) — separate surface.
- Charm/rock packaging for the UI (§15.1) — `paas-charm` scaffolding, no UI code
  changes.
