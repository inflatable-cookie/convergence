# 059 Team Onboarding

Status: complete
Updated: 2026-07-25
Roadmap: `g02.016`

## Objective

Audit P1.6: there was no way to set up a team. Repos, users, and grants
existed only as startup flags and a dev seed function, so a second person
could not be added without editing the server's command line and
restarting it.

## Scope of the actual problem

Authorization was fully enforced and had been since 11.1 — what was
missing was every operation that *creates* the things it authorizes
against. Tokens lived in a `HashMap` built from `--token` flags, so
issuing one meant a restart. Repo creation existed only inside
`--seed-dev`. Nothing listed who was on a repo.

## In Scope

- tokens persisted in the metadata store, hashed, with runtime issuance
- `converge-server --bootstrap-admin <handle>`: first admin, one token
- site admin as a grant on the `*` repo; `create_repo` for site admins
- `converge repo create`, `converge member add`, `converge member list`
- a two-user quickstart that a test keeps honest

## Out Of Scope

- token TTL, rotation, and revocation UX (doc 14 §7 backlog; the trigger
  is a deployment outside a trusted network)
- teams/groups as grant subjects: one subject per grant is enough for
  the beachhead, and group expansion is a semantics decision, not
  plumbing

## Acceptance Criteria

- a bare server plus one bootstrap admin can reach two people publishing
  into the same repo, using only documented commands; all suites green

## Outcome

- tokens are stored **hashed** (blake3) and minted from the OS CSPRNG
  via `getrandom`. The server needs only to recognise a token, so a
  leaked database hands over nothing usable
- lookup order is startup flags, then the store: dev `--token` pairs keep
  working, and issued tokens need no restart
- `--bootstrap-admin` is idempotent about *credentials*: the grant is
  re-applied, but a token is minted only when the subject has none. A
  restart must not print new secrets into a log
- site admin is a grant against the `*` repo, which works because
  `has_grant` matches `repo_id` exactly — a repo admin cannot widen into
  a site admin by accident, and `authorize` was not touched
- `repo create` provisions scope `default` and gate `intake` and grants
  the creator everything. A repo with no gate cannot accept a publish, so
  shipping one would be a trap dressed as a feature
- `member add` refuses unknown capability strings rather than storing
  rows that silently grant nothing, and prints the exact `converge login`
  line for the new member
- `login` writing local config only is what makes the flow work: the
  admin names a repo that does not exist, then creates it
- `docs/guides/001-two-user-quickstart.md` documents the flow and
  `onboarding_e2e.rs` runs it — bootstrap, create, publish, onboard,
  second-person publish, plus the negative cases (publish rights are not
  admin rights, repo creation is site-admin only, unknown token refused)
- 161 tests green

## Next Task

Batch card 16.4 (output polish: Debug leaks, message-flag consistency,
`watch --json` envelope, transfer progress).
