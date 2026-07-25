# 016 Workflow Completion

Status: in progress (16.1-16.3 complete)
Owner: repo maintainers
Updated: 2026-07-25

## Context

The UX audit found dead ends at exactly the moments users most need
guidance: `resolve apply` produces a manifest id no verb consumes, the
inbox's "resolve" recommendation routes somewhere that cannot accept
it, `sync pull` needs an undiscoverable manual restore, and there is no
end-to-end path for a second teammate to join a repo. The flagship
conflicts-as-data flow must complete without reading source code.

## Findings Addressed

- P1.1: `resolve apply` yields an orphan manifest id — no verb snaps,
  publishes, or materializes the result
- P1.2: inbox "resolve" recommendation maps to `fetch`, but resolution
  only accepts local snap ids — dead end
- P1.3: `sync pull` requires manual `restore --force` nobody is told
  about
- P1.4: `fetch` without `--into` is invisible; no "checkout bundle and
  continue" verb
- P1.6: no bootstrap/member-management verbs — team onboarding
  impossible end-to-end
- P3: `{:?}` Debug leaks in CLI human output; message/note flag
  inconsistency (`snap -m` / `annotate` positional / `--notes`);
  `fetch` vs `bundle`/`verify` bundle-id arity inconsistency;
  `watch --json` breaks the envelope contract
- P4.18: no `show <snap>` contents browser (History Enter is
  destructive restore)
- P4.19: no undo (`unsnap` named in the UX spec, absent)
- P4.20: no transfer progress for large binaries — the beachhead's
  most-felt gap

## Execution Plan (batch details in cards)

- **16.1 Close the resolve loop** (complete, card 057): `resolve`
  accepts bundle ids and fetches on demand; `resolve apply` lands the
  resolution as a snap and checks it out, naming the next verb; inbox
  actions moved into `converge_cli::inbox_actions` so CLI text and TUI
  Enter run the same command. The e2e test found a third wall behind the
  two the audit named — a resolution republished into an open window
  re-superposed — fixed by counting variant membership as base
  containment (doc 17 §2)
- **16.2 Arrival ergonomics** (complete, card 058): `sync pull
  --materialize`; `fetch --checkout` lands a bundle as a snap to
  continue from while `--into` keeps meaning "copy elsewhere";
  `show <snap|bundle> [--path]` browses read-only and renders superposed
  paths as such; `unsnap` undoes the capture and leaves the work,
  refusing on non-leaf or published snaps. Both arrival verbs now name
  the next command instead of printing an id
- **16.3 Team onboarding** (complete, card 059): tokens persisted
  hashed with runtime issuance, `--bootstrap-admin` for the first admin,
  site admin as a `*`-repo grant, `converge repo create` /
  `member add|list`, and `docs/guides/001-two-user-quickstart.md` kept
  honest by `onboarding_e2e.rs`
- **16.4 Output polish**: Debug-format leaks removed; unified
  `-m/--message` across verbs; `watch --json` envelope compliance;
  transfer progress reporting on push/pull/fetch for chunked blobs

## Exit Criteria

- scripted two-user walkthrough (init → snap → publish → conflict →
  inbox → resolve → promote → release → fetch) completes with no
  out-of-band knowledge
- every inbox recommendation is copy-paste runnable
- no `{:?}` output in any human-mode verb

## Next Task

Open batch card 16.4 (output polish).
