# Value Tracks

Purpose: synthesize research by design problem so Convergence can turn many system-specific observations into a smaller set of coherent architectural lessons.

## Priority Tracks

### Core Convergence Semantics

1. **Continuous capture vs. explicit commit**
   - Snap as point-in-time vs. commit as logical unit
   - Auto-save patterns in existing tools
   - Buildability assumptions and breaking them

2. **Gate-based workflows and phased convergence**
   - CI/CD pipeline patterns in existing systems
   - Perforce streams, GitHub protected branches
   - Policy enforcement points and promotion semantics

3. **Conflict preservation and superpositions**
   - Existing approaches to conflict handling
   - Commutative patches (Pijul, Darcs)
   - Conflict as data vs. conflict as blocker

4. **Bundle semantics and coalescence**
   - How systems represent aggregated changes
   - Patch series, stacked commits, pull requests as bundles
   - Provenance tracking across aggregations

### Scale and Content

5. **Large repository handling**
   - Git partial clones, sparse checkout, VFS for Git
   - Perforce's client views and file locking
   - Sapling's virtual file system approach

6. **Large binary and asset workflows**
   - Git LFS, Git annex, Perforce binary handling
   - Plastic SCM's semantic diff for assets
   - Game industry patterns (Unity, Unreal asset management)

7. **Workspace state management**
   - Working copy models across systems
   - Shelve/stash patterns
   - Jujutsu's "working copy as commit"

### Authority and Collaboration

8. **Server authority models**
   - Centralized (Perforce, Subversion)
   - Distributed with canonical upstream (Git)
   - Federated and peer-to-peer possibilities

9. **Identity and provenance**
   - Cryptographic signatures in VCS
   - Attestation and SBOM integration
   - Multi-factor identity (human + CI + automated)

10. **Permissions and access control**
    - Path-based permissions (Perforce, Gitolite)
    - Branch protection and required reviews
    - Fine-grained capabilities

### Integration and Workflow

11. **Review and approval workflows**
    - Pull request models
    - Stacked diffs/differential (Phabricator, Sapling, JJ)
    - Pre-commit review vs. post-commit audit

12. **CI/CD integration points**
    - Webhooks and event models
    - Required checks and gating
    - Build provenance and reproducibility

13. **Editor and tooling integration**
    - VS Code version control APIs
    - JetBrains VCS integration patterns
    - CLI vs. GUI workflow splits

### Emerging and Frontier

14. **Peer-to-peer and offline-first patterns**
    - Radicle's gossip protocol
    - IPFS-based approaches
    - CRDTs for collaboration

15. **AI-assisted workflows**
    - AI-generated commit messages
    - Automated conflict resolution attempts
    - Semantic code understanding in VCS

16. **Blockchain and attestation**
    - Immutable audit trails
    - Smart contract-based access control
    - Supply chain security patterns

## Track Method

For each track:
1. Summarize the shared problem
2. Compare how at least three systems approached it
3. Identify repeat failure patterns
4. Identify promising frontier work outside mainstream systems
5. Write Convergence implications and prototype/evidence needs

## Track Template

Use `docs/research/templates/value-track-synthesis-template.md`.

## Current Syntheses

### Phase 2 Complete (g01.044)

- [continuous-capture-vs-explicit-commit.md](./continuous-capture-vs-explicit-commit.md) — ✅ Complete
- [gate-based-workflows.md](./gate-based-workflows.md) — ✅ Complete
- [conflict-preservation.md](./conflict-preservation.md) — ✅ Complete

### Summary

Three core value tracks synthesizing Convergence's novel semantics:

| Track | Core Question | Key Insight |
|-------|---------------|-------------|
| **Track 1** | What is a `snap`? | Automatic capture, explicit publish |
| **Track 2** | What is a `gate`? | Policy boundary producing bundles |
| **Track 3** | What is a `superposition`? | First-class conflict as data |

### Planned for Future Phases

Priority order for expansion:
4. Large binary and asset workflows (games/VFX use case)
5. Workspace state management
6. Server authority models (centralized vs. distributed)
7. Review and approval workflows

## Next Task

After Git, Mercurial, and Perforce dossiers are written, synthesize:
- Track 1: Continuous capture (compare Git's index, Fossil's auto-sync, Jujutsu's auto-amend)
- Track 2: Gate workflows (compare Perforce streams, GitHub branch protection, CI gating)

These two tracks will inform the core `snap` → `publish` → `bundle` → `promote` semantics.
