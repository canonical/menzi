# Chat and Tool-Call Rendering — Technical Implementation Plan

| | |
|---|---|
| **Status** | Draft for review |
| **Date** | 29-SEP-26 |
| **Scope** | Turn the Code page chat from a comma-joined tool summary into a structured transcript of agent turns, tool steps and reasoning |
| **Design** | `docs/chat-tool-calls-plan.md` (what and why) |
| **Codebase** | `frontend/` React 19 + `@canonical/react-components`; `crates/session-proxy` Rust/axum |

---

## 1. Where the code stands

| Concern | Location | State |
|---|---|---|
| Turn rendering | `src/features/sessions/ChatPanel.tsx` | `textOf()` concatenates text; `toolSummary()` joins tool names with commas |
| Part types | `src/lib/types.ts` | `OpencodeMessagePart` is an open-ended `{ type: string; [key: string]: unknown }` |
| Transport | `src/lib/api/opencode.ts` | `listMessages`, `sendPrompt`; `eventStreamUrl()` is exported but unused |
| Data fetching | `ChatPanel` | `refetchInterval: 4000` polling |
| Session tabs | `CodePage.tsx` | tab bar, split into chat and review columns |
| Diff view | `CodeReviewPanel.tsx` | working, uses `app-diff__line--*` tokens |

The gap between this and the target is concentrated in one file. `ChatPanel.tsx` is
the only component that consumes message parts, so the change is additive for
everything else.

---

## 2. Two findings that change the plan

Both verified against the running stack, not inferred.

### 2.1 opencode's tool state is a discriminated union, and `input` changes type

From `GET /openapi.json` on v2.0.6, schema `Session.Message.Assistant.Tool`:

```
state : anyOf ToolState.Streaming | Running | Completed | Errored   (required)
```

| `status` | `input` | other |
|---|---|---|
| `streaming` | `string` — partial, unparsed | — |
| `running` | `object` | `metadata` |
| `completed` | `object` | `content: ToolContent[]`, `metadata` |
| `error` | `object` | `error: {message, status}`, `content?` |

Two consequences. `input` is a `string` while a call is in flight, so any renderer
that assumes `object` will emit `[object Object]` at exactly the moment the user is
watching. And failure is a **status**, not a boolean; there is no `failed: true`.

This is why step 1 is a total normaliser rather than optional tidying.

### 2.2 The proxy cannot stream — this blocks live updates

Measured, not assumed:

```
opencode  GET /api/event  direct  -> data: {...server.connected...}  immediately
Menzi     GET /api/event  via :5173 -> (nothing, 6s timeout)
```

`forward()` in `crates/session-proxy/src/proxy.rs` ends with:

```rust
let bytes = upstream.bytes().await;          // buffers the entire body
Response::new(Body::from(bytes.clone()))      // …then responds
```

`GET /api/event` is an infinite SSE stream, so `.bytes()` never returns and nothing
reaches the browser. Any route added to the allowlist today inherits this.

This does **not** affect `/api/session/:id/message`, which is a finite JSON body and
works today. Steps 1–6 are therefore unblocked. Step 7 needs the streaming work in
§5, and must not be attempted by adding a route to the allowlist.

---

## 3. Constraints this work is bound by

Discovered the hard way earlier in this repo; each has already bitten once.

| Gate | Rule | Consequence here |
|---|---|---|
| `eslint.config.js` | raw `<button> <table> <dialog> <form> <input> …` banned in `src/features` and `src/components` | use `Button`, `MainTable`, `Form`, `Accordion` |
| `.stylelintrc.json` | `color-no-hex`; `selector-class-pattern` rejects `p-*`/`l-*` | new CSS uses `--vf-*` tokens and `app-*` classes only |
| `scripts/check-copy.mjs` | sentence case, no "please"/"e.g.", no empty strings | every user-facing string goes in `strings/catalogue.ts` |
| `backend-contract.test.ts` | a frontend call to an unregistered route fails the suite | any new path must exist in the generated inventory |
| `src/testing/axe.ts` | six upstream react-components rules are disabled | new components are still axe-checked against everything else |

