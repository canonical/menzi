# Chat and Tool-Call Display Plan

| | |
|---|---|
| **Status** | Draft for review |
| **Date** | 29-SEP-26 |
| **Scope** | Replace the current comma-joined tool summary in the Code page chat with a structured, readable transcript of agent turns and their tool calls |
| **Reference** | `canonical/harvest` `web-ui/src/components/chat/` and `web-ui/src/lib/tool-render.js` |
| **Data source** | opencode v2.0.6 `GET /openapi.json`, schemas `Session.Message.*` |

---

## Table of contents

1. [Current state](#1-current-state)
2. [What Harvest does](#2-what-harvest-does)
3. [The opencode contract we must render](#3-the-opencode-contract-we-must-render)
4. [Target design](#4-target-design)
5. [Implementation plan](#5-implementation-plan)
6. [Testing](#6-testing)
7. [Risks and open questions](#7-risks-and-open-questions)

---

## 1. Current state

`src/features/sessions/ChatPanel.tsx` renders each turn as a paragraph of text. Tool
calls are collapsed into a single line:

```
build, read, grep
```

Everything the user needs to understand what the agent *did* is lost. `partsOf()`
reads `message.parts ?? message.content`, `textOf()` concatenates every `text` part,
and `toolSummary()` joins the tool names with commas. A failed tool is
indistinguishable from a successful one, and the input and output are never shown.

---

## 2. What Harvest does

Harvest's chat is built from three small components, and the design is worth copying
almost exactly. The key insight is that **the label is prose, not a tool name**.

`ToolCallStep.vue` — one collapsible row per tool call:

- a status icon, spinning while `status === 'running'`
- a **human sentence** as the label, e.g. `Searching for "parseConfig" functions`
- a chevron, present only when there is detail to show
- expanded detail containing the raw tool name, the **Input** JSON, and a **Result**
  preview

Two details worth copying precisely:

1. **A collapsible row is only interactive when it has content.**
   `hasDetail = !!(step.input || step.preview)` drives the chevron, `role="button"`,
   `tabindex`, and `aria-expanded` together. A tool call with nothing to reveal is
   plain text, not a dead button.
2. **Keyboard support is first-class.** `keydown.enter` and `keydown.space` both
   toggle, with `preventDefault`.

`ToolCallGroup.vue` — consecutive steps of the same kind collapse into a native
`<details>` whose summary reads `Ran 4 graph queries`, using a noun map so the
summary is grammatical.

`ThinkingBlock.vue` — reasoning is folded away rather than shown inline.

`lib/tool-render.js` — `describeToolCall(name, input)` is a pure function returning
prose per tool, with a `default` of `name.replace(/_/g, ' ')`. Long arguments are
truncated: `command.slice(0, 28).trimEnd() + '…'`.

Harvest's README states the intent: *"each tool invocation appears as a collapsible
step with an AI-generated plain-English description, tool name, raw inputs, and
result preview."*

---

## 3. The opencode contract we must render

Verified against `GET /openapi.json` on opencode v2.0.6, not from memory. An
assistant message carries a `content` array whose entries are one of:

| Part type | Fields |
|---|---|
| `text` | `text`, `state` |
| `reasoning` | `text`, `state`, `time` |
| `tool` | `id`, `name`, `executed`, `state`, `time` |
| `compaction` | running / completed / failed variants |
| `retry` | `attempt`, `at`, `error` |

A `tool` part's `state` is a discriminated union, and **this is the field the UI keys
off**:

| `state.status` | Shape |
|---|---|
| `streaming` | `input: string` (partial, not yet parsed) |
| `running` | `input: object`, `metadata: object` |
| `completed` | `input: object`, `content: ToolContent[]`, `metadata` |
| `error` | `input: object`, `error: { message, status }`, `content?: ToolContent[]` |

`ToolContent` is `Tool.TextContent { text }` or `Tool.FileContent { uri, mime, name }`.

Consequences the plan must respect:

- Tool state is a **union**, not a flat object. `input` is a `string` in `streaming`
  and an `object` everywhere else. A renderer that assumes `object` will render
  `"[object Object]"` mid-stream.
- `error` is a *status*, not a boolean. There is no `failed: true`.
- A tool part with no `state` is legal (`state` is required by the schema, but
  `anyOf` includes variants without it), so the UI needs a total function.
- `reasoning` and `compaction` are first-class parts and must not be rendered as
  tool calls.

---

## 4. Target design

Each assistant turn renders as:

```
┌─────────────────────────────────────────────┐
│ Agent · build · stub/mini            [stop] │
│                                              │
│ Text answer, markdown rendered.              │
│                                              │
│ ▸ Searching for "parseConfig" functions     │  ← collapsible, prose label
│ ▸ Running "cargo test" on agent-1           │
│ ▸ Reading source of Config::load      (err) │
│   ▾ expanded:                                │
│      read                                     │
│      Input   {"path":"src/config.rs"}        │
│      Result  file not found                  │
│                                              │
│ ▸ Thinking                             [▸]  │  ← folded reasoning
└─────────────────────────────────────────────┘
```

Rules:

1. **Labels are prose.** `describeToolCall()` returns a sentence. The raw tool name
   appears only in the expanded detail, as a tag.
2. **Interactivity is conditional.** No chevron, no `role`, no `tabindex` unless the
   step has input or a result preview.
3. **Status is visible before the label.** Icon plus colour, not colour alone.
4. **Errors stay in place.** A failed step is not hoisted out of the sequence; it is
   styled and keeps its input so the user can see what was attempted.
5. **Empty states are honest.** A part with no recognisable type renders nothing
   rather than a blank row.

---

## 5. Implementation plan

Ordered so each step is shippable and testable on its own.

### Step 1 — Types for the real contract

Replace the loose `OpencodeMessagePart` index signature with a discriminated union
mirroring the OpenAPI schemas: `TextPart`, `ReasoningPart`, `ToolPart`, `CompactionPart`,
`RetryPart`, and `ToolState = Streaming | Running | Completed | Errored`.

Add narrow type guards (`isToolPart`, `isTextPart`, …) and a
`normaliseToolState(part)` that always returns a total value, so no component has to
defensively re-check.

**Done when:** `tsc` rejects any access to `tool.state.content` without narrowing.

### Step 2 — `describeToolCall()`

A pure function, `src/lib/chat/describeToolCall.ts`, mirroring Harvest's
`tool-render.js`. Signature:

```ts
describeToolCall(name: string, input: Record<string, unknown>): string
```

- Cases for the opencode tools Menzi actually exposes: `read`, `write`, `edit`,
  `bash`/`shell`, `grep`, `glob`, `list`, `patch`, plus the session tools.
- Interpolate the salient argument (path, pattern, command).
- Truncate commands at 28 characters, as Harvest does.
- `default:` returns `name.replace(/_/g, ' ')`, so an unknown tool still reads as
  words rather than leaking a snake_case identifier.

This is a pure function with no React dependency, so it is exhaustively unit-testable.

**Done when:** every tool name in the opencode spec produces a non-empty, non-raw
string.

### Step 3 — `ToolCallStep` component

`src/features/sessions/chat/ToolCallStep.tsx`.

- Props: `{ part: ToolPart }`.
- `hasDetail` computed from the normalised state, not from a truthy object, so
  `streaming` with a string still counts.
- The row is a `<button>` **only** when `hasDetail`; otherwise a `<div>`. Using a real
  button rather than `role="button"` on a div gives Enter and Space for free and
  removes the manual keydown handling Harvest has to write.
- `aria-expanded` bound to the open state.
- Expanded detail: raw name as a tag, then `Input` and `Result` sections.
- Result rendering: `Tool.TextContent` as text; `Tool.FileContent` as a file chip
  with name and mime, since a URI is not something we can usefully link yet.
- Long JSON is collapsed past a line threshold with a "show more" control.
- Styles in `src/styles/index.scss` using only `--vf-*` tokens, so it works in light
  and dark. Prefix `app-tc-*`.

**Done when:** a completed tool renders its label, and expanding shows name, input
and result.

### Step 4 — `ToolCallGroup`

`src/features/sessions/chat/ToolCallGroup.tsx`. Collapse runs of ≥3 consecutive tool
parts of the same `name` into a native `<details>` with a summary like
`Ran 4 file reads`. A `GROUP_NOUNS` map provides the noun; fall back to
`name.replace(/_/g, ' ')`. Open by default while any member is `running`, so live work
is visible without interaction.

**Done when:** three consecutive `read` calls render as one group.

### Step 5 — Reasoning and compaction

`src/features/sessions/chat/ThinkingBlock.tsx`. Render `reasoning` parts through a
Vanilla `Accordion`, collapsed by default, labelled `Thinking`. `compaction` parts
render as a single quiet marker row — `Compacting context`, `Context compacted`, or the
failure message — never as a tool step.

**Done when:** a reasoning part is present but folded, and a compaction marker is
visually distinct from a tool call.

### Step 6 — Rewire `ChatPanel`

Replace `textOf()` and `toolSummary()` with a single ordered part renderer that
preserves the original sequence: text, reasoning, tool, compaction, in the order
opencode returned them. Order matters — interleaving is what makes a transcript
readable.

Keep the existing per-turn header, model and finish chips, and the inline error block.
`idle` messages stay filtered out.

**Done when:** a turn with text and two tools renders text then two steps, in order.

### Step 7 — Streaming

Consume `GET /api/event` (`message.part.updated` and friends) and merge parts into
the message list, replacing the 4-second `refetchInterval`. A `running` step then
animates in place instead of appearing a poll later.

This is the largest single piece and the one most likely to need iteration against the
real event shapes, which are not yet documented in the OpenAPI spec. It is
deliberately last: steps 1–6 are worth shipping even if streaming is deferred, and
`refetchInterval` remains a correct fallback.

**Done when:** a prompt produces steps that update live without a refetch.

---

## 6. Testing

| Layer | What is covered |
|---|---|
| `describeToolCall` | Every tool name; argument interpolation; the 28-character truncation; unknown names fall back to words |
| Type guards | Each guard accepts its own part and rejects the others; `normaliseToolState` is total, including a part with no state |
| `ToolCallStep` | Label renders for each status; chevron and `aria-expanded` absent when there is no detail; expanding shows input and result; a `streaming` string input does not render `[object Object]`; a text result and a file result both render |
| `ToolCallGroup` | Three same-name steps group; two do not; a running member opens the group |
| `ChatPanel` | Interleaved order preserved; reasoning folded; `idle` filtered; existing tests for error surfacing and the empty state still pass |

Accessibility: axe on a turn containing a running step, a completed step, an errored
step and a reasoning block. The conditional-button rule means both the interactive and
non-interactive paths need covering.

The `p-code-snippet` classes used by the current diff viewer do not exist in Vanilla
and were already found to render add/remove lines identically; the new components must
use the real `app-*` classes and be visually checked in both themes. I have no browser
attached, so that check is currently impossible and is called out in §7.

---

## 7. Risks and open questions

1. **No model in this environment emits tool calls.** The configured provider is a
   local stub that returns fixed text, so the `tool` part path cannot be exercised
   end-to-end. Until a real provider is configured, this work is verified by unit
   tests against the OpenAPI-derived types and by hand-built fixtures, not by a live
   agent run. The stub should grow a mode that emits a tool call, otherwise this gap
   persists.
2. **Event-stream shapes are undocumented.** `/api/event` emits
   `{ id, type, data }` and heartbeats, but the `data` payload for
   `message.part.updated` is not in the OpenAPI spec. Step 7 must be written against
   observed output, and the reducer should tolerate unknown event types by ignoring
   them.
3. **`ToolState.Streaming.input` is a string.** Mid-flight a tool's input is partial
   text, not parsed JSON. Attempting to parse it will throw. Show it raw, or omit it,
   while `status === 'streaming'`.
4. **The stream is a single global channel**, not per session. Messages from other
   sessions will arrive; filter on session id before merging.
5. **Tool names are open-ended.** Menzi will add tools, and opencode's set is larger
   than Menzi's. The `default` branch of `describeToolCall` is therefore a permanent
   requirement, not a placeholder.
6. **Long transcripts need virtualisation.** Design doc §8.2 calls for
   `@tanstack/react-virtual`. Out of scope here, but the part renderer should not
   assume a small list, and this is the natural trigger to add it.
7. **No browser is attached**, so the light/dark and mobile appearance of the new
   components is unverified. The tokens are correct by construction, but that is an
   argument, not an observation.
