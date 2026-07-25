//! Batch 19.3: the secret verbs, driven the way a person drives them.
//!
//! The claim under test is the product's whole promise: a value stored
//! from one machine comes back on that machine and nowhere else — not
//! for a teammate holding every capability, and not for anyone reading
//! the server's database.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use anyhow::Result;
use converge_server::{AppState, FsObjectStore, MetadataStore, SqliteMetadataStore, router};
use std::ops::Not;

fn converge_with_stdin(dir: &Path, home: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_converge"))
        .current_dir(dir)
        .env("CONVERGE_HOME", home)
        .env("CONVERGE_PASSPHRASE", "correct horse battery staple")
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn converge");
    if let Some(text) = stdin {
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(text.as_bytes())
            .expect("write stdin");
    }
    child.wait_with_output().expect("run converge")
}

fn converge(dir: &Path, home: &Path, args: &[&str]) -> Output {
    converge_with_stdin(dir, home, args, None)
}

fn json_data(out: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(text.trim()).expect("parse envelope");
    assert_eq!(v["ok"], true, "envelope not ok: {v}");
    v["data"].clone()
}

fn start_server(data_dir: &Path) -> Result<String> {
    let meta = SqliteMetadataStore::open(&data_dir.join("meta.sqlite"))?;
    meta.create_repo("repo")?;
    meta.create_scope("repo", "default", "2026-07-25T00:00:00Z")?;
    for subject in ["alice", "bob"] {
        meta.upsert_user(subject)?;
        for capability in ["read", "publish", "secret", "admin"] {
            meta.add_grant(subject, "repo", "*", capability)?;
        }
    }
    let state = AppState {
        meta: Arc::new(meta),
        objects: Arc::new(FsObjectStore::new(data_dir)),
        tokens: HashMap::from([
            ("token-a".to_string(), "alice".to_string()),
            ("token-b".to_string(), "bob".to_string()),
        ]),
        gc_running: Default::default(),
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::from_std(listener).expect("adopt listener");
            axum::serve(listener, router(state)).await.expect("serve");
        });
    });
    Ok(format!("http://{addr}"))
}

/// A workspace with a logged-in, keyed identity.
fn member(base_url: &str, token: &str) -> Result<(tempfile::TempDir, tempfile::TempDir)> {
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            home.path(),
            &[
                "login", "--url", base_url, "--token", token, "--repo", "repo", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );
    assert!(
        converge(ws.path(), home.path(), &["key", "init", "--yes"])
            .status
            .success()
    );
    Ok((ws, home))
}

#[test]
fn a_secret_round_trips_for_its_owner_and_nobody_else() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    let stored = json_data(&converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "set", "db-password"],
        Some("hunter2\n"),
    ));
    assert_eq!(stored["name"], "db-password");
    assert_eq!(stored["version"], 1);

    // The owner reads it back, and the trailing newline the shell added
    // is not part of the secret.
    let got = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "get", "db-password"],
    );
    assert!(
        got.status.success(),
        "{}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "hunter2");

    // Bob holds every capability including admin, and cannot read it.
    let denied = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "db-password"],
    );
    assert!(
        !denied.status.success(),
        "a non-recipient read the secret: {}",
        String::from_utf8_lossy(&denied.stdout)
    );
    assert!(
        !String::from_utf8_lossy(&denied.stdout).contains("hunter2"),
        "the value leaked into a failed read"
    );

    // Bob can see that it exists — that is what lets him ask for access.
    let listed = json_data(&converge(
        bob_ws.path(),
        bob_home.path(),
        &["--json", "secret", "list"],
    ));
    assert_eq!(listed.as_array().map(Vec::len), Some(1));
    assert_eq!(listed[0]["name"], "db-password");
    assert_eq!(listed[0]["owner"], "alice");
    assert!(
        !listed.to_string().contains("hunter2"),
        "a listing carried the value"
    );

    // Nothing on the server holds the plaintext.
    let db = std::fs::read(server_dir.path().join("meta.sqlite"))?;
    assert!(
        !String::from_utf8_lossy(&db).contains("hunter2"),
        "the plaintext reached the server"
    );
    Ok(())
}