Two traps to avoid, both previously hit: `p-code-snippet__line--added` does not exist
in Vanilla and silently renders every diff line identically; and any class in a
`no-restricted-syntax` scope must come from `react-components`.

---

## 4. Type layer

Replace the open-ended part type in `src/lib/types.ts` with a union mirroring the
spec, plus a total normaliser so no component re-checks.

```ts
export type ToolState =
  | { status: 'streaming'; input: string }
  | { status: 'running'; input: Record<string, unknown>; metadata?: Record<string, unknown> }
  | { status: 'completed'; input: Record<string, unknown>; content: ToolContent[]; metadata?: Record<string, unknown> }
  | { status: 'error'; input: Record<string, unknown>; error: { message: string; status?: number }; content?: ToolContent[] }

export type ToolContent =
  | { type: 'text'; text: string }
  | { type: 'file'; uri: string; mime?: string; name?: string }

export interface ToolPart {
  type: 'tool';
  id: string;
  name: string;
  executed?: boolean;
  state?: ToolState;
  time?: { start?: number; end?: number };
}

export interface TextPart { type: 'text'; text: string }
export interface ReasoningPart { type: 'reasoning'; text: string }
export interface CompactionPart { type: 'compaction'; /* running | completed | failed */ }
export interface RetryPart { type: 'retry'; attempt: number; error: { message: string } }

export type MessagePart = TextPart | ToolPart | ReasoningPart | CompactionPart | RetryPart;
```

`state` is optional because the spec's `anyOf` includes tool variants that omit it.
`normaliseToolState(part: ToolPart): ToolState` returns a `{ status: 'running',
input: {} }` fallback so downstream code is total.

```ts
export function hasDetail(state: ToolState): boolean
export function describeResult(state: ToolState): string
```

`hasDetail` must be true for `streaming` even though its input is a string — that is
the case the naive `!!state.input` check gets wrong.

---

## 5. Work items

Ordered; each is independently shippable. Steps 1–6 need no Rust change.

### 1. Types and normaliser
`src/lib/types.ts`, `src/lib/chat/normalise.ts`.
Discriminated union, `normaliseToolState`, `hasDetail`, guards `isToolPart`,
`isTextPart`, `isReasoningPart`.
*Accept:* `tsc` rejects `part.state.content` without narrowing on `status`.

### 2. `describeToolCall`
`src/lib/chat/describeToolCall.ts` — pure, no React, mirrors Harvest's
`tool-render.js`.

```ts
export function describeToolCall(name: string, input: Record<string, unknown>): string
```

Cases for the tools opencode actually exposes here (`read`, `write`, `edit`, `bash`,
`grep`, `glob`, `list`, `patch`). Interpolate the salient argument; truncate commands
at 28 characters as Harvest does; `default:` returns `name.replace(/_/g, ' ')`.

*Accept:* exhaustive table test — every known name yields prose, and an unknown name
never leaks a snake_case identifier.

### 3. `ToolCallStep`
`src/features/sessions/chat/ToolCallStep.tsx`.

- `hasDetail` false → render a plain `div` row, **no** chevron, `tabindex`, or
  `aria-expanded`. Interactive only when there is something to reveal.
- `hasDetail` true → a real `Button` styled as a row. A native button gives Enter and
  Space for free, so no manual keydown handling (Harvest hand-rolls this).
- Label is `describeToolCall`; the raw name appears only in the expanded detail.
- Detail: name tag, then `Input` and `Result` sections. Text result as text; file
  result as a name + mime chip, since a `uri` is not usefully linkable yet.
- `streaming` shows the raw string input, never parsed.
- `error` keeps the step in place, styled `negative`, showing `error.message`.
- CSS as `app-tc-*` in `src/styles/index.scss`, `--vf-*` tokens only.
*Accept:* label per status; no interactive affordance without detail; expanding shows
input and result; `streaming` never renders `[object Object]`.

