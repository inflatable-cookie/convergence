# 060 Output Polish

Status: complete
Updated: 2026-07-25
Roadmap: `g02.016`

## Objective

Audit P3 and P4.20: human output leaked Rust `{:?}` formatting, message
flags differed per verb, bundle-naming differed per verb, `watch` broke
the envelope contract, and a large transfer looked hung.

## Scope of the actual problem

These read as cosmetics and are not. `Ready { promotable: false }` is
printed at exactly the moment a user needs to be told a bundle is
superposed and what to run. `watch`'s stray lines were gated on
`!cli.json`, which is *true* in capture mode — the mode the TUI drives —
so the TUI's own screen was the thing being corrupted. And a
multi-hundred-MiB publish with no output is indistinguishable from a
hang, on the binary-heavy beachhead.

## In Scope

- `describe_status` / `describe_window` / `describe_limit` for the three
  records that rendered with `{:?}`
- `watch` progress lines gated on `OutputMode::Human`
- `-m/--message` on every verb carrying a message; `--notes` kept as an
  alias on `publish` and `release`
- one `bundle_ref` helper: id or `--release <channel>` on `fetch`,
  `bundle`, and `verify`
- transfer progress on stderr, per batch, human mode only

## Out Of Scope

- byte-level progress bars: the client negotiates and then moves 8 MiB
  batches, so finer granularity would be invented detail
- colour and terminal styling (g02.017 owns TUI presentation)

## Acceptance Criteria

- no `{:?}` output in any human-mode verb, `--json` emits exactly one
  envelope line, message and bundle-naming flags are uniform, transfers
  report progress; all suites green

## Outcome

- status now reads "ready to promote" / "ready, blocked by
  superpositions" / "failed: …" — the blocked case names the reason
  rather than printing a struct
- window renders as "publications 3-7" (or "publication 3"), retention
  limits as a number or "keep all"
- the `watch` fix is the sharpest one: the guard was `!cli.json`, so
  capture mode — the TUI — printed to the terminal it was drawing on.
  Gated on `OutputMode::Human` instead, which is what the check always
  meant
- `--notes` survives as a clap alias on `publish` and `release`: the
  rename is about consistency, and breaking a flag people have in their
  shell history buys nothing
- `bundle_ref` gives `fetch`, `bundle`, and `verify` one addressing
  shape. Inspecting what you just fetched no longer means copying a hash
  by hand
- progress is reported by the *client*, through an optional sink, so the
  library still prints nothing on its own; the CLI installs a stderr
  reporter only in human mode. Batch granularity is honest — it is what
  the wire moves in
- pinned by an e2e that publishes a 12 MiB file and asserts progress on
  stderr, prose on stdout, and a single envelope line under `--json`
- 165 tests green

## Next Task

Roadmap `g02.016` is complete. `g02.017` opened at batch card 17.1
(card 061).
