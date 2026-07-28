# 001 Two-User Quickstart

Status: active
Updated: 2026-07-25
Roadmap: `g02.016` Batch 16.3

Setting up a server, a repo, and a second person, end to end. Every
command here is exercised by `converge-cli/tests/onboarding_e2e.rs`, so
this file cannot drift from what the binaries do without a test failing.

## 1. Start the server and mint the first admin

```bash
converge-server --addr 127.0.0.1:8080 --data-dir ./converge-data \
  --bootstrap-admin root
```

The bootstrap prints the admin token **once**:

```
admin root token (shown once): 9f1c…
```

The server stores only its hash, so there is no way to print it again.
Restarting with the same flag does not issue a second token — a restart
that sprayed fresh credentials into the logs would be worse than losing
one. To recover, delete that subject's rows from `tokens` and restart.

## 2. Create a repo

`login` writes local config and talks to nobody, so naming a repo that
does not exist yet is the normal path:

```bash
mkdir acme && cd acme
converge init
converge login --url http://127.0.0.1:8080 --token 9f1c… \
  --repo acme --scope default --gate intake
converge repo create
```

That creates the repo with a `default` scope and an `intake` gate, and
grants the creator everything in it. A repo without a gate could not
accept a publish, so it is not left as a second setup step.

## 3. Do some work

```bash
echo "the plan" > plan.md
converge snap -m plan
converge publish
```

## 4. Onboard a teammate

```bash
converge member add dana --capability read --capability publish --issue-token
```

Prints their token once, plus the exact `converge login` line for them to
run. Capabilities are `read`, `snap-sync`, `publish`, `resolve`,
`approve`, `promote`, `release`, `admin`; unknown ones are refused rather
than stored as rows that grant nothing. Default when `--capability` is
omitted: `read`, `publish`, `resolve`.

Scope-limited grants use a pattern:

```bash
converge member add contractor --capability read --scope-pattern 'client-a/*'
```

## 5. The teammate joins

```bash
mkdir dana-work && cd dana-work
converge init
converge login --url http://127.0.0.1:8080 --token <their token> \
  --repo acme --scope default --gate intake
echo "dana's work" > dana.md
converge snap -m dana
converge publish
```

`converge member list` shows who holds what. Only repo admins can add
members, and only server admins can create repos — publish rights are not
admin rights.

## 6. When two people change the same file

The publish that lands second produces a superposed bundle. From there:

```bash
converge inbox                    # names the bundle and the command to run
converge resolve list <bundle>    # the contested paths
converge resolve apply <bundle> decisions.json
converge publish --snap <the resolution snap>
```

Details of that loop: `docs/architecture/17-lineage-and-merge-semantics.md`.

## The terminal UI

Everything above is the CLI, which is the semantic contract — every
front-end drives these same verbs (architecture doc 15). There is also a
terminal UI over the top of them:

```
converge tui        # or run `converge-tui` directly
```

It is a separate binary that ships alongside `converge`, and `converge
tui` hands over to it. Worth knowing which way round that is: the TUI
depends on the CLI for the argv contract, so the CLI cannot depend on
the TUI, and the verb is a convenience rather than the real home.

## Notes on output

`-m/--message` is the message flag on every verb that takes one (`snap`,
`annotate`, `publish`, `release`, `resolve apply`); `--notes` still works
on `publish` and `release` as an alias. Any verb that names a bundle
(`fetch`, `bundle`, `verify`) accepts an id or `--release latest|<version>|<range>`.

Human mode prints transfer progress to stderr, so `--json` output stays
one envelope on stdout and pipelines are safe.

## Next Task

Roadmap `g02.016` is complete. Next: `g02.017` TUI spec parity.
