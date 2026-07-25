# 071 Secret Verbs

Status: complete
Updated: 2026-07-25
Roadmap: `g02.019`

## Objective

`converge secret set|get|list|rm`: encryption on the client, the wire
from 19.2 underneath, and nothing about the flow that requires knowing
how any of it works.

## Scope of the actual problem

19.1 made keys, 19.2 made an envelope service. Neither is reachable by a
person. This batch is the join: a value goes in, ciphertext goes to the
server, and the value comes back only on a machine holding the key.

Two details decide whether this is safe in practice. Where the plaintext
*enters* — a command-line argument lands in shell history and process
listings, so values come from stdin. And which keys it is sealed to —
sealing only to the newest key would make every earlier key useless the
moment someone rotates.

## In Scope

- `converge secret set NAME` — value from stdin (hidden prompt on a
  terminal), sealed to every key the caller has registered in this repo
- `converge secret get NAME` — decrypt with whichever local key fits
- `converge secret list` — names, owners, versions; never values
- `converge secret rm NAME`
- read-modify-write with the version guard from 19.2, so a rotation
  racing another rotation is refused rather than lost
- actionable failures when there is no key, no registered key, or no
  key that fits the ciphertext

## Out Of Scope

- consumption surfaces: `converge run --secret`, `write-env`,
  `secret.read` events, redaction (19.5)
- sharing with other people (`g02.020`) — recipients are still just the
  caller's own keys
- migrating the workspace's own remote token (19.4)

## Acceptance Criteria

- a value set on one machine is readable on that machine and by nobody
  else; the plaintext never appears in argv; a rotated key does not
  strand existing secrets; all suites green

## Validation

- `effigy validate`
- `effigy qa:docs`

## Outcome

- `converge secret set|get|list|rm`, encrypting locally and moving only
  ciphertext over the wire
- **the value can only enter through stdin.** There is no `--value`
  flag, and a test asserts the help text never grows one: a
  command-line argument lands in shell history and in every process
  listing on the machine, so the flag would quietly undo the feature.
  A terminal gets a hidden prompt; a pipe is read to end, with one
  trailing newline treated as the shell's rather than the secret's
- **secrets are sealed to every key the caller holds**, not the newest.
  Sealing to one key would strand every earlier secret the moment
  someone rotates; a test rotates and then reads a secret written
  beforehand
- decryption tries each local key in turn, which is the other half of
  that: the old key stays on the machine (19.1) precisely so a rotation
  is not a migration
- `secret get` prints the bare value in human mode so `$(...)` captures
  it cleanly, and `{name, version, value}` under `--json` — the seam
  doc 19 §10b hands to injectors
- writes are read-modify-write against 19.2's version guard, so a
  rotation racing another rotation is refused rather than silently
  overwriting
- failures name the fix: no key says `converge key init`, an
  unregistered key says `converge key rotate`, and a wrong passphrase
  says so rather than reporting a decryption failure
- `secret rm` prints that the credential itself is unchanged and should
  be rotated at its source — the same honesty doc 19 §6 requires of
  recipient removal
- the TUI gained `key` and `secret` in the palette, both on the async
  worker, and `secret rm` confirms once
- 214 tests green

## Next Task

Batch card 19.4 (token migration and adversarial tests).