### 4. `ToolCallGroup`
`src/features/sessions/chat/ToolCallGroup.tsx`. Collapse runs of ≥3 consecutive
same-`name` steps into a native `<details>`, summary `Ran 4 file reads`, with a noun
map and a `replace(/_/g,' ')` fallback. Force-open while any member is `running` so
live work is visible.
*Accept:* three `read` steps group; two do not; a running member opens the group.

### 5. Reasoning and compaction
`src/features/sessions/chat/ThinkingBlock.tsx` — `reasoning` through a Vanilla
`Accordion`, collapsed. `compaction` as a single quiet marker row, visually distinct
from a tool step.
*Accept:* reasoning folded; compaction marker distinguishable.

### 6. Rewire `ChatPanel`
Replace `textOf` and `toolSummary` with one ordered part renderer. **Order is the
point** — text, reasoning, tool, compaction in the order opencode returned them, or
the transcript stops reading as a narrative. Keep the turn header, model and finish
chips, and the inline error block. Keep filtering `idle`.
*Accept:* a turn with text and two tools renders text then two steps, in order.

### 7. Streaming — requires Rust first
Blocked by §2.2. Three parts, in order:

**7a. Stream through the proxy.** Add a dedicated route rather than widening
`forward`:

```rust
.route("/api/oc/event", get(stream_event))
```

Subscribes to the upstream SSE and returns `Sse::new(...)`, which axum streams
incrementally. `forward()` stays as-is for finite bodies. Add it to the allowlist and
regenerate the inventory, or the contract test fails.

**7b. Merge events client-side.** Consume `/api/oc/event`, apply
`message.part.updated`, and drop the `refetchInterval: 4000`. Filter by `sessionID` —
the stream is **global**, not per session, so unfiltered messages will land in the
wrong tab. Tolerate unknown event types by ignoring them.

**7c. Keep polling as fallback.** If 7a or 7b stalls, `refetchInterval` already
produces a correct, if less lively, UI.

*Accept:* a prompt produces steps that update in place with no refetch.

---

## 6. Testing

| Layer | Coverage |
|---|---|
| `normalise` | each status round-trips; missing `state` yields the fallback; `hasDetail` true for `streaming` |
| `describeToolCall` | every tool name; interpolation; 28-char truncation; unknown-name fallback |
| `ToolCallStep` | label per status; non-interactive without detail; expand shows input and result; `streaming` string safe; text and file results; error styling |
| `ToolCallGroup` | three group, two do not; running member opens |
| `ChatPanel` | interleaved order preserved; reasoning folded; `idle` filtered; existing error and empty-state tests still pass |

Run axe over a turn containing a running step, a completed step, an errored step and a
reasoning block — both the interactive and non-interactive paths need checking, since
only one of them is a button.

---

## 7. Risks

1. **No local model emits tool calls.** The configured provider is a stub returning
   fixed text, so the `tool` path cannot be exercised live. Steps 3–6 are verified by
   unit tests against spec-derived types and fixtures, not a real agent run. Teaching
   the stub to emit one tool call would close this; otherwise the gap outlives the
   work.
2. **Event payload shapes are undocumented.** `/api/event` gives `{id, type, data}`
   and heartbeats; the `data` for `message.part.updated` is not in the spec. 7b is
   written against observed output.
3. **The proxy rewrite in 7a is the riskiest item.** `forward()` is exercised by the
   existing forwarding tests; a streaming route must not regress them, and the
   allowlist and inventory must stay in step.
4. **Tool names are open-ended.** opencode's set is larger than Menzi's, so the
   `default` branch of `describeToolCall` is permanent, not a placeholder.
5. **Long transcripts.** Design doc §8.2 wants `@tanstack/react-virtual`. Out of
   scope, but the part renderer should not assume a short list — this is the trigger
   to add it.
6. **No browser attached**, so light/dark and mobile appearance is unverified. The
   tokens are right by construction; that is an argument, not an observation.
