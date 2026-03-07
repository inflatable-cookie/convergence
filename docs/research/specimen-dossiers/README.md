# Specimen Dossiers

Purpose: treat real version control and collaboration systems as specimens so Convergence can study shipped strengths, recurring failures, and architectural corrections over time.

## What belongs here

Each dossier should capture:
- Product identity and era context (when created, by whom, for what problem)
- Defining architectural bets (what did they prioritize?)
- Standout strengths (what are they unusually good at?)
- Chronic pain points and production constraints
- Between-release removals, refactors, or reversals (what did they undo?)
- Convergence-relevant lessons
- Source inventory with confidence notes

## Initial Priority Set

### Tier 1 specimens (foundational, must understand):
- `git` — The incumbent. Understand its data model, constraints, and why it won.
- `mercurial` — Contemporary of Git, different choices, similar era.
- `perforce-helix-core` — Enterprise/games, centralized model, gate workflows, file locking.
- `plastic-scm` — Unity's VCS, semantic merge, visual branching, binary handling.

### Tier 2 specimens (significant alternatives):
- `fossil` — SQLite philosophy, integrated bug tracker/wiki, self-contained.
- `pijul` — Patch-based, commutative changes, modern Rust implementation.
- `sapling` — Meta's Git-compatible, scalable, stacked commits.
- `jujutsu` — Google's Rust-based, working copy as commit, conflict commits.

### Tier 3 specimens (specialized or historical):
- `subversion` — Centralized, predecessor understanding.
- `darcs` — Patch theory, commute-based merges.
- `bazaar` — Canonical's attempt, now historical.
- `bitkeeper` — Pre-Git, commercial, famous for the Linux fallout.

### Tier 4 (adjacent collaboration systems):
- GitHub/GitLab/Bitbucket — How platforms extend VCS with workflows
- Reviewable, Gerrit — Code review as primary interface
- Phabricator/Phorge — Differential, stacked diffs

## Dossier Rules

- Prefer one dossier per system family, with release-era subsections inside it.
- Do not flatten every version into a separate top-level file unless the architecture meaningfully reset.
- Always include a `between releases` section documenting what changed and why.
- Flag uncertain claims explicitly instead of smoothing them into narrative fact.
- Be honest about failure modes — every system has them.

## Output Standard

Each dossier should answer:
- What this system is unusually good at
- What it paid to get there
- What broke under scale, content pressure, or team size
- What Convergence should study further versus reject early

## Dossier Template

Use `docs/research/templates/specimen-dossier-template.md`.

## Current Dossiers

### Phase 1 Complete (g01.043)

- [git.md](./git.md) — ✅ Complete (incumbent, object store, packfiles)
- [mercurial.md](./mercurial.md) — ✅ Complete (contemporary, revlog, phases)
- [perforce-helix-core.md](./perforce-helix-core.md) — ✅ Complete (centralized, streams, locking)
- [plastic-scm.md](./plastic-scm.md) — ✅ Complete (hybrid, semantic merge)
- [jujutsu.md](./jujutsu.md) — ✅ Complete (modern, conflicts-as-data)

### Planned for Future Phases

- [fossil.md](./fossil.md) — Pending (integrated philosophy)
- [pijul.md](./pijul.md) — Pending (patch-based, commutative)
- [sapling.md](./sapling.md) — Pending (Meta's scale solution)
- [subversion.md](./subversion.md) — Pending (historical context)

## Summary

The Phase 1 dossiers establish comparative baseline across architectural approaches:

| System | Model | Key Differentiator |
|--------|-------|-------------------|
| Git | Distributed | Object store, staging area |
| Mercurial | Distributed | Revlog, phases, extensions |
| Perforce | Centralized | Streams, locking, gate workflow |
| Plastic | Hybrid | Semantic merge, visual branching |
| Jujutsu | Distributed (Git-backed) | Conflicts-as-data, operation log |

## Next Task

Proceed to Phase 2 (g01.044): Synthesize value tracks for continuous capture, gate workflows, and conflict preservation. Focus on:
- The immutable object store model
- The staging area (index) design
- Branch vs. tag semantics
- What makes large repositories painful
- What makes binary files painful
- What CI/CD workflows have bolted on

Then write Mercurial and Perforce to establish comparative baseline.
