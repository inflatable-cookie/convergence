# 016 Workflow Completion

Status: planned
Owner: repo maintainers
Updated: 2026-07-24

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

- **16.1 Close the resolve loop**: `resolve apply` lands as a snap
  (lineage-correct, bundle-derived) with materialize-or-publish
  follow-through; inbox recommendations emit runnable commands that
  actually run end-to-end; superposed-bundle fetch flows into
  resolution
- **16.2 Arrival ergonomics**: `sync pull --materialize` (safe-restore
  path from g02.012); `fetch --into` default behavior rationalized;
  `show <snap|bundle>` read-only browser; `unsnap` undo per UX spec
- **16.3 Team onboarding**: server bootstrap verb (first admin, repo
  create, grant issue), member add/list from the CLI, documented
  two-user quickstart that works start to finish
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

Blocked behind g02.013 completion (parallelizable with g02.014/015 if
operator chooses).
