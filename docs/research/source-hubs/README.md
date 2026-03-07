# Source Hubs

Purpose: keep a curated index of where Convergence should look for high-signal research instead of rediscovering source quality every time a new topic opens.

## Core Hubs

### Primary Sources (authoritative)

**Version Control System Official Documentation**
- Git documentation (git-scm.com/book, official man pages)
- Mercurial wiki and hgbook.red-bean.com
- Perforce documentation (helixdocs.perforce.com)
- Plastic SCM documentation
- Fossil documentation (fossil-scm.org/home/doc/trunk/www/)
- Pijul documentation (pijul.org/manual)
- Sapling documentation (sapling-scm.com/docs/)
- Jujutsu documentation (martinvonz.github.io/jj/)

**Academic Sources**
- ACM Digital Library (SIGSOFT, ICSE, FSE conferences)
- IEEE Xplore (software engineering track)
- arXiv (cs.SE, cs.DC, cs.PL)
- Git Merge conference recordings
- Papers We Love (VCS-related papers)

### Research and Industry Hubs

**Version Control and Developer Tooling**
- GitHub Blog (engineering posts)
- GitLab Handbook (remote workflow documentation)
- Meta Engineering Blog (Sapling-related posts)
- Google Research Blog (Jujutsu, Piper-related)

**Game Industry Workflows**
- GDC Vault (version control, asset pipeline talks)
- Game Developer / Gamasutra postmortems
- Gamasutra Programming / Production categories

**Distributed Systems**
- ACM Queue
- Martin Kleppmann's blog and papers
- Papers on CRDTs (crdt.tech, invisible-college)

## Source Rules

- Capture the strongest source first, then add weaker supporting sources only when they add context.
- Prefer sources that explain why a system changed, not only how it looked at launch.
- When a claim is based on inference rather than an explicit source statement, mark it as inference.
- Track publication date and system/version scope for every source set.

## Output Standard

Each source-hub note should make it easy to answer:
- What kinds of questions this hub is useful for
- How authoritative it is
- Where it tends to be biased or incomplete
- Which Convergence tracks it should feed

## Source-Hub Template

Use `docs/research/templates/source-hub-template.md`.

## Current Source Maps

None yet — create as value tracks are synthesized.

Proposed initial source maps:
- `001-git-internals-and-object-model.md` — Plumbing commands, object store, refs
- `002-mercurial-revlogs-and-changeset-model.md` — Revlog format, manifest structure
- `003-perforce-depot-and-client-view-model.md` — Depot syntax, client specs, streams
- `004-stacked-commits-and-differential-review.md` — Phabricator, Sapling, Jujutsu approaches
- `005-binary-asset-management-patterns.md` — LFS, annex, Perforce binary handling
- `006-distributed-systems-consensus.md` — For server authority and federation research

## Next Task

Create source map 001 (Git internals) as the first research artifact. Focus on:
- Object model (blob, tree, commit, tag)
- Refs and reflog
- Index/staging area mechanics
- Packfiles and delta compression
- Transport protocols

This will serve as reference for comparing all other systems.