#[test]
fn the_value_never_appears_in_argv() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    // There is no flag that takes a value: the only way in is stdin.
    let help = converge(ws.path(), home.path(), &["secret", "set", "--help"]);
    let text = String::from_utf8_lossy(&help.stdout).to_lowercase();
    assert!(
        !text.contains("--value") && !text.contains("--from"),
        "a value-bearing flag exists, so values will end up in shell history:\n{text}"
    );
    Ok(())
}

#[test]
fn rotating_a_key_does_not_strand_existing_secrets() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "before-rotation"],
        Some("old-value\n"),
    );
    assert!(
        converge(ws.path(), home.path(), &["key", "rotate"])
            .status
            .success()
    );

    // Sealed to the old key, still readable: rotation keeps the old key
    // on the machine precisely so this works (batch 19.1).
    let got = converge(
        ws.path(),
        home.path(),
        &["secret", "get", "before-rotation"],
    );
    assert!(
        got.status.success(),
        "a rotation stranded an existing secret: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "old-value");

    // A secret written after the rotation is sealed to both keys.
    let stored = json_data(&converge_with_stdin(
        ws.path(),
        home.path(),
        &["--json", "secret", "set", "after-rotation"],
        Some("new-value\n"),
    ));
    assert_eq!(
        stored["recipients"].as_array().map(Vec::len),
        Some(2),
        "a new secret should be sealed to every key the caller holds"
    );
    Ok(())
}

#[test]
fn updating_and_deleting_behave_like_the_rest_of_the_cli() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "token"],
        Some("v1"),
    );
    let updated = json_data(&converge_with_stdin(
        ws.path(),
        home.path(),
        &["--json", "secret", "set", "token"],
        Some("v2"),
    ));
    assert_eq!(updated["version"], 2, "a second write is a new version");
    let got = converge(ws.path(), home.path(), &["secret", "get", "token"]);
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "v2");

    assert!(
        converge(ws.path(), home.path(), &["secret", "rm", "token"])
            .status
            .success()
    );
    let gone = converge(ws.path(), home.path(), &["secret", "get", "token"]);
    assert!(
        !gone.status.success(),
        "a deleted secret was still readable"
    );
    Ok(())
}

#[test]
fn a_machine_without_the_key_gets_told_what_to_do() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "thing"],
        Some("value"),
    );

    // Same person, new machine: logged in, no key yet.
    let home = tempfile::tempdir()?;
    let ws = tempfile::tempdir()?;
    assert!(converge(ws.path(), home.path(), &["init"]).status.success());
    assert!(
        converge(
            ws.path(),
            home.path(),
            &[
                "login", "--url", &base_url, "--token", "token-a", "--repo", "repo", "--scope",
                "default", "--gate", "intake",
            ],
        )
        .status
        .success()
    );

    let out = converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "another"],
        Some("value"),
    );
    let message = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(!out.status.success());
    assert!(
        message.contains("key init"),
        "the error should name the fix, got: {message}"
    );
    Ok(())
}

