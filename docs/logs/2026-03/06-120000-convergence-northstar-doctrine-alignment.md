# Convergence Northstar Doctrine Alignment

Status: Complete
Owner: Documentation
Date: 2026-03-06
Related roadmap: `g01.042`

## Change Summary

- added the Northstar core docs spine under `docs/vision/`, `docs/roadmaps/`, and `docs/logs/`
- moved the flat roadmap corpus into `docs/roadmaps/g01/`
- moved decision history into `docs/logs/2026-01/`
- rewrote root/docs guidance to the new structure and removed the retired flat folders

## Files Touched

- `docs/README.md`
- `docs/vision/`
- `docs/roadmaps/`
- `docs/logs/`
- `README.md`
- `AGENTS.md`
- `docs/architecture/README.md`
- `docs/processes/260-agents-operating-guardrails.md`

## Why

Convergence had strong content but the older flat `docs/roadmap/` and `docs/decisions/` layout made the docs contract inconsistent with the Northstar standard now used across the project set.

## Vision Target Delta

- `MAINT`: the documentation structure is now canonical and easier to keep consistent.
- `UX`: roadmap and rationale discovery is simpler for fresh operators and agents.

## Next Task

Open `g01.043` only when there is a real next execution milestone, and record future rationale or batch updates in `docs/logs/YYYY-MM/`.
