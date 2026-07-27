# Batch 26.4 — Multi-Gate End To End

Driving a staged gate graph by hand, on the shakedown repo, the way
batch 22.4 drove everything else. The graph — `intake → review (1
approval) → release (releasable)` — is the first that has ever existed,
because until `g02.026` the graph could not be changed after repo
creation.

Four findings. Three are one assumption wearing three hats; the fourth
is mine, from batch 22.4.

## 1. A gate graph deeper than two levels could not be traversed (fixed)

`promote` checked the target gate's upstreams against the gate that
*produced* the bundle. A bundle keeps its producing gate for ever, so
after `intake → review` the bundle was still "from intake", and
`review → release` was refused:

```
gate release does not accept promotions from intake
```

Every gate whose upstream was not an entry gate was unreachable. A
staged pipeline — the thing gates are for — could be described and never
walked.

Doc 14 §3 has always said re-promoting "to a further downstream gate
records the promotion", so the intent was there; only the check was
narrow. Promotion now considers where the bundle has *got to*: its
producing gate plus every gate a recorded promotion delivered it to.
Fan-out to siblings still works, and skipping a stage is still refused —
promoting straight from intake to release fails until the bundle has
actually reached review, which the test asserts.

## 2. `required_approvals` was never enforced on any gate but the first (fixed)

The same line chose which gate's approval policy applied: the producing
gate's. In a staged graph that is the entry gate, which typically
requires none. So `review`'s "1 approval" was configuration that did
nothing, on the gate whose entire purpose is to require one.

The policy that applies is now the gate being promoted *out of*. On the
real repo, the second hop refused with `0 of 1 required approvals`
before an approval, and went through after — the first time that setting
has ever bitten.

## 3. A bundle could not be released from the gate it reached (fixed)

`release` read `may_release` off the producing gate. In a staged graph
that is the entry gate, and an entry gate that may release is not a
staged graph. So a bundle promoted all the way into a release gate was
refused with `gate intake may not release`.

Now it releases if any gate it has reached may release, and the refusal
names the whole path rather than one gate.

## 4. A shortened id was recorded as itself (fixed) — from batch 22.4

Batch 22.4 taught the server to accept shortened bundle ids, because the
CLI prints them (finding 22). What that cost, unnoticed until now: every
verb that *records* an id wrote back whatever the caller typed.
`get_bundle` resolved the prefix, and then the handler kept using the
twelve-character string.

The live deployment held a promotion row, an approval and a release
record all keyed by `3bfead9b0253`, which references no bundle. Two
consequences, one visible and one not:

- promote compared the partition's stored base against the short string,
  decided the bundle was not the current window, and reported it stale —
  which is how this was found
- **GC protects released bundles by comparing ids.** A truncated id never
  matches, so a released bundle was not protected from collection. That
  one would have been silent

Handlers now shadow the caller's string with the resolved id
immediately after `get_bundle`. The regression test drives approve,
promote and release entirely through a twelve-character prefix and reads
the stored rows back.

Repairing the live deployment meant three `UPDATE`s against
`meta.sqlite` — promotions, approvals, and the release record's JSON.

## The measurement findings 10 and 34 asked for

Both traced to a window that never advances: a single-gate repo can
never GC published objects, and a wedged partition never drains. That
was reasoning, and this is the number.

`publications_to_drop` keeps anything with `seq > window_floor`. Before
any promotion the floor is 0, so **no publication is ever eligible,
whatever the retention policy says**. After the first real promotion
advanced intake's floor to 14:

| | before | after |
| --- | --- | --- |
| Window floor | 0 | 14 |
| Publications droppable | 0 | 14 |
| Objects sweepable | 0 | 33 (21.5 KB) |
| Reachable objects | 134 | 101 |

So the causal story in findings 10 and 34 is confirmed, and gate
administration is what unblocks it. A single-gate repo still cannot
advance its own window — that is not a defect, it is what a single gate
means — but it is now a choice rather than the only possibility.

## Next Task

Close `g02.026`. `22.5` becomes the operator's call again.
