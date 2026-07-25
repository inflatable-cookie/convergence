# 067 Properties And Pathological Input

Status: complete
Updated: 2026-07-25
Roadmap: `g02.018`

## Objective

Move the core invariants from example-based tests to properties over
generated input, and cover the filenames the audit named as an untested
class — the class that hides fast-import bugs.

## Scope of the actual problem

Merge determinism, variant-key stability, and lineage identity were each
pinned by one or two hand-picked cases. A hand-picked case proves the
code handles *that* case. And nothing anywhere tested a filename with a
newline, a quote, or two unicode normalisations of the same word.

## In Scope

- merge determinism and idempotence over generated windows
- keyed resolution independent of variant order
- lineage identity: distinct triples never collide, order matters, tree
  matters
- hostile filenames through capture → restore and through git export

## Out Of Scope

- adding `proptest`. The repo already generates with a seeded xorshift
  (chunking properties), and a failure that names its seed reproduces
  exactly. Shrinking is the feature a fuzzing crate would add, and it is
  worth less here than a suite that never reports "sometimes"
- filename fuzzing through the *server*: names travel as manifest
  entries, which the merge treats as opaque strings; the risky
  translation is to a filesystem or a git stream, and both are local

## Outcome

Three defects, all in the class the audit predicted:

- **the fast-import newline bug, as forecast.** `M {mode} inline {path}`
  emitted paths unquoted, so a file named `new\nline.txt` split the
  command and git read the rest of the name as the next instruction. The
  stream was not corrupted so much as *reinterpreted*. Paths are now
  C-quoted whenever they carry a quote, a backslash, or a control
  character
- **a backslash filename was capturable but not restorable.** The
  hostile-manifest check (batch 12.1) banned `\` outright, which is
  right on Windows and wrong everywhere else: capture accepted the name,
  restore refused it, and the workspace could not be recovered on the
  platform that produced it. The ban is now `cfg!(windows)`-gated; the
  traversal defence is unchanged, since `components()` already rejects
  `..`, absolute paths, and anything that is not one plain component
- **`None` and `Some("")` hashed to the same snap id.** `unwrap_or("")`
  erased the difference between "no provenance edge" and "an empty one".
  Records are write-once, so a malformed record would have squatted an
  honest snap's id and locked it out. Identity gains a presence byte and
  the domain tag moves to `converge-snap-v4` (pre-1.0, no shim); doc 17
  §1 carries the formula
- unicode normalisation is left to the filesystem, deliberately: where a
  platform keeps NFC and NFD as two files, Convergence keeps two entries;
  where it folds them, so do we. Normalising in the store would silently
  merge two files a user can see side by side
- 196 tests green

## Next Task

Batch card 18.4 (live backend lane) — done, card 068.
