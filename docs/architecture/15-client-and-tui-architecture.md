# 15 Client and TUI Architecture

Status: active
Updated: 2026-07-24
Roadmap: `g02.002` Batch 2.4, `g02.015` Batch 15.3

Client side of the rebuild. UX authority:
`docs/rebuild/002-tui-ux-spec.md`.

## 1. Layering

```
converge-tui  ── argv contract ──▶ converge-cli ──▶ converge-client ──▶ converge-model
```

- **`converge-client`** (lib): workspace scan/snap, local content-addressed
  store (salvaged, plus sharded fanout), diff, superposition resolution,
  remote sync client. No terminal concerns.
- **`converge-cli`** (`converge`): the canonical verb surface — `snap`,
  `publish`, `promote`, `release`, plus workspace/remote verbs. Every command
  has a stable argv shape and a `--json` output mode. The CLI is the semantic
  contract; anything a front-end can do must be expressible here first.
- **`converge-tui`**: thin front-end per the UX spec — wizards assemble argv
  and call the CLI command layer in-process. Preserved from g01 by design:
  TUI and CLI cannot diverge (UX spec §4.1).

## 2. Preserved UX principles (from the spec)

- Command-console shell: persistent input + fuzzy palette, single screen,
  view stack (UX spec §1-3).
- One state-computed primary action per screen; `Enter` on empty input runs
  it (§4.2).
- Agent trace: JSONL semantic events (screen views with selectable items and
  primary CTA, canonical actions, classified errors) — machine-drivable TUI
  as a first-class requirement (§4.3).
- Destructive actions confirm once (§4.5); workflow-profile-aware
  recommendations (§4.6).

## 3. Required fixes (UX spec §7)

- **Async remote operations.** g01 froze the UI during remote calls. The
  rebuilt client runs remote work on a task pool; the TUI event loop never
  blocks; in-flight operations render progress in the status strip.
- Wizards gain back-one-step and a review step before execution.
- Structured option prompts — unrecognized input is an error, never silently
  swallowed.
- Quit is explicit; stray `Esc` at root does not exit.
- Local/Remote context labeled in the prompt, not color-only.
- Consistent contextual key layer alongside the console (direct view-jump
  keys), keeping the superposition `Alt+N` pattern as the template.

## 4. Session (front-end refresh economics)

The argv contract says a front-end may only speak in CLI verbs; it does
not say each verb must rediscover the world. `converge_cli::Session` is
the per-process state a long-lived front-end holds across commands, and
`execute_in(&session, argv)` is the same code path `execute` runs:

- workspace handle, discovered once and keyed by the cwd it came from
- working-tree manifest scan, keyed by a **dirstamp** — a metadata-only
  walk (name, kind, mode, size, mtime) over exactly the paths the scan
  would read. An idle refresh stats the tree instead of hashing it
- remote HTTP client (connection pool), keyed by base url + token

Every entry self-invalidates: the stamp moves when the tree moves, and
the client key moves when `login` rewrites the remote. No verb has to
remember to flush a cache.

The stamp's blind spot is deliberate and bounded: a write landing within
one mtime tick that leaves the size identical is invisible to it. That
is tolerable only because the stamp gates a cache whose miss path is the
real scan, and because the capture paths — `snap`, `watch` — never read
it. They always rescan.

The TUI holds one session for its lifetime, shared with its worker
threads. Event arrival refreshes the inbox as well as status: remote
events are precisely the thing that changes what is waiting on you.

## 5. Local capture path

The validated theory stays: snaps are cheap, automatic-capable, offline, and
carry no quality gate. Quality enters at `publish` (server-side gate policy,
doc 14). Snap capture, manifest scan, and materialize are salvaged modules;
chunking is replaced per doc 16.

## Next Task

First rebuild implementation roadmap: `converge-client` + `converge-cli`
vertical slice (init → snap → publish against the doc-14 server slice), TUI
after the CLI surface stabilizes.
