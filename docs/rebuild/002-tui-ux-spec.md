# 002 TUI UX Specification

Status: capture artifact + implementation status (§8)
Updated: 2026-07-25
Roadmap: `g02.002` Batch 2.1; reconciled with the implementation in
`g02.017` Batch 17.4

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

Root was originally specified as **two contexts**, Local and Remote, with
distinct command sets, default actions, and accent colors. Batch 23.1
removed the split: see §7's wart and §8. There is one Root, showing head
and pending changes alongside the remote target and what was last
published and last seen.

## 3. Navigation and Keys

Key meaning depends on whether the input buffer is empty:

- `q` quit
- `Esc` layered back: clear input → pop view stack toward Root → quit
- `Tab` accept the selected suggestion. It used to also toggle
  Local↔Remote on empty input, which made one key mean two things
  depending on state (batch 23.1)
- `Enter` empty input: run the computed **default action**; with input: run
  typed/selected command
- `↑/↓` empty input: move view-list selection; suggestions open: move
  suggestion selection; else: command history
- `←/→` empty input: rotate ambient hint; else: cursor movement
- printable char: start command entry, live fuzzy suggestions
- Superpositions only: `Alt+1..9` pick variant, `Alt+0` clear decision,
  `Alt+f` next invalid, `Alt+n` next missing

Views are entered by command (`history`, `inbox`, `bundles`, …), pushed onto a
mode stack; `Esc` pops. Every verb runs from every screen: the
auto-crossing rule existed so users never hit "wrong mode for this
command", and with the mode gone there is no wrong mode to be in.

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
- ~~Two-color context theming for Local vs Remote~~ (removed, batch 23.1)

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
  the prompt/header. **Batch 23.1 went further and deleted the split.**
  Driving the real TUI showed both dashboards using four lines of a
  thirty-line pane, each withholding what the other one showed: the
  local one knew the head, the remote one knew where it published to,
  and a person needed both at once. Labelling a mode is a smaller fix
  than not having one.
- Internal log pane is dead weight (superseded by the "Last" strip) — drop.

## 8. Implementation Status (batch 17.4)

Where the rebuilt TUI stands against sections 1-7. This section is the
reconciliation the spec was missing: a UX contract nobody has measured
against the build is a wish list.

### Built

- shell model (§1); the "Last" strip renders fields rather than raw JSON
  (batch 17.3)
- views (§2): Root, History, Inbox, Bundles, Releases, Lanes, Gate
  Graph, Superpositions, Secrets (batch 23.2, not in the original spec:
  the substrate postdates it)
- keys (§3): `q`, layered `Esc` with quit confirmation, `Tab` suggestion
  accept, `Enter` primary action, `↑/↓` list and history, `←/→`/Home/End
  caret movement, `Alt+1..9`/`Alt+0` variant picks, `Alt+f` next
  invalid, `Alt+n` next missing, plus `Alt` jumps to each view. In-view
  keys: History `m`/`d`, Bundles `p`/`e` (batch 23.3), Secrets `r`/`u`
  (batch 23.2)
- layout (§6): the Superpositions 65/35 list+detail split, with a
  bounded preview per variant (batch 23.5)
- wizards (§5): Login, Publish, Annotate, plus Member, Fetch, Release
  and Promote (batch 23.3). A wizard's review step **is** the
  confirmation for a verb the console would confirm, so its legend names
  the consequence rather than saying "run" — before 23.3 no wizard drove
  such a verb, so that path was untested rather than correct
- principles (§4): argv contract (1), computed primary action (2), JSONL
  agent trace (3), console + fuzzy palette (4), confirm-once (5),
  ambient hints (7), idle auto-refresh (8, at 5s not 3s — the scan is
  dirstamp-gated, and a slower tick is inaudible)
- flows (§5): Login, Publish, Annotate wizards with back-one-step and a
  review step; non-modal superposition resolution with live validation
- warts (§7): all seven addressed — async loads with an in-flight
  indicator, contextual key layer, wizard step-back and review,
  structured option prompts, `Esc`-quit confirmation, one Root instead
  of a labelled mode (batch 23.1), no log pane

### Secrets view (batch 23.2)

Loaded from `secret audit`, so the screen answers "who can read this and
what has gone stale" rather than "these exist". It shows state and hands
commands over; it does not mutate.

That is a constraint, not a scoping choice. **Any verb that opens the
caller's private key cannot run from this program**: unlocking prompts
for a passphrase, and the prompt writes straight to the tty the TUI
holds in raw mode — it lands on top of the drawn screen and then
competes with the event loop for the keystrokes meant to answer it.
Driving the real binary is what found this; pressing `u` on a stale
recipient printed `passphrase:` across the header and hung. `secret
get`, `set`, `rotate`, `share`, `unshare`, `write-env`, `key init`,
`key rotate` and `run` are therefore handed over as a command to run in
a terminal, unless `CONVERGE_PASSPHRASE` is set, in which case nothing
prompts and they run normally. `secret list` and `secret audit` read
metadata the server already holds in the clear, which is why the screen
can exist at all.

