# Convergence — current state

The rebuilt stack is on `main`: CLI, TUI, single-process server, Postgres/S3
backends, gate graph, identity, secrets, git interop, and semver releases.
The g01-era implementation lives at tag `v0-legacy` and branch `archive/g01`.
No tagged product release has been cut.

Work is captured continuously or explicitly, then converged through
configurable gates. Conflicts stay as data until a gate policy resolves them.
The vocabulary is load-bearing: `snap`, `publish`, `candidate`, `promote`,
`release`, `superposition`.

## Knowledge (internal truth)

- Index: [knowledge/README.md](knowledge/README.md)
- Vision: [knowledge/vision.md](knowledge/vision.md)
- Architecture: [knowledge/architecture/](knowledge/architecture/README.md)
- Contracts: [knowledge/contracts/](knowledge/contracts/README.md)
- Release: [knowledge/contracts/release.md](knowledge/contracts/release.md)

## Product documentation

- [guides/](guides/) — task-shaped walkthroughs proven by tests
- [git-podcast/](git-podcast/README.md) — origin rationale
- [research/](research/README.md) — comparative-systems evidence
- [rebuild/](rebuild/) — rebuild capture, including the TUI UX spec

## What's next

The project's plan is in Queue: its lanes, their documents and their order.