/// Batch 19.5: a secret reaches a child process without ever being
/// written into the working tree.
#[test]
fn run_injects_named_secrets_into_one_child_and_nowhere_else() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;

    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "db-password"],
        Some("hunter2"),
    );

    // The derived variable name is the conventional shape.
    let out = converge(
        ws.path(),
        home.path(),
        &[
            "run",
            "--secret",
            "db-password",
            "--",
            "sh",
            "-c",
            "printf %s \"$DB_PASSWORD\"",
        ],
    );
    assert!(
        out.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hunter2");

    // An explicit mapping when the names differ.
    let out = converge(
        ws.path(),
        home.path(),
        &[
            "run",
            "--secret",
            "PGPASSWORD=db-password",
            "--",
            "sh",
            "-c",
            "printf %s \"$PGPASSWORD\"",
        ],
    );
    assert_eq!(String::from_utf8_lossy(&out.stdout), "hunter2");

    // Nothing was written to the workspace on the way through.
    let tree: Vec<String> = std::fs::read_dir(ws.path())?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !tree.iter().any(|name| name.contains("env")),
        "run left something behind: {tree:?}"
    );

    // The child's exit code is the command's answer, not ours.
    let failed = converge(
        ws.path(),
        home.path(),
        &["run", "--secret", "db-password", "--", "sh", "-c", "exit 3"],
    );
    assert_eq!(failed.status.code(), Some(3));
    Ok(())
}

/// The escape hatch closes the door behind itself.
#[test]
fn write_env_warns_and_self_ignores() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "api-key"],
        Some("s3cr3t value"),
    );

    let out = converge(ws.path(), home.path(), &["secret", "write-env", ".env"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let said = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(said.contains("plaintext"), "the warning is missing: {said}");
    assert!(
        said.contains("converge run"),
        "it should point at the better option: {said}"
    );

    // The file holds a usable value, quoted so a space survives.
    let written = std::fs::read_to_string(ws.path().join(".env"))?;
    assert_eq!(written.trim(), "API_KEY='s3cr3t value'");

    // And it is ignored, so no snap can ever capture it.
    let ignore = std::fs::read_to_string(ws.path().join(".convergeignore"))?;
    assert!(
        ignore.lines().any(|line| line.trim() == ".env"),
        "the dotenv was not added to .convergeignore: {ignore}"
    );
    let status = json_data(&converge(ws.path(), home.path(), &["--json", "status"]));
    let pending: Vec<&str> = status["pending"]["changes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["path"].as_str())
        .collect();
    assert!(
        !pending.contains(&".env"),
        "the dotenv is capturable despite the ignore rule: {pending:?}"
    );
    // The ignore file itself is project configuration and *should* show
    // up — a teammate restoring the snap needs it.
    assert!(
        pending.contains(&".convergeignore"),
        "expected the ignore file to be capturable: {pending:?}"
    );
    Ok(())
}

/// Reads are on the record (doc 19 §10c).
#[test]
fn reading_a_secret_leaves_an_audit_event() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (ws, home) = member(&base_url, "token-a")?;
    converge_with_stdin(
        ws.path(),
        home.path(),
        &["secret", "set", "audited"],
        Some("v"),
    );
    converge(ws.path(), home.path(), &["secret", "get", "audited"]);

    let events = json_data(&converge(ws.path(), home.path(), &["--json", "events"]));
    let kinds: Vec<&str> = events
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["kind"].as_str())
        .collect();
    assert!(
        kinds.contains(&"secret.read"),
        "a read left no trace: {kinds:?}"
    );
    assert!(
        kinds.contains(&"secret.changed"),
        "a write left no trace: {kinds:?}"
    );
    assert!(
        !events.to_string().contains("\"v\""),
        "an event carried the value"
    );
    Ok(())
}

/// Batch 20.1: a secret two people can read, and a third cannot.
#[test]
fn sharing_lets_a_teammate_read_and_unsharing_stops_future_versions() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("shared-value"),
    );
    assert!(
        converge(
            bob_ws.path(),
            bob_home.path(),
            &["secret", "get", "deploy-key"]
        )
        .status
        .success()
        .not(),
        "bob could read before being shared with"
    );

    let shared = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "share", "deploy-key", "--with", "bob"],
    ));
    assert_eq!(shared["version"], 2, "sharing writes a new version");
    assert_eq!(
        shared["recipients"].as_array().map(Vec::len),
        Some(2),
        "sealed to both people's keys"
    );

    let got = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(
        got.status.success(),
        "bob still cannot read it: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&got.stdout).trim_end(),
        "shared-value"
    );

    // Unsharing is honest about what it is not.
    let out = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "unshare", "deploy-key", "--from", "bob"],
    );
    assert!(out.status.success());
    let said = String::from_utf8_lossy(&out.stdout).to_lowercase();
    assert!(
        said.contains("rotate the credential"),
        "unshare must say what it does not do: {said}"
    );
    assert!(
        !said.contains("revoke"),
        "unshare must not claim to revoke access: {said}"
    );

    // Future versions are closed to bob.
    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("rotated-value"),
    );
    let denied = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(
        !denied.status.success(),
        "bob read a version written after being unshared"
    );
    Ok(())
}

