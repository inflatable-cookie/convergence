# 025 Edge And Horizontal Scale

Status: parked — trigger not fired
Owner: repo maintainers
Updated: 2026-07-25

## Context

Doc 14 §7 keeps the target architecture visible and honest: async bundle
builds with partition workers, horizontal scaling across partitions, and
edge nodes doing read-through caching and upload buffering. None is
built. The shipped server is one process, which is both the unit of
availability and the write ceiling — §6 says so rather than implying
otherwise.

`g02.015` measured the merge and found it bounded by changed paths, and
`g02.018` proved the transactional guards hold under real concurrency.
So the ceiling that would justify this work has not been reached; it has
been measured and found adequate.

## Triggers

- **async builds**: publish latency measured as painful in a real
  deployment. Batch 14.2 already declined this once, because publish
  commits publication and bundle in one guarded batch and splitting it
  would reintroduce an interleaving window for unmeasured latency
- **horizontal scaling**: a measured write ceiling from a real workload
- **edge nodes**: a customer with genuine multi-site locality pain

## Sketch (not a plan)

- partition workers with a build queue, and the crash semantics that
  come with them
- a coordination story for multiple processes over one partition
- read-through caches with an invalidation rule that survives promotion

## Next Task

None. Revisit when a trigger fires, and prefer measurement over
anticipation.
