# 2026-07-23 19:00:00 BST - Batch 2.3 Docs Spine Restructure Complete

Roadmap: `g02.002`

## Summary

Reduced the docs tree from 134 to 59 files. Everything remaining is a keeper
(vision, canonical architecture 01+04, guardrails, research findings, podcast
summary, rebuild captures), a live planning surface, or a log.

## Changes

- removed g01 roadmap files, architecture 02-03/05-12 + prototype notes,
  operators/, processes/, testing/, research scaffolding (templates, hubs,
  playbooks, crossrefs, intake), podcast raw transcript + analysis set —
  all reachable on branch `archive/g01`
- rewrote architecture, research, and git-podcast READMEs for the lean spine;
  architecture README names the large-org-claim gap explicitly and assigns it
  to Batch 2.4
- object-model dedupe: canonical statement is architecture/01 + 04; vision
  carries intent only; memo 003 remains as research evidence
- updated docs README structure list, roadmaps front doors, generation index,
  AGENTS references; fixed dangling prototype/g01 links
- closed card `004-docs-spine-restructure.md`; opened ready card
  `005-rebuild-architecture.md`

## Validation

- `effigy qa:docs` / `effigy qa:northstar` — green

## Next Task

Execute the `g02.002` Batch 2.4 rebuild-architecture card.