/// Two people can hold the same secret name without either being served
/// the wrong one — the resolution defect batch 20.1 fixed.
#[test]
fn two_owners_can_hold_the_same_name() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "db-password"],
        Some("alice-value"),
    );
    converge_with_stdin(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "set", "db-password"],
        Some("bob-value"),
    );

    // Each reads their own, whatever the storage order happens to be.
    let alice_got = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "get", "db-password"],
    );
    assert_eq!(
        String::from_utf8_lossy(&alice_got.stdout).trim_end(),
        "alice-value"
    );
    let bob_got = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "db-password"],
    );
    assert_eq!(
        String::from_utf8_lossy(&bob_got.stdout).trim_end(),
        "bob-value"
    );

    // Both records exist, listed with their owners.
    let listed = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "list"],
    ));
    assert_eq!(listed.as_array().map(Vec::len), Some(2));

    // Shared with alice, bob's copy is reachable by naming the owner.
    converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "share", "db-password", "--with", "alice"],
    );
    let explicit = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "get", "db-password", "--owner", "bob"],
    );
    assert_eq!(
        String::from_utf8_lossy(&explicit.stdout).trim_end(),
        "bob-value",
        "--owner did not select the other person's secret"
    );
    Ok(())
}

/// Batch 20.2: removing a member is honest about what it did not do.
#[test]
fn removing_a_member_reports_the_secrets_still_sealed_to_them() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("shared-value"),
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );

    let out = converge(
        alice_ws.path(),
        alice_home.path(),
        &["member", "remove", "bob"],
    );
    assert!(
        out.status.success(),
        "removal failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("deploy-key"),
        "the affected secret was not named: {said}"
    );
    assert!(
        said.contains("unshare") && said.to_lowercase().contains("rotate"),
        "removal must say what the owner still has to do: {said}"
    );

    // Bob really is out: the server refuses him now.
    let denied = converge(
        bob_ws.path(),
        bob_home.path(),
        &["--json", "secret", "list"],
    );
    assert!(
        !denied.status.success(),
        "a removed member still reached the repo"
    );

    // Removing the last admin is refused rather than locking everyone out.
    let out = converge(
        alice_ws.path(),
        alice_home.path(),
        &["member", "remove", "alice"],
    );
    assert!(!out.status.success(), "a repo can be left with no admin");
    Ok(())
}

/// `secret audit` shows who can read what, and flags recipients who have
/// left — the state that membership change creates and nothing else
/// surfaces.
#[test]
fn audit_flags_recipients_who_are_no_longer_members() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (_bob_ws, _bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("v"),
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );

    // While bob is a member, he is simply a reader.
    let audit = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "audit"],
    ));
    let row = &audit[0];
    let readers: Vec<&str> = row["readers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r.as_str())
        .collect();
    assert!(readers.contains(&"bob"), "bob should be listed: {row}");
    assert_eq!(row["stale"].as_array().map(Vec::len), Some(0));

    // After removal, the same recipient is flagged rather than silently
    // remaining in the list.
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["member", "remove", "bob"],
    );
    let audit = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "audit"],
    ));
    let stale = audit[0]["stale"].as_array().cloned().unwrap_or_default();
    assert_eq!(
        stale.len(),
        1,
        "the departed recipient was not flagged: {audit}"
    );
    assert_eq!(stale[0]["subject"], "bob");
    assert!(
        stale[0]["why"]
            .as_str()
            .unwrap()
            .contains("no longer a member"),
        "{stale:?}"
    );

    // Human output points at the fix without claiming access was undone.
    let text = String::from_utf8_lossy(
        &converge(alice_ws.path(), alice_home.path(), &["secret", "audit"]).stdout,
    )
    .to_lowercase();
    assert!(text.contains("stale recipient"), "{text}");
    assert!(text.contains("rotate at the source"), "{text}");
    assert!(
        !text.contains("revoke"),
        "audit must not claim revocation: {text}"
    );
    Ok(())
}