A second rule sits on top for values specifically: a secret value must
never enter the input buffer even when a passphrase *is* available. The
buffer is echoed, submitted lines are pushed into `command_history`, and
`↑` replays them, so typing a credential would persist it in three
places at once. `secret rotate` is handed over unconditionally.

### Known limits (batch 23.1, from a real session)

- **the `Alt` jump layer does nothing on stock macOS terminals.**
  Terminal.app and iTerm send composed characters for Option unless "Use
  Option as Meta key" is enabled, so the whole shortcut layer silently
  fails for the platform most likely to be running this. Typing the verb
  still reaches every view. Help now says so. Not fixed by inventing a
  second key scheme in a subtraction batch
- ~~**Superpositions asks for a decision it does not show.**~~ Fixed in
  batch 23.5: the 65/35 split with a bounded per-variant preview
- **a bundle id is only ever printed once.** `publish` prints it; the
  inbox lists a bundle only when it needs *your* action, so one that is
  immediately ready never appears. `events` is the only place to find it
  again, and it is documented as "hints; reconcile via inbox"

### Intentionally different

- **Settings → Help.** The spec's Settings view is "config as selectable
  action list". Configuration is edited by verbs (`login`, `retention`,
  `profile`, `scope`, `member`), and a second editing surface would be a
  place for the two to disagree. The Help view shows the same
  information read-only, plus the key map and verb list
- **Idle refresh at 5s**, not ~3s. The event poller already runs at 3s
  and is the thing that notices remote change; the local tick only has
  to beat human patience
- **Progress feedback is per batch**, not a continuous bar (batch 16.4):
  the client negotiates and then moves 8 MiB batches, so finer
  granularity would be invented detail

### Deferred, with triggers

- **Workflow-profile term renaming (§4.6).** Profiles exist, are
  settable (`converge profile --set`), and shape *guidance* — the flow
  and release hints shown on the remote dashboard and in Help. Renaming
  domain nouns across every surface (release → "mastered mixdown") is
  not built: it multiplies every string by three profiles and would have
  to cover the CLI too, or the two front-ends would speak different
  languages. Trigger: a design partner in one of the non-software
  profiles asking for it by name
- **Remaining wizards (§5)**: Bootstrap, Sync, Move/rename, Gate-graph
  edits. Member, Fetch, Release and Promote were built in batch 23.3 —
  the flag-heavy four, which is the trigger the deferral named. The rest
  are reachable from the console and stay deferred on the same terms:
  observed use where the flag surface is the obstacle, one at a time.

  Two of the built four exist to stop a specific mistake rather than to
  save typing. Fetch asks *where it lands* as one question with three
  answers, because `--checkout` and `--into` are mutually exclusive and
  mean different things (batch 16.2) — a flag list invites giving both.
  Member turns a repeating `--capability` into one field, because the
  alternative is four yes/no questions.

  **No wizard may collect a secret.** Batch 23.2 established that a
  value must never enter the input buffer, and batch 23.3 found that an
  access token had been doing exactly that: the Login wizard echoed one
  while typing and again at review. Credential fields are masked, and
  argv is redacted where it is displayed or traced
- ~~**Ranked recommendations with owner labels on the dashboard
  (§4.7)**~~ — built in batch 23.4. The deferral was right that it
  needed a ranking rule rather than "what the inbox said". The rule is
  **what blocks other people, first**: a superposed bundle stops its
  gate window for everyone, an approval holds up one publisher, lane
  work blocks nobody, and a publication is news. It lives in
  `converge_cli::inbox_actions`, which sorts before returning, so the
  Inbox view and the dashboard read the same order by construction
  rather than by agreement — a second traversal would have been a
  second rule waiting to drift
- ~~**List + detail 65/35 split for Superpositions (§6)**~~ — built in
  batch 23.5. The stated trigger was "variants carrying more than a
  source and a size", which read as a nice-to-have. Driving the flat
  list in batch 23.1 showed the deferral had mis-scoped itself: the
  screen was asking someone to choose between two file contents and
  showing neither, which is a decision-correctness problem, not polish.
  `resolve list --preview` returns a bounded look at each variant, the
  detail pane renders it, and "binary, 4.1 MB" is a legitimate answer —
  two variants labelled only "binary" are not a choice

## Next Task

Roadmap `g02.017` is complete. Remaining audit-hardening work:
`g02.018` adversarial test hardening.
