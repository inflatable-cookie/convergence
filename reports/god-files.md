# God Files

- Root: `/Users/tom/Dev/projects/convergence`
- Thresholds: warn=`250` high=`400` critical=`700`
- Scanned files: `166`
- Skipped generated: `0`
- Findings: `45`

| Severity | Code Lines | Total Lines | Path |
| --- | ---: | ---: | --- |
| critical | 2395 | 2747 | `crates/converge-cli/src/dispatch.rs` |
| critical | 1629 | 2200 | `crates/converge-tui/src/app.rs` |
| critical | 1117 | 1264 | `crates/converge-server/src/meta_postgres.rs` |
| critical | 1078 | 1228 | `crates/converge-server/src/meta_sqlite.rs` |
| critical | 1005 | 1236 | `crates/converge-cli/tests/secret_verbs.rs` |
| critical | 872 | 1016 | `crates/converge-tui/src/render.rs` |
| critical | 838 | 1087 | `crates/converge-tui/src/main.rs` |
| critical | 721 | 949 | `crates/converge-cli/tests/onboarding_e2e.rs` |
| high | 553 | 675 | `crates/converge-tui/src/wizard.rs` |
| high | 537 | 636 | `crates/converge-tui/src/app/keys.rs` |
| high | 521 | 681 | `crates/converge-cli/tests/cli_verbs.rs` |
| high | 516 | 641 | `crates/converge-server/tests/releases.rs` |
| high | 494 | 629 | `crates/converge-model/src/gates.rs` |
| high | 489 | 589 | `crates/converge-server/tests/identity_adversarial.rs` |
| high | 476 | 593 | `crates/converge-server/src/merge.rs` |
| high | 468 | 601 | `crates/converge-cli/tests/resolve_loop_e2e.rs` |
| high | 424 | 643 | `crates/converge-cli/src/commands.rs` |
| high | 420 | 493 | `crates/converge-server/tests/e2e_sync.rs` |
| warning | 398 | 597 | `crates/converge-model/src/wire.rs` |
| warning | 394 | 461 | `crates/converge-server/tests/lanes.rs` |
| warning | 387 | 533 | `crates/converge-server/tests/gate_administration.rs` |
| warning | 376 | 554 | `crates/converge-server/src/storage.rs` |
| warning | 375 | 456 | `crates/converge-client/src/git_export.rs` |
| warning | 374 | 486 | `crates/converge-cli/src/secrets.rs` |
| warning | 355 | 457 | `crates/converge-cli/tests/secret_adversarial.rs` |
| warning | 338 | 423 | `crates/converge-server/tests/backend_conformance.rs` |
| warning | 333 | 439 | `crates/converge-server/src/http.rs` |
| warning | 318 | 359 | `crates/converge-server/tests/base_aware_merge.rs` |
| warning | 315 | 363 | `crates/converge-server/src/meta_postgres/ops.rs` |
| warning | 315 | 376 | `crates/converge-server/tests/failure_injection.rs` |
| warning | 309 | 355 | `crates/converge-server/tests/decision_table.rs` |
| warning | 309 | 337 | `crates/converge-server/tests/gate_strategies.rs` |
| warning | 301 | 354 | `crates/converge-server/src/engine/publish.rs` |
| warning | 299 | 333 | `crates/converge-server/src/meta_sqlite/ops.rs` |
| warning | 278 | 391 | `crates/converge-cli/src/check.rs` |
| warning | 278 | 336 | `crates/converge-client/src/remote/transport.rs` |
| warning | 272 | 315 | `crates/converge-server/tests/transactions.rs` |
| warning | 272 | 344 | `crates/converge-tui/src/wizard/defs.rs` |
| warning | 271 | 336 | `crates/converge-cli/tests/backup_restore.rs` |
| warning | 268 | 391 | `crates/converge-model/src/overwrite.rs` |
| warning | 266 | 338 | `crates/converge-server/tests/concurrency.rs` |
| warning | 254 | 349 | `crates/converge-server/src/engine/gates.rs` |
| warning | 253 | 345 | `crates/converge-server/src/gc.rs` |
| warning | 251 | 320 | `crates/converge-cli/tests/doctor.rs` |
| warning | 251 | 287 | `crates/converge-client/src/remote/candidates.rs` |