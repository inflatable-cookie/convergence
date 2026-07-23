# 002 TUI UX Specification

Status: capture artifact
Updated: 2026-07-23
Roadmap: `g02.002` Batch 2.1

Implementation-independent specification of the g01-era TUI UX. The rebuilt
TUI targets this spec; the archived implementation is evidence, not authority.
Section 7 lists deliberate improvements the rebuild should make.

## 1. Shell Model

The TUI is a **command-console hybrid on one persistent screen**, not a page
router. Fixed vertical layout, five regions:

1. header bar (workspace/repo context, identity state)
2. active view body (swappable)
3. "Last" result strip — echoes last command and its result, color-coded
   (output white, error red, command cyan) with timestamp; the primary
   feedback channel
4. suggestions palette (0 or 9 rows, fuzzy-filtered)
5. command input line with left hint (primary action commands) and right hint
   (global key legend)

## 2. Views

Every view has a mode, title, refresh timestamp, and vertical list navigation.
View chrome: bordered block titled `Title <timestamp>`.

| View | Prompt | Purpose |
| --- | --- | --- |
| Root — Local | `root>` | Local dashboard: working-tree change summary, latest snap, sync/publish state |
| Root — Remote | `root>` (accent color) | Remote dashboard: `repo=/scope=/gate=` header, identity, inbox/bundle/release counts, gate promotion state, ranked recommended next actions with owner labels |
| History | `history>` | Snap list + pending-changes header, head marker, per-snap detail |
| Inbox | `inbox>` | Remote publications awaiting triage; snaps missing locally |
| Bundles | `bundles>` | Gate outputs: promotable/blocked/pinned; gateway to resolution, approve, promote, release |
| Releases | `releases>` | Per-channel latest bundle + timestamp |
| Lanes | `lanes>` | Per-user lane heads, last-synced |
| Superpositions | `supers>` | Conflict workspace: path list (65%) + variant detail (35%), live validation counts |
| Gate Graph | `gates>` | Gate DAG: upstreams, required approvals |
| Settings | `settings>` | Config as selectable action list |

Root splits into **Local and Remote contexts** with distinct command sets,
default actions, and accent colors. The same color language repeats in prompt,
input line, and Tab hint.

## 3. Navigation and Keys

Key meaning depends on whether the input buffer is empty:

- `q` quit
- `Esc` layered back: clear input → pop view stack toward Root → quit
- `Tab` empty input: toggle Local↔Remote; with suggestions open: accept
  selected suggestion
- `Enter` empty input: run the computed **default action**; with input: run
  typed/selected command
- `↑/↓` empty input: move view-list selection; suggestions open: move
  suggestion selection; else: command history
- `←/→` empty input: rotate ambient hint; else: cursor movement
- printable char: start command entry, live fuzzy suggestions
- Superpositions only: `Alt+1..9` pick variant, `Alt+0` clear decision,
  `Alt+f` next invalid, `Alt+n` next missing

Views are entered by command (`history`, `inbox`, `bundles`, …), pushed onto a
mode stack; `Esc` pops. Commands **auto-cross the Local/Remote boundary**: a
remote command typed in Local context switches context with a status note —
users never hit "wrong mode for this command."

## 4. Core Interaction Principles (must preserve)

1. **TUI is a thin front-end over the CLI's argv contract.** Wizards and
   actions assemble a CLI-style argv and call the same command implementations
   the CLI uses. TUI and CLI cannot diverge in behavior.
2. **One primary action per screen, computed from state.** Changes → snap;
   unsynced → sync; unpublished → publish; else history. Remote: login/
   bootstrap → create-repo → inbox triage. `Enter` on empty input runs it:
   a zero-typing happy path with the full command set still available.
3. **Machine-drivable and observable by design.** Optional JSONL agent trace:
   `session_start`, `screen_view` (screen id, selectable items, focused
   element, primary CTA), `user_action` (canonical + raw), `state_change`,
   `validation_error` / `system_error`, `session_end` with stats. Screen views
   dedupe by signature — the trace records semantic transitions, not frames.
   Every screen advertises its selectable items and single primary CTA.
4. **Command console + fuzzy palette as the universal entry point.**
   Persistent input, live suggestions with count and scrolling selection.
5. **Destructive actions confirm once.** Restore/revert/unsnap, promote,
   release, purge, settings resets open a Confirm modal ("Run / Where / This
   action changes data. Enter: confirm, Esc: cancel"); confirmed actions do
   not re-prompt within the flow.
6. **Workflow-profile awareness.** Profiles (e.g. DAW / GameAssets / Software)
   reorder recommendations and rename domain terms (release = "mastered
   mixdown" / "build-ready pack" / "channel output") with human role owners.
7. **Ambient guidance.** Left hint always shows the primary-action command;
   right hint shows the global key legend; remote dashboard shows ranked
   recommended next steps with counts and navigation targets
   (`[bundles -> superpositions]`).
8. **Idle auto-refresh.** Local dashboard refreshes ~3s when idle (no modal,
   empty input).

## 5. Flows

Wizard pattern: sequence of single-field text-input modals, each pre-filled
with a sensible default; `Enter` advances, `Esc` cancels the wizard. On finish,
assemble argv → shared command impl. Background dims while a modal owns keys.

- **Login**: url → token → repo → scope → gate
- **Bootstrap** (server first-run): url → bootstrap-token → handle →
  display-name → repo → scope → gate
- **Publish**: start prompt ("Enter = publish latest now" / `edit` to
  customize) → snap → scope → gate → metadata-only; defaults surfaced inline
- **Sync**: lane → client → snap
- **Fetch**: kind (snap/bundle/release/lane) → id → user → options
- **Release / Promote / Pin / Approve**: bundle id → target/notes/action
- **Member / Lane-member**: action → handle → role/lane
- **Move/rename**: from → to (case-safe, glob-aware)
- **Gate-graph edits**: add gate (id → name → upstream), edit upstream, set
  approvals

**Superposition resolution** is non-modal and in-view: navigate conflicted
paths, assign variants (`Alt+N`), jump to next unresolved/invalid, watch live
validation counts (missing/invalid/out-of-range), then apply.

## 6. Layout Patterns

- List + detail split (Superpositions 65/35); most views are a bordered list
  with a title-line summary
- Modal kinds: Viewer, editable SnapMessage, ConfirmAction, TextInput
- View stack push/pop like a nav stack
- Two-color context theming for Local vs Remote

## 7. Known Warts — Fix in Rebuild

- **Blocking remote calls freeze the UI** with no progress indication. Rebuild
  needs async remote operations with progress feedback.
- **No direct view-jump keys**; everything routes through command entry. The
  `Alt+N` superposition keys are the only rich in-view keys — inconsistent.
  Rebuild: consistent contextual key layer alongside the console.
- **Wizards are strictly linear** — no back-one-step or review; mid-flow edit
  means full restart. Rebuild: step-back and a final review step.
- **Free-text option parsing** in single fields (fetch options, move globs);
  typos silently ignored. Rebuild: structured option prompts; never swallow
  unrecognized input.
- **Esc overload**: stray Esc on empty input at Root quits the app. Rebuild:
  quit needs its own confirmation or distinct key.
- **Dual home dashboards** distinguished only by accent color behind one
  `root>` prompt — context easy to lose. Rebuild: explicit context label in
  the prompt/header.
- Internal log pane is dead weight (superseded by the "Last" strip) — drop.

## Next Task

Feed this spec into Batch 2.4 client architecture; rebuilt TUI implements
sections 1-6 and resolves section 7.