/// Batch 20.3: the defect that prompted the card. Updating a shared
/// secret must keep everyone who could read it — sealing to the writer's
/// own keys unshares the rest silently, which is the worst way to lose
/// access.
#[test]
fn updating_a_shared_secret_keeps_its_recipients() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("first"),
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );

    // Alice updates the value without touching sharing.
    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("second"),
    );

    let got = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(
        got.status.success(),
        "updating silently unshared bob: {}",
        String::from_utf8_lossy(&got.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "second");

    // Same for the explicit rotate verb.
    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "rotate", "deploy-key"],
        Some("third"),
    );
    let got = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert_eq!(String::from_utf8_lossy(&got.stdout).trim_end(), "third");
    Ok(())
}

/// An audit can tell a rotation from a re-share.
#[test]
fn value_version_counts_rotations_not_reshares() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (_bob_ws, _bob_home) = member(&base_url, "token-b")?;

    let first = json_data(&converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "set", "api-key"],
        Some("v1"),
    ));
    assert_eq!(first["version"], 1);
    assert_eq!(first["value_version"], 1);

    // Sharing writes a version but not a value version.
    let shared = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "share", "api-key", "--with", "bob"],
    ));
    assert_eq!(shared["version"], 2, "sharing is still a write");
    assert_eq!(
        shared["value_version"], 1,
        "sharing must not look like a rotation"
    );

    // Rotating moves both.
    let rotated = json_data(&converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "rotate", "api-key"],
        Some("v2"),
    ));
    assert_eq!(rotated["version"], 3);
    assert_eq!(rotated["value_version"], 2, "a rotation is a value change");

    // Unsharing does not.
    let unshared = json_data(&converge(
        alice_ws.path(),
        alice_home.path(),
        &["--json", "secret", "unshare", "api-key", "--from", "bob"],
    ));
    assert_eq!(unshared["value_version"], 2);

    // And the audit reports it, which is the question an operator asks
    // after somebody leaves.
    let audit = String::from_utf8_lossy(
        &converge(alice_ws.path(), alice_home.path(), &["secret", "audit"]).stdout,
    )
    .into_owned();
    assert!(
        audit.contains("value last changed"),
        "audit does not report when the value changed: {audit}"
    );
    assert!(audit.contains("value version 2"), "{audit}");
    Ok(())
}

/// Batch 20.4: the trap between two correct decisions. Preserving
/// recipients on rotation (20.3) and leaving a departed member's key in
/// place (20.2) are each right, and together they re-seal a rotated
/// credential to someone who has left.
#[test]
fn rotating_after_someone_leaves_warns_that_they_are_still_a_recipient() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("v1"),
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["member", "remove", "bob"],
    );

    // Rotating still works — an operator mid-incident needs the new
    // value stored — but it says what it just did.
    let rotated = converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "rotate", "deploy-key"],
        Some("v2"),
    );
    assert!(rotated.status.success());
    let warning = String::from_utf8_lossy(&rotated.stderr);
    assert!(
        warning.contains("bob") && warning.contains("left the repo"),
        "rotation did not warn about the departed recipient: {warning}"
    );
    assert!(
        warning.contains("unshare"),
        "the warning should name the fix: {warning}"
    );

    // Bob cannot reach the new value: his grants are gone.
    let denied = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(
        !denied.status.success(),
        "a removed member read a value written after their removal"
    );

    // After unsharing, the warning stops and bob is off the list.
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "unshare", "deploy-key", "--from", "bob"],
    );
    let clean = converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "rotate", "deploy-key"],
        Some("v3"),
    );
    assert!(
        !String::from_utf8_lossy(&clean.stderr).contains("left the repo"),
        "the warning survived the fix: {}",
        String::from_utf8_lossy(&clean.stderr)
    );
    Ok(())
}

