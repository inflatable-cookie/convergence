# 057 Close The Resolve Loop

Status: complete
Updated: 2026-07-25
Roadmap: `g02.016`

## Objective

Make the flagship conflicts-as-data flow finish. Audit P1.1 and P1.2:
`resolve apply` produced a manifest id no verb consumed, and the inbox's
"resolve" recommendation pointed at `fetch`, which cannot resolve
anything.

## Scope of the actual problem

Every piece existed and nothing connected them. Resolution took local
snap ids while superpositions arrive in *bundles*. Applying a resolution
returned an `ObjectId` with no snap, no head, no working tree. The inbox
knew a bundle needed resolving and recommended a verb that could not
accept it. A user who managed to reconstruct the flow by hand then hit a
third wall, which no unit test could see: republishing the resolution
into a still-open window re-superposed it.

## In Scope

- `resolve list|validate|apply` accept a bundle id as well as a snap id,
  fetching the bundle's tree on demand
- `resolve apply` lands the resolution as a snap and checks it out
  (`--no-checkout` to record only, `--force` to overwrite local changes),
  reporting the next verb
- `Workspace::capture_tree` / `adopt_tree`: doc 17 §1's head rule split
  into two operations
- inbox recommendations become runnable argv, owned by the CLI so the
  TUI cannot diverge from what a user is told to paste
- end-to-end test driving the real binary against a real server

## Out Of Scope

- arrival ergonomics (16.2), onboarding (16.3), output polish (16.4)

## Acceptance Criteria

- conflict → inbox → resolve → publish completes with no out-of-band
  knowledge, proven by a test that runs the binary; all suites green

## Outcome

- `resolve_target` resolves a ref to a root manifest: local snap first,
  otherwise a bundle, fetched on demand. Bundles are *why*
  superpositions exist, so refusing them was the dead end
- `resolve apply` now emits `{snap, root_manifest, derived_from_bundle,
  paths_resolved, checked_out, next}` and lands the tree as a snap. The
  bundle is a provenance edge, never a parent (doc 17 §1)
- head-rule split made real: `capture_tree` records without moving head,
  `adopt_tree` materializes then moves it. `--no-checkout` uses the
  first, so head and the working tree never disagree
- **the third wall, found by the e2e test**: a resolution republished
  into an open window re-superposed, because supersession by base
  containment only matched an *exact* value. A publisher whose declared
  base holds a superposition at that path saw every variant and chose;
  doc 17 §2 now says base containment counts variant membership. The
  existing safety condition still carries the weight — the superseder
  has its own explicit opinion at the path — and a publisher who never
  saw the variants is untouched, which the decision-table test pins
- inbox mapping moved from the TUI into `converge_cli::inbox_actions`:
  one source for what the CLI tells you to paste and what the TUI runs
  on Enter. Human `inbox` output now prints `run: converge …` per row
- `resolve` joined the TUI's remote-command set and the resolution view
  is entered from a worker result, because listing a bundle may fetch
  it and the event loop must not block (arch 15 §3)
- `fetch` and `resolve` share `fetch_bundle_tree`, which records the
  fetched bundle as the publish base. Missing that was what made the
  resolution publish declare no knowledge of what it resolved
- 157 tests green, including `resolve_loop_e2e` (real binary, real
  server, conflict through promotable republish)

## Next Task

Batch card 16.2 (arrival ergonomics) — done, card 058.