/// A stale recipient cannot sit unnoticed: audit reports it whether or
/// not anyone has rotated since.
#[test]
fn a_stale_recipient_cannot_persist_unnoticed() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (_bob_ws, _bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("v1"),
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["member", "remove", "bob"],
    );

    for _ in 0..2 {
        let audit = json_data(&converge(
            alice_ws.path(),
            alice_home.path(),
            &["--json", "secret", "audit"],
        ));
        let stale = audit[0]["stale"].as_array().cloned().unwrap_or_default();
        assert_eq!(
            stale.len(),
            1,
            "the stale recipient vanished from audit: {audit}"
        );
        assert_eq!(stale[0]["subject"], "bob");

        // Rotating must not quietly clear the flag: the key is still on
        // the record until someone unshares.
        converge_with_stdin(
            alice_ws.path(),
            alice_home.path(),
            &["secret", "rotate", "deploy-key"],
            Some("rotated"),
        );
    }
    Ok(())
}

/// Two writers racing: one wins, the other is told to retry, and no
/// recipient is lost either way.
#[test]
fn concurrent_share_and_rotate_cannot_lose_a_recipient() -> Result<()> {
    let server_dir = tempfile::tempdir()?;
    let base_url = start_server(server_dir.path())?;
    let (alice_ws, alice_home) = member(&base_url, "token-a")?;
    let (bob_ws, bob_home) = member(&base_url, "token-b")?;

    converge_with_stdin(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "set", "deploy-key"],
        Some("v1"),
    );

    // Both operations read version 1 and then write. The version guard
    // means exactly one lands.
    let share = std::thread::spawn({
        let ws = alice_ws.path().to_path_buf();
        let home = alice_home.path().to_path_buf();
        move || {
            converge(
                &ws,
                &home,
                &["secret", "share", "deploy-key", "--with", "bob"],
            )
        }
    });
    let rotate = {
        let ws = alice_ws.path().to_path_buf();
        let home = alice_home.path().to_path_buf();
        converge_with_stdin(&ws, &home, &["secret", "rotate", "deploy-key"], Some("v2"))
    };
    let share = share.join().expect("share thread");

    let winners = [share.status.success(), rotate.status.success()]
        .iter()
        .filter(|ok| **ok)
        .count();
    assert!(winners >= 1, "both writers failed");

    // Whatever happened, the secret is intact and readable by its owner.
    let got = converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(got.status.success(), "the secret was left unreadable");

    // Re-running the loser lands cleanly, and bob ends up a recipient
    // with the current value — nothing was lost, only retried.
    converge(
        alice_ws.path(),
        alice_home.path(),
        &["secret", "share", "deploy-key", "--with", "bob"],
    );
    let bob_got = converge(
        bob_ws.path(),
        bob_home.path(),
        &["secret", "get", "deploy-key"],
    );
    assert!(
        bob_got.status.success(),
        "bob could not read after the retry: {}",
        String::from_utf8_lossy(&bob_got.stderr)
    );
    let owner_value = String::from_utf8_lossy(&got.stdout).trim_end().to_string();
    assert_eq!(
        String::from_utf8_lossy(&bob_got.stdout).trim_end(),
        owner_value,
        "owner and recipient see different values"
    );
    Ok(())
}
